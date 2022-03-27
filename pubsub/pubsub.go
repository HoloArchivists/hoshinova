package pubsub

import "sync"

// PubSub is a publish/subscribe messaging system.
type PubSub[T any] interface {
	// Publish publishes the data to the given topic.
	Publish(topic string, data T) error
	// Subscribe returns a channel which will yield messages that match the given
	// list of topics.
	Subscribe(topics ...string) (chan T, error)
}

type pubsub[T any] struct {
	subscriptions map[string][]chan T
	mu            sync.RWMutex
	bufferSize    int
}

func New[T any](bufferSize int) PubSub[T] {
	return &pubsub[T]{
		subscriptions: make(map[string][]chan T),
		bufferSize:    bufferSize,
	}
}

func (ps *pubsub[T]) initTopic(topic string) {
	ps.mu.Lock()
	defer ps.mu.Unlock()
	if _, ok := ps.subscriptions[topic]; !ok {
		ps.subscriptions[topic] = make([]chan T, 0)
	}
}

func (ps *pubsub[T]) Publish(topic string, data T) error {
	ps.initTopic(topic)

	ps.mu.RLock()
	defer ps.mu.RUnlock()
	for _, ch := range ps.subscriptions[topic] {
		ch <- data
	}

	return nil
}

func (ps *pubsub[T]) Subscribe(topics ...string) (chan T, error) {
	ch := make(chan T, ps.bufferSize)

	ps.mu.Lock()
	defer ps.mu.Unlock()

	for _, topic := range topics {
		ps.initTopic(topic)
		ps.subscriptions[topic] = append(ps.subscriptions[topic], ch)
	}

	return ch, nil
}
