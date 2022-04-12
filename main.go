package main

import (
	"context"
	"os"
	"os/signal"

	"github.com/HoloArchivists/hoshinova/config"
	"github.com/HoloArchivists/hoshinova/logger"
	"github.com/HoloArchivists/hoshinova/module/scraper"
	"github.com/HoloArchivists/hoshinova/pubsub"
	"github.com/HoloArchivists/hoshinova/task"
	"github.com/HoloArchivists/hoshinova/util"
)

func main() {
	cfg, err := config.LoadConfig("config.yaml")
	if err != nil {
		panic(err)
	}

	lg := logger.New(cfg.App.LogLevel)

	lg.Info("Using log level", cfg.App.LogLevel)
	lg.Info("Starting hoshinova")

	// Create context
	ctx, cancel := context.WithCancel(context.Background())
	ctx = util.WithLogger(ctx, lg)
	ctx = util.WithConfig(ctx, cfg)

	ps := pubsub.New[task.Task](99)
	var scrapers []scraper.Scraper

	// Create scrapers
	for _, s := range cfg.Scrapers {
		scr, err := scraper.New(&s)
		if err != nil {
			panic(err)
		}
		scrapers = append(scrapers, scr)
	}

	// Start scrapers
	for _, scr := range scrapers {
		go func(scr scraper.Scraper) {
			scr.Start(ctx, ps)
		}(scr)
	}

	// Handle interrupt
	c := make(chan os.Signal, 1)
	signal.Notify(c, os.Interrupt)

	<-c
	cancel()

	lg.Info("Waiting for all goroutines to finish...")
	lg.Info("Press Ctrl+C again to force exit")
	go func() {
		<-c
		lg.Info("Force exiting")
		os.Exit(1)
	}()
}
