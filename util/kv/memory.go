package kv

import (
	"context"
	"sync"
	"time"
)

// MemoryKV is a in-memory implementation of KV.
type MemoryKV[K string, V any] struct {
	data map[K]kvData[V]
	lock sync.RWMutex
}

type kvData[V any] struct {
	value  V
	expire time.Time
}

func NewMemoryKV[K string, V any]() KV[K, V] {
	return &MemoryKV[K, V]{
		data: make(map[K]kvData[V]),
	}
}

func (kv *MemoryKV[K, V]) Get(ctx context.Context, key K) (V, error) {
	kv.lock.RLock()
	defer kv.lock.RUnlock()
	if data, ok := kv.data[key]; ok {
		if data.expire.After(time.Now()) {
			return data.value, nil
		}
		delete(kv.data, key)
	}

	var zero V
	return zero, ErrNotFound
}

func (kv *MemoryKV[K, V]) Set(ctx context.Context, key K, value V, ttl time.Duration) error {
	kv.lock.Lock()
	defer kv.lock.Unlock()
	kv.data[key] = kvData[V]{
		value:  value,
		expire: time.Now().Add(ttl),
	}
	return nil
}

func (kv *MemoryKV[K, V]) Delete(ctx context.Context, key K) error {
	kv.lock.Lock()
	defer kv.lock.Unlock()
	delete(kv.data, key)
	return nil
}
