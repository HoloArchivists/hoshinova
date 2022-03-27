package taskman

import (
	"sync"

	orderedmap "github.com/wk8/go-ordered-map"
)

type TaskMap struct {
	tasks *orderedmap.OrderedMap
	lock  sync.RWMutex
}

func NewTaskMap() *TaskMap {
	return &TaskMap{
		tasks: orderedmap.New(),
	}
}

func (m *TaskMap) Len() int {
	m.lock.RLock()
	defer m.lock.RUnlock()

	return m.tasks.Len()
}

func (m *TaskMap) Get(key VideoId) (*Task, bool) {
	m.lock.RLock()
	defer m.lock.RUnlock()

	t, ok := m.tasks.Get(key)
	if !ok {
		return nil, false
	}
	return t.(*Task), true
}

func (m *TaskMap) Set(key VideoId, value *Task) {
	m.lock.Lock()
	defer m.lock.Unlock()

	m.tasks.Set(key, value)
}

func (m *TaskMap) Delete(key VideoId) {
	m.lock.Lock()
	defer m.lock.Unlock()

	m.tasks.Delete(key)
}

func (m *TaskMap) Iter() chan *Task {
	ch := make(chan *Task)
	go func() {
		m.lock.RLock()
		defer m.lock.RUnlock()

		for pair := m.tasks.Oldest(); pair != nil; pair = pair.Next() {
			ch <- pair.Value.(*Task)
		}
		close(ch)
	}()
	return ch
}
