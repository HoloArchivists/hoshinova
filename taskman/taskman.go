package taskman

import (
	"errors"
	"fmt"
	"io"
	"os"
	"time"

	"github.com/hizkifw/hoshinova/config"
	"github.com/hizkifw/hoshinova/logger"
	"github.com/hizkifw/hoshinova/util/atomic"
	"github.com/jedib0t/go-pretty/v6/table"
)

type Step string
type VideoId string

const (
	StepIdle           Step = "idle"
	StepWaitingForLive Step = "waiting for live"
	StepRecording      Step = "recording"
	StepMuxing         Step = "muxing"
	StepMuxed          Step = "muxed"
	StepUploading      Step = "uploading"
	StepUploaded       Step = "uploaded"
	StepDone           Step = "done"
	StepErrored        Step = "errored"
	StepCancelled      Step = "cancelled"
)

var (
	ErrTaskAlreadyExists = errors.New("task already exists")
	ErrTaskNotFound      = errors.New("task not found")
)

// Task represents a single video that is being processed.
type Task struct {
	Video    Video
	Step     Step
	Logs     []LogEntry
	Progress string

	CreatedAt        time.Time
	LastStepUpdate   time.Time
	WorkingDirectory string

	// consumed turns true after the task has been returned in a Subscribe call.
	// It is flipped back to false when the task's step is changed.
	consumed atomic.ABool
}

type Video struct {
	Id          VideoId
	Title       string
	ChannelId   string
	ChannelName string
}

type LogEntry struct {
	Time    time.Time
	Message string
}

type TaskManager struct {
	tasks  *TaskMap
	config *config.Config
	logger logger.Logger
}

func New(config *config.Config, logger logger.Logger) *TaskManager {
	return &TaskManager{
		tasks:  NewTaskMap(),
		config: config,
		logger: logger,
	}
}

func (t *TaskManager) Insert(video Video) (*Task, error) {
	// Check if the task already exists
	if _, ok := t.tasks.Get(video.Id); ok {
		return nil, ErrTaskAlreadyExists
	}

	// Create a temporary working directory
	workdir, err := os.MkdirTemp(t.config.Workdir, "hsnv__"+string(video.Id)+"__")
	if err != nil {
		return nil, fmt.Errorf("failed to create temporary working directory: %w", err)
	}
	t.logger.Debugf("Created temporary working directory %s for video %s\n", workdir, video.Id)

	task := Task{
		Video: video,
		Step:  StepIdle,
		Logs: []LogEntry{
			{
				Time:    time.Now(),
				Message: "Task created",
			},
		},
		WorkingDirectory: workdir,
		CreatedAt:        time.Now(),
		LastStepUpdate:   time.Now(),
	}

	// Add the task to the map
	t.tasks.Set(video.Id, &task)

	return &task, nil
}

func (t *TaskManager) Get(videoId VideoId) (*Task, bool) {
	return t.tasks.Get(videoId)
}

func (t *TaskManager) GetOneByStep(step Step) *Task {
	for task := range t.tasks.Iter() {
		if task.Step == step && !task.consumed.Get() {
			task.consumed.Set(true)
			return task
		}
	}

	return nil
}

func (t *TaskManager) Subscribe(step Step) <-chan *Task {
	ch := make(chan *Task)

	go func() {
		for {
			task := t.GetOneByStep(step)
			if task != nil {
				ch <- task
			}
		}
	}()

	return ch
}

func (t *TaskManager) GetAll() []Task {
	tasks := make([]Task, 0, t.tasks.Len())
	for task := range t.tasks.Iter() {
		tasks = append(tasks, *task)
	}

	return tasks
}

func (t *TaskManager) LogEvent(videoId VideoId, message string) error {
	task, ok := t.tasks.Get(videoId)
	if !ok {
		return ErrTaskNotFound
	}

	t.logger.Debugf("(%s) %s\n", videoId, message)
	task.Logs = append(task.Logs, LogEntry{
		Time:    time.Now(),
		Message: message,
	})

	t.tasks.Set(videoId, task)

	return nil
}

func IsDirectoryEmpty(name string) (bool, error) {
	f, err := os.Open(name)
	if err != nil {
		return false, err
	}
	defer f.Close()

	_, err = f.Readdirnames(1) // Or f.Readdir(1)
	if err == io.EOF {
		return true, nil
	}
	return false, err // Either not empty or error, suits both cases
}

func (t *TaskManager) UpdateStep(videoId VideoId, step Step) error {
	task, ok := t.tasks.Get(videoId)
	if !ok {
		return ErrTaskNotFound
	}

	if task.Step == step {
		return nil
	}

	task.Step = step
	logMessage := "Task state changed to " + string(step)
	task.Logs = append(task.Logs, LogEntry{
		Time:    time.Now(),
		Message: logMessage,
	})
	task.LastStepUpdate = time.Now()
	task.consumed.Set(false)
	t.tasks.Set(videoId, task)
	t.logger.Debugf("(%s) %s\n", videoId, logMessage)

	// If the task is done, remove the working directory
	if step == StepDone {
		t.logger.Debugf("Removing temporary working directory %s for video %s\n", task.WorkingDirectory, videoId)
		if err := os.RemoveAll(task.WorkingDirectory); err != nil {
			t.logger.Errorf("Failed to remove temporary working directory %s for video %s: %w\n", task.WorkingDirectory, videoId, err)
		}
	}

	// If task is errored, remove the working directory only if the directory is
	// empty.
	if step == StepErrored {
		if empty, _ := IsDirectoryEmpty(task.WorkingDirectory); empty {
			t.logger.Debugf("Removing temporary working directory %s for video %s\n", task.WorkingDirectory, videoId)
			if err := os.RemoveAll(task.WorkingDirectory); err != nil {
				t.logger.Errorf("Failed to remove temporary working directory %s for video %s: %w\n", task.WorkingDirectory, videoId, err)
			}
		}
	}

	return nil
}

func (t *TaskManager) UpdateProgress(videoId VideoId, progress string) error {
	task, ok := t.tasks.Get(videoId)
	if !ok {
		return ErrTaskNotFound
	}

	task.Progress = progress
	t.tasks.Set(videoId, task)

	return nil
}

func (t *TaskManager) PrintTable() {
	tbl := table.NewWriter()
	tbl.SetOutputMirror(os.Stdout)
	tbl.AppendHeader(table.Row{"Video Id", "Channel", "Title", "Status", "Progress"})
	tbl.SortBy([]table.SortBy{
		{Name: "Status", Mode: table.Asc},
		{Name: "Video Id", Mode: table.Asc},
	})

	for task := range t.tasks.Iter() {
		tbl.AppendRow(table.Row{
			task.Video.Id,
			fmt.Sprintf("%.10s", task.Video.ChannelName),
			fmt.Sprintf("%.30s", task.Video.Title),
			task.Step,
			task.Progress,
		})
	}

	tbl.SetStyle(table.StyleColoredDark)
	tbl.Render()
}

func (t *TaskManager) ClearOldTasks() {
	for task := range t.tasks.Iter() {
		if (task.Step == StepDone || task.Step == StepErrored) &&
			time.Since(task.LastStepUpdate) > time.Hour*24*7 {
			t.tasks.Delete(task.Video.Id)
		}
	}
}

// GetTaskByIndex returns the task at the given index, sorted by the time the
// task was created.
func (t *TaskManager) GetTaskByIndex(index int) (*Task, error) {
	if index < 0 || index >= t.tasks.Len() {
		return nil, ErrTaskNotFound
	}

	for task := range t.tasks.Iter() {
		if index == 0 {
			return task, nil
		}
		index--
	}

	return nil, ErrTaskNotFound
}
