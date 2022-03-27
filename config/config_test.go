package config_test

import (
	"testing"

	"github.com/HoloArchivists/hoshinova/config"
	"github.com/stretchr/testify/assert"
)

func TestExampleConfig(t *testing.T) {
	assert := assert.New(t)

	// Load the example config
	cfg, err := config.LoadConfig("../config.example.yaml")
	assert.NoError(err)
	assert.NotNil(cfg)
}
