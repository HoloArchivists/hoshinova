package recorder

import (
	"context"
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

	url := "https://www.youtube.com/watch?v=" + string(task.Video.Id)
	tempdir, err := os.MkdirTemp("", "rec")
	if err != nil {
		return nil, err
	}
	// Do not defer os.RemoveAll(tempdir) because we want to keep the recordings
	// in case of error.

	lg.Infof("Downloading %s to %s\n", task.Video.Id, tempdir)

	cmd := exec.CommandContext(
		ctx,
		"ytarchive",
		"--wait", "--vp9", "--merge",
		"--thumbnail", "--add-metadata",
		"--threads", "8",
		"--output", "%(id)s",
		url, "best",
	)
	cmd.Dir = tempdir

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
		}
	})
	cmd.Stdout = cw
	cmd.Stderr = cw

	if err := cmd.Start(); err != nil {
		lg.Error("ytarchive failed to start", "error", err)
		return nil, err
	}

	// Check return code
	if err := cmd.Wait(); err != nil {
		lg.Error("ytarchive exited with", "error", err)
		return nil, err
	}

	return &Recording{
		VideoID:  task.Video.Id,
		FilePath: tempdir,
	}, nil
}
