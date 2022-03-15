package tui

import (
	"context"
	"time"

	"github.com/hizkifw/hoshinova/util"

	"github.com/gdamore/tcell/v2"
	"github.com/rivo/tview"
)

type Tui struct {
	app *tview.Application
}

func New() *Tui {
	app := tview.NewApplication()
	return &Tui{app}
}

func (t *Tui) Run(ctx context.Context, cancel context.CancelFunc) error {
	tm := util.GetTaskManager(ctx)

	// Table of tasks
	table := tview.NewTable().
		SetFixed(1, 1).
		SetSelectable(true, false).
		SetSelectedStyle(
			tcell.StyleDefault.Bold(true),
		).
		SetContent(tm).
		SetDoneFunc(func(key tcell.Key) {
			if key == tcell.KeyCtrlC {
				t.app.Stop()
				cancel()
			}
		})
	table.
		SetTitle("Tasks").
		SetTitleAlign(tview.AlignLeft).
		SetBorder(true)

	// TextView for logs
	logs := tview.NewTextView().
		SetDynamicColors(false).
		SetScrollable(true).
		SetWordWrap(true).
		SetChangedFunc(func() {
			t.app.Draw()
		})
	logs.
		SetTitle("Logs").
		SetTitleAlign(tview.AlignLeft).
		SetBorder(true)

	// Grid layout
	grid := tview.NewGrid().
		SetRows(0, 0).
		SetColumns(0).
		AddItem(table, 0, 0, 1, 1, 0, 0, true).
		AddItem(logs, 1, 0, 1, 1, 0, 0, false)

	t.app.SetRoot(grid, true).SetFocus(table)

	// Force a redraw every second.
	go func() {
		for {
			select {
			case <-ctx.Done():
				return
			case <-time.After(time.Second):
				t.app.Draw()
			}
		}
	}()
	defer cancel()

	// Redirect logs to the text view.
	lg := util.GetLogger(ctx)
	lastOutput := lg.GetOutput()
	defer lg.SetOutput(lastOutput)
	lg.SetOutput(logs)

	lg.Info("Starting TUI")
	return t.app.Run()
}
