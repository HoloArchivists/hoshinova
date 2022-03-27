package util

import (
	"context"
	"time"

	"github.com/HoloArchivists/hoshinova/config"
	"github.com/HoloArchivists/hoshinova/logger"
	// "github.com/HoloArchivists/hoshinova/taskman"
)

const (
	waitGroupKey = "waitGroup"
	configKey    = "config"
	taskmanKey   = "taskman"
	loggerKey    = "logger"
)

func WithConfig(ctx context.Context, config *config.Config) context.Context {
	return context.WithValue(ctx, configKey, config)
}

func GetConfig(ctx context.Context) *config.Config {
	return ctx.Value(configKey).(*config.Config)
}

// func WithTaskManager(ctx context.Context, taskman *taskman.TaskManager) context.Context {
// return context.WithValue(ctx, taskmanKey, taskman)
// }
//
// func GetTaskManager(ctx context.Context) *taskman.TaskManager {
// return ctx.Value(taskmanKey).(*taskman.TaskManager)
// }

func WithLogger(ctx context.Context, logger logger.Logger) context.Context {
	return context.WithValue(ctx, loggerKey, logger)
}

func GetLogger(ctx context.Context) logger.Logger {
	return ctx.Value(loggerKey).(logger.Logger)
}

// SleepContext sleeps for the given duration or until the context is canceled.
func SleepContext(ctx context.Context, d time.Duration) error {
	select {
	case <-time.After(d):
		return nil
	case <-ctx.Done():
		return ctx.Err()
	}
}

// LoopUntilCancelled runs the given function in a loop until the context is
// canceled. If the supplied function returns an error, the loop will be
// terminated and the error returned.
func LoopUntilCancelled(ctx context.Context, f func() error) error {
	for {
		select {
		case <-ctx.Done():
			return ctx.Err()
		default:
			if err := f(); err != nil {
				return err
			}
		}
	}
}
