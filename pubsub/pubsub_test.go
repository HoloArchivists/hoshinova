package pubsub_test

import (
	"testing"

	"github.com/HoloArchivists/hoshinova/pubsub"
	"github.com/stretchr/testify/assert"
)

func TestPubSub(t *testing.T) {
	assert := assert.New(t)

	// Create a new pubsub
	ps := pubsub.New[string](10)
	assert.NotNil(ps)

	// Subscribe to a topic
	sub, err := ps.Subscribe("foo")
	assert.Nil(err)
	assert.NotNil(sub)

	// Publish a message
	err = ps.Publish("foo", "bar")
	assert.Nil(err)

	// Get the message
	msg := <-sub
	assert.Equal("bar", msg)
}
