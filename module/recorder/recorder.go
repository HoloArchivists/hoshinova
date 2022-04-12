package recorder

import (
	"context"
	"fmt"

	"github.com/HoloArchivists/hoshinova/config"
	"github.com/HoloArchivists/hoshinova/task"
)

// Recorder is the interface for recording videos. It waits for tasks from the
// task queue and records matching ones.
type Recorder interface {
	// Start starts the recorder. It should block until the context is canceled,
	// and only return when a fatal error occurs.
	Start(ctx context.Context, ps task.PubSub) error
}

func New(r *config.Recorder) (Recorder, error) {
	switch r.Type {
	case "ytarchive":
		return NewYTArchive(r)
	default:
		return nil, fmt.Errorf("unknown recorder type: %s", r.Type)
	}
}
