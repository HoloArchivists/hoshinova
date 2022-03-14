package util

import (
	"context"
	"time"

	"github.com/hizkifw/hoshinova/config"
	"github.com/hizkifw/hoshinova/logger"
	"github.com/hizkifw/hoshinova/taskman"
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

func WithTaskManager(ctx context.Context, taskman *taskman.TaskManager) context.Context {
	return context.WithValue(ctx, taskmanKey, taskman)
}

func GetTaskManager(ctx context.Context) *taskman.TaskManager {
	return ctx.Value(taskmanKey).(*taskman.TaskManager)
}

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

// RunLoopContext runs the given function in a loop until the context is canceled.
func RunLoopContext(ctx context.Context, f func()) error {
	for {
		select {
		case <-ctx.Done():
			return ctx.Err()
		default:
			f()
		}
	}
}
