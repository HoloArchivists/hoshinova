package recorder

import (
	"context"
	"fmt"
	"os"
	"os/exec"

	"github.com/hizkifw/hoshinova/taskman"
	"github.com/hizkifw/hoshinova/util"
)

type Recording struct {
	VideoID  taskman.VideoId
	FilePath string
}

func Record(ctx context.Context, task *taskman.Task) (*Recording, error) {
	tm := util.GetTaskManager(ctx)
	tm.LogEvent(task.Video.Id, "starting ytarchive")
	lg := util.GetLogger(ctx)
	lg.Debug("starting ytarchive", "video_id", task.Video.Id)

	// Set up the command.
	url := "https://www.youtube.com/watch?v=" + string(task.Video.Id)
	cmd := exec.Command(
		"ytarchive",
		"--wait", "--vp9", "--merge",
		"--thumbnail", "--add-metadata",
		"--threads", "8",
		"--output", "%(id)s",
		url, "best",
	)
	cmd.Dir = task.WorkingDirectory

	// The callback writer will receive the output from the command and parse it.
	yta := NewYTA()
	cw := util.NewCallbackWriter(func(line string) {
		yta.ParseLine(line)
		tm.UpdateProgress(task.Video.Id, yta.TotalSize)

		switch yta.State {
		case YTAStateWaiting:
			tm.UpdateStep(task.Video.Id, taskman.StepWaitingForLive)
		case YTAStateRecording:
			tm.UpdateStep(task.Video.Id, taskman.StepRecording)
		case YTAStateMuxing:
			tm.UpdateStep(task.Video.Id, taskman.StepMuxing)
		case YTAStateFinished:
			// Set to idle instead of done. Only set to done after the file has been
			// uploaded and the notification has been sent.
			tm.UpdateStep(task.Video.Id, taskman.StepIdle)
		case YTAStateError:
			tm.UpdateStep(task.Video.Id, taskman.StepErrored)
		case YTAStateInterrupted:
			tm.UpdateStep(task.Video.Id, taskman.StepCancelled)
		}
	})

	// Pipe the output of the command to the callback writer.
	cmd.Stdout = cw
	cmd.Stderr = cw

	// Start the command
	if err := cmd.Start(); err != nil {
		lg.Error("ytarchive failed to start", "error", err)
		return nil, err
	}

	// Set up a goroutine to send an interrupt signal to the process if the
	// context is cancelled.
	finished := make(chan bool, 1)
	go func() {
		select {
		case <-ctx.Done():
			lg.Debug("Interrupting ytarchive for", "video_id", task.Video.Id)
			cmd.Process.Signal(os.Interrupt)
		case <-finished:
		}
	}()

	// Wait for the command to exit
	code := waitForExitCode(cmd)
	finished <- true
	if code != 0 {
		lg.Error("ytarchive exited with", code, "for", task.Video.Id)
		return nil, fmt.Errorf("ytarchive failed with exit code %d", code)
	}

	lg.Infof("Finished ytarchive for %s", task.Video.Id)
	return &Recording{
		VideoID:  task.Video.Id,
		FilePath: yta.OutputFile,
	}, nil
}

// waitForExitCode waits for the command to exit and returns the exit code.
func waitForExitCode(cmd *exec.Cmd) int {
	for {
		if err := cmd.Wait(); err != nil {
			if exitErr, ok := err.(*exec.ExitError); ok {
				code := exitErr.ExitCode()

				if err.Error() == "signal: interrupt" {
					continue
				}
				return code
			}
		}
	}
}
