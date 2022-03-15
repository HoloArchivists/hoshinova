package tui

import (
	"context"
	"time"

	"github.com/hizkifw/hoshinova/config"
	"github.com/hizkifw/hoshinova/logger"
	"github.com/hizkifw/hoshinova/taskman"
	"github.com/hizkifw/hoshinova/util"

	"github.com/gdamore/tcell/v2"
	"github.com/rivo/tview"
)

type Tui struct {
	app    *tview.Application
	cfg    *config.Config
	tm     *taskman.TaskManager
	cancel context.CancelFunc
}

func New(ctx context.Context, cancel context.CancelFunc) *Tui {
	cfg := util.GetConfig(ctx)
	tm := util.GetTaskManager(ctx)

	// Create a new tview application.
	app := tview.NewApplication()
	table := tview.NewTable().
		SetFixed(1, 1).
		SetSelectable(true, false).
		SetSelectedStyle(
			tcell.StyleDefault.Bold(true),
		).
		SetContent(tm).
		SetDoneFunc(func(key tcell.Key) {
			if key == tcell.KeyCtrlC {
				app.Stop()
				cancel()
			}
		})
	app.SetRoot(table, true).SetFocus(table)

	// Force a redraw every second.
	go func() {
		for {
			select {
			case <-ctx.Done():
				return
			case <-time.After(time.Second):
				app.Draw()
			}
		}
	}()

	return &Tui{app, cfg, tm, cancel}
}

func (t *Tui) Run(ctx context.Context) error {
	lg := util.GetLogger(ctx)
	lastLogLevel := lg.GetLogLevel()
	lg.SetLogLevel(logger.LogLevelFatal)
	defer lg.SetLogLevel(lastLogLevel)
	defer t.cancel()

	return t.app.Run()
}
