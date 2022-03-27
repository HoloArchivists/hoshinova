package main

import (
	"context"
	"fmt"
	"os"
	"os/signal"
	"time"

	"github.com/HoloArchivists/hoshinova/config"
	"github.com/HoloArchivists/hoshinova/logger"
	"github.com/HoloArchivists/hoshinova/module/notifier"
	"github.com/HoloArchivists/hoshinova/module/scraper"
	"github.com/HoloArchivists/hoshinova/module/uploader"
	"github.com/HoloArchivists/hoshinova/util"
	"github.com/HoloArchivists/hoshinova/watcher"
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

	var scrapers []scraper.Scraper
	var uploaders []uploader.Uploader
	var notifiers []notifier.Notifier

	// Create scrapers
	for _, s := range cfg.Scrapers {
		scr, err := scraper.New(&s)
		if err != nil {
			panic(err)
		}
		scrapers = append(scrapers, scr)
	}

	// Create uploaders
	for _, u := range cfg.Uploaders {
		upl, err := uploader.NewUploader(u)
		if err != nil {
			panic(err)
		}
		uploaders = append(uploaders, upl)
	}

	// Create notifiers
	for _, n := range cfg.Notifiers {
		not, err := notifier.NewNotifier(n)
		if err != nil {
			panic(err)
		}
		notifiers = append(notifiers, not)
	}

	// Start watching the channels for new videos
	wg := watcher.Watch(ctx)

	// Handle interrupt
	c := make(chan os.Signal, 1)
	signal.Notify(c, os.Interrupt)

	// Print table when the program is interrupted
	lg.Info("Application started. Press Ctrl+C once to print the status, twice to exit.")
	func() {
		for {
			select {
			case <-ctx.Done():
				return

			case <-c:
				fmt.Println("")
				tm.PrintTable()
				lg.Info("Press Ctrl+C again to exit")

				select {
				case <-c:
					return
				case <-time.After(time.Second):
				}
			}
		}
	}()
	lg.Debug("Cancelling the context")
	cancel()

	lg.Info("Waiting for all goroutines to finish...")
	lg.Info("Press Ctrl+C again to force exit")
	go func() {
		<-c
		lg.Info("Force exiting")
		os.Exit(1)
	}()
	wg.Wait()
}
