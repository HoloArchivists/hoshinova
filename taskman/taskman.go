package taskman

import (
	"errors"
	"fmt"
	"os"
	"sync"
	"time"

	"github.com/gdamore/tcell/v2"
	"github.com/hizkifw/hoshinova/config"
	"github.com/hizkifw/hoshinova/logger"
	"github.com/jedib0t/go-pretty/v6/table"
	"github.com/rivo/tview"
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

	CreatedAt        time.Time
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
	tasks  *TaskMap
	lock   sync.RWMutex
	config *config.Config
	logger logger.Logger

	// TableContentReadOnly implements noop write methods to tview.TableContent,
	// so we only have to implement the read methods.
	tview.TableContentReadOnly
}

// TaskManager should implement tview.TableContent
var _ tview.TableContent = (*TaskManager)(nil)

func New(config *config.Config, logger logger.Logger) *TaskManager {
	return &TaskManager{
		tasks:  NewTaskMap(),
		config: config,
		logger: logger,
	}
}

func (t *TaskManager) Insert(video Video) (*Task, error) {
	t.lock.Lock()
	defer t.lock.Unlock()

	// Check if the task already exists
	if _, ok := t.tasks.Get(video.Id); ok {
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
		CreatedAt:        time.Now(),
		LastStepUpdate:   time.Now(),
	}

	// Add the task to the map
	t.tasks.Set(video.Id, &task)

	return &task, nil
}

func (t *TaskManager) Get(videoId VideoId) (*Task, error) {
	t.lock.RLock()
	defer t.lock.RUnlock()
	return t.Get(videoId)
}

func (t *TaskManager) GetAll() []Task {
	t.lock.RLock()
	defer t.lock.RUnlock()

	tasks := make([]Task, 0, t.tasks.Len())
	for task := range t.tasks.Iter() {
		tasks = append(tasks, *task)
	}

	return tasks
}

func (t *TaskManager) LogEvent(videoId VideoId, message string) error {
	t.lock.Lock()
	defer t.lock.Unlock()

	task, ok := t.tasks.Get(videoId)
	if !ok {
		return ErrTaskNotFound
	}

	task.Logs = append(task.Logs, LogEntry{
		Time:    time.Now(),
		Message: message,
	})

	t.tasks.Set(videoId, task)

	return nil
}

func (t *TaskManager) UpdateStep(videoId VideoId, step Step) error {
	t.lock.Lock()
	defer t.lock.Unlock()

	task, ok := t.tasks.Get(videoId)
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
	t.tasks.Set(videoId, task)

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

	task, ok := t.tasks.Get(videoId)
	if !ok {
		return ErrTaskNotFound
	}

	task.Progress = progress
	t.tasks.Set(videoId, task)

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
	t.lock.Lock()
	defer t.lock.Unlock()

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
	t.lock.RLock()
	defer t.lock.RUnlock()

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

// GetCell returns the content of the cell at the given position. This method
// is used by tview to render the table.
func (t *TaskManager) GetCell(row, col int) *tview.TableCell {
	t.lock.RLock()
	defer t.lock.RUnlock()

	headers := []string{"Video Id", "Channel", "Title", "Status", "Progress"}
	if row == 0 {
		return tview.
			NewTableCell(headers[col]).
			SetBackgroundColor(tcell.ColorPurple).
			SetTextColor(tcell.ColorWhite).
			SetAttributes(tcell.AttrBold).
			SetSelectable(false)
	}

	task, err := t.GetTaskByIndex(row - 1)
	if err != nil {
		return tview.NewTableCell("")
	}

	switch col {
	case 0:
		return tview.NewTableCell(string(task.Video.Id))
	case 1:
		return tview.NewTableCell(fmt.Sprintf("%.10s", task.Video.ChannelName))
	case 2:
		return tview.NewTableCell(fmt.Sprintf("%.30s", task.Video.Title))
	case 3:
		return tview.NewTableCell(string(task.Step))
	case 4:
		return tview.NewTableCell(task.Progress)
	}

	return tview.NewTableCell("")
}

// GetColumnCount returns the number of columns in the table.
func (t *TaskManager) GetColumnCount() int {
	return 5
}

// GetRowCount returns the number of rows in the table.
func (t *TaskManager) GetRowCount() int {
	t.lock.RLock()
	defer t.lock.RUnlock()

	// +1 for the header
	return t.tasks.Len() + 1
}
