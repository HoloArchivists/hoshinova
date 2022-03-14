package taskman

import (
	"errors"
	"fmt"
	"os"
	"sync"
	"time"

	"github.com/hizkifw/hoshinova/config"
	"github.com/hizkifw/hoshinova/logger"
	"github.com/jedib0t/go-pretty/v6/table"
)

type Step string
type VideoId string

const (
	StepIdle           Step = "idle"
	StepWaitingForLive Step = "waiting for live"
	StepRecording      Step = "recording"
	StepMuxing         Step = "muxing"
	StepUploading      Step = "uploading"
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

	LastStepUpdate   time.Time
	WorkingDirectory string
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
	tasks  map[VideoId]Task
	lock   sync.RWMutex
	config *config.Config
	logger logger.Logger
}

func New(config *config.Config, logger logger.Logger) *TaskManager {
	return &TaskManager{
		tasks:  make(map[VideoId]Task),
		config: config,
		logger: logger,
	}
}

func (t *TaskManager) Insert(video Video) (*Task, error) {
	if _, ok := t.tasks[video.Id]; ok {
		return nil, ErrTaskAlreadyExists
	}

	// Create a temporary working directory
	workdir, err := os.MkdirTemp("", "hoshinova")
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
	}

	t.lock.Lock()
	t.tasks[video.Id] = task
	t.lock.Unlock()

	return &task, nil
}

func (t *TaskManager) Get(videoId VideoId) (*Task, error) {
	t.lock.RLock()
	defer t.lock.RUnlock()

	task, ok := t.tasks[videoId]
	if !ok {
		return nil, ErrTaskNotFound
	}

	return &task, nil
}

func (t *TaskManager) LogEvent(videoId VideoId, message string) error {
	t.lock.Lock()
	defer t.lock.Unlock()

	task, ok := t.tasks[videoId]
	if !ok {
		return ErrTaskNotFound
	}

	task.Logs = append(task.Logs, LogEntry{
		Time:    time.Now(),
		Message: message,
	})

	t.tasks[videoId] = task

	return nil
}

func (t *TaskManager) UpdateStep(videoId VideoId, step Step) error {
	t.lock.Lock()
	defer t.lock.Unlock()

	task, ok := t.tasks[videoId]
	if !ok {
		return ErrTaskNotFound
	}

	if task.Step == step {
		return nil
	}

	task.Step = step
	task.Logs = append(task.Logs, LogEntry{
		Time:    time.Now(),
		Message: "Task state changed to " + string(step),
	})
	task.LastStepUpdate = time.Now()
	t.tasks[videoId] = task

	// If the task is done, remove the working directory
	if step == StepDone {
		t.logger.Debug("Removing temporary working directory %s for video %s\n", task.WorkingDirectory, videoId)
		os.RemoveAll(task.WorkingDirectory)
	}

	return nil
}

func (t *TaskManager) UpdateProgress(videoId VideoId, progress string) error {
	t.lock.Lock()
	defer t.lock.Unlock()

	task, ok := t.tasks[videoId]
	if !ok {
		return ErrTaskNotFound
	}

	task.Progress = progress
	t.tasks[videoId] = task

	return nil
}

func (t *TaskManager) PrintTable() {
	t.lock.RLock()
	defer t.lock.RUnlock()

	tbl := table.NewWriter()
	tbl.SetOutputMirror(os.Stdout)
	tbl.AppendHeader(table.Row{"Video Id", "Channel", "Title", "Status", "Progress"})
	tbl.SortBy([]table.SortBy{
		{Name: "Status", Mode: table.Asc},
		{Name: "Video Id", Mode: table.Asc},
	})

	for _, task := range t.tasks {
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
	t.lock.Lock()
	defer t.lock.Unlock()

	for videoId, task := range t.tasks {
		if (task.Step == StepDone || task.Step == StepErrored) &&
			time.Since(task.LastStepUpdate) > time.Hour*24*7 {
			delete(t.tasks, videoId)
		}
	}
}
