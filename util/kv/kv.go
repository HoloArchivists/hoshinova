package kv

import (
	"context"
	"errors"
	"time"
)

var (
	// ErrNotFound is returned when a key is not found in the store.
	ErrNotFound = errors.New("not found")
)

// KV is the interface for key-value stores. Implementations of this interface
// are expected to be thread-safe.
type KV[K any, V any] interface {
	Get(ctx context.Context, key K) (V, error)
	Set(ctx context.Context, key K, value V, ttl time.Duration) error
	Delete(ctx context.Context, key K) error
}
