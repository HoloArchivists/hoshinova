package recorder

import (
	"context"
	"fmt"
	"os/exec"
	"syscall"
	"time"

	"github.com/HoloArchivists/hoshinova/config"
	"github.com/HoloArchivists/hoshinova/task"
	"github.com/HoloArchivists/hoshinova/util"
)

type YTArchive struct {
	cfgRecorder  *config.Recorder
	cfgYTArchive YTArchiveConfig
}

type YTArchiveConfig struct {
	// ExecutablePath is the path to the ytarchive executable.
	ExecutablePath string `yaml:"executable_path"`
	// Quality is the video quality to use.
	Quality string `yaml:"quality"`
	// Flags are additional flags to pass to ytarchive.
	Flags []string `yaml:"flags"`
}

func NewYTArchive(cfgRecorder *config.Recorder) (Recorder, error) {
	var configYTArchive YTArchiveConfig
	if err := cfgRecorder.Config.Unmarshal(&configYTArchive); err != nil {
		return nil, fmt.Errorf("failed to parse ytarchive config: %w", err)
	}

	return &YTArchive{
		cfgRecorder:  cfgRecorder,
		cfgYTArchive: configYTArchive,
	}, nil
}

func (y *YTArchive) Start(ctx context.Context, ps task.PubSub) error {
	return util.LoopUntilCancelled(ctx, func() error {
		return nil
	})
}

func (y *YTArchive) recordVideo(ctx context.Context, task *task.Task) error {
	lg := util.GetLogger(ctx)

	// Set up the command.
	args := []string{"--wait", "--merge"}
	args = append(args, y.cfgYTArchive.Flags...)
	args = append(
		args,
		"https://www.youtube.com/watch?v="+string(task.VideoID),
		y.cfgYTArchive.Quality,
	)

	cmd := exec.CommandContext(ctx, y.cfgYTArchive.ExecutablePath, args...)
	cmd.Dir = task.WorkDir

	lg.Debugf("(%s) starting ytarchive with command %#v\n", task.VideoID, cmd.Args)

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
			lg.Debugf("(%s) waiting for live\n", task.VideoID)
		case YTAStateRecording:
			if !db.Check() {
				return
			}
		case YTAStateMuxing:
			lg.Debugf("(%s) muxing\n", task.VideoID)
		case YTAStateError:
			lg.Errorf("(%s) errored\n", task.VideoID)
		case YTAStateInterrupted:
			lg.Debugf("(%s) interrupted\n", task.VideoID)
		case YTAStateFinished:
			lg.Debugf("(%s) finished recording\n", task.VideoID)
		}
	})

	// Pipe the output of the command to the callback writer.
	cmd.Stdout = cw
	cmd.Stderr = cw

	// Start the command
	if err := cmd.Start(); err != nil {
		lg.Errorf("(%s) ytarchive failed to start: %w\n", task.VideoID, err)
		return err
	}

	lg.Debugf("(%s) ytarchive started\n", task.VideoID)

	// Wait for the command to exit
	if err := cmd.Wait(); err != nil {
		lg.Errorf("(%s) ytarchive failed: %w\n", task.VideoID, err)
		return fmt.Errorf("ytarchive failed: %w", err)
	}

	lg.Infof("(%s) ytarchive finished\n", task.VideoID)

	return nil
}
