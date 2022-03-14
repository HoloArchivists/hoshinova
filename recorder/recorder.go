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
	cw := NewCallbackWriter(func(line string) {
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
			tm.UpdateStep(task.Video.Id, taskman.StepDone)
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
			lg.Debug("ytarchive context cancelled, sending interrupt signal")
			cmd.Process.Signal(os.Interrupt)
		case <-finished:
		}
	}()

	// Wait for the command to exit
	code := waitForExitCode(cmd)
	finished <- true
	if code != 0 {
		lg.Error("ytarchive failed", "exit_code", code)
		return nil, fmt.Errorf("ytarchive failed with exit code %d", code)
	}

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

				// Exit code -1 means the process received a signal. We only want to
				// return if the process has exited.
				if code != -1 {
					return code
				}
			}
		}
	}
}
