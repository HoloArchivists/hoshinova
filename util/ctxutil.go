package util

import (
	"context"
	"sync"
	"time"

	"github.com/hizkifw/hoshinova/config"
	"github.com/hizkifw/hoshinova/taskman"
)

const (
	waitGroupKey = "waitGroup"
	configKey    = "config"
	taskmanKey   = "taskman"
)

func WithWaitGroup(ctx context.Context, wg *sync.WaitGroup) context.Context {
	return context.WithValue(ctx, waitGroupKey, wg)
}

func GetWaitGroup(ctx context.Context) *sync.WaitGroup {
	return ctx.Value(waitGroupKey).(*sync.WaitGroup)
}

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
