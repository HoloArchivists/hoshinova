package recorder

import (
	"context"
	"fmt"
	"os/exec"
	"syscall"
	"time"

	"github.com/hizkifw/hoshinova/taskman"
	"github.com/hizkifw/hoshinova/util"
)

func RecordVideo(ctx context.Context, task *taskman.Task) (*Recording, error) {
	tm := util.GetTaskManager(ctx)
	lg := util.GetLogger(ctx)
	cfg := util.GetConfig(ctx)
	tm.LogEvent(task.Video.Id, "starting ytarchive")

	// Set up the command.
	args := []string{"--wait", "--merge"}
	args = append(args, cfg.YTArchive.Flags...)
	args = append(
		args,
		"https://www.youtube.com/watch?v="+string(task.Video.Id),
		cfg.YTArchive.Quality,
	)

	cmd := exec.CommandContext(ctx, cfg.YTArchive.Path, args...)
	cmd.Dir = task.WorkingDirectory

	lg.Debugf("(%s) starting ytarchive with command %#v\n", task.Video.Id, cmd.Args)

	// Start the process in a separate process group. This prevents signals from
	// being sent to the child processes.
	cmd.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}

	// Track the status of the YTArchive process
	yta := NewYTA()
	db := util.NewDebounce(time.Second)

	// The callback writer will receive the output from the command and parse it.
	cw := util.NewCallbackWriter(func(line string) {
		yta.ParseLine(line)

		switch yta.State {
		case YTAStateWaiting:
			tm.UpdateStep(task.Video.Id, taskman.StepWaitingForLive)
		case YTAStateRecording:
			if !db.Check() {
				return
			}
			tm.UpdateStep(task.Video.Id, taskman.StepRecording)
			tm.UpdateProgress(task.Video.Id, yta.TotalSize)
		case YTAStateMuxing:
			tm.UpdateStep(task.Video.Id, taskman.StepMuxing)
		case YTAStateError:
			tm.UpdateStep(task.Video.Id, taskman.StepErrored)
		case YTAStateInterrupted:
			tm.UpdateStep(task.Video.Id, taskman.StepCancelled)
		case YTAStateFinished:
			tm.UpdateStep(task.Video.Id, taskman.StepIdle)
		}
	})

	// Pipe the output of the command to the callback writer.
	cmd.Stdout = cw
	cmd.Stderr = cw

	// Start the command
	if err := cmd.Start(); err != nil {
		lg.Errorf("(%s) ytarchive failed to start: %w\n", task.Video.Id, err)
		return nil, err
	}

	lg.Debugf("(%s) ytarchive started\n", task.Video.Id)

	// Wait for the command to exit
	if err := cmd.Wait(); err != nil {
		lg.Errorf("(%s) ytarchive failed: %w\n", task.Video.Id, err)
		return nil, fmt.Errorf("ytarchive failed: %w", err)
	}

	lg.Infof("(%s) ytarchive finished\n", task.Video.Id)
	return &Recording{
		VideoID:  task.Video.Id,
		FilePath: yta.OutputFile,
	}, nil
}
