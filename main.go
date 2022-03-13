package main

import (
	"context"

	"github.com/hizkifw/hoshinova/config"
	"github.com/hizkifw/hoshinova/watcher"
)

func main() {
	cfg, err := config.LoadConfig()
	if err != nil {
		panic(err)
	}

	ctx := context.Background()
	watcher.Watch(cfg, ctx)
}
