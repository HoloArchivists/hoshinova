package main

import (
	"context"
	"fmt"
	"os"
	"os/signal"

	"github.com/hizkifw/hoshinova/config"
	"github.com/hizkifw/hoshinova/notifier"
	"github.com/hizkifw/hoshinova/recorder"
	"github.com/hizkifw/hoshinova/uploader"
	"github.com/hizkifw/hoshinova/watcher"
)

func main() {
	cfg, err := config.LoadConfig()
	if err != nil {
		panic(err)
	}

	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	var uploaders []uploader.Uploader
	var notifiers []notifier.Notifier

	for _, u := range cfg.Uploaders {
		upl, err := uploader.NewUploader(u)
		if err != nil {
			panic(err)
		}
		uploaders = append(uploaders, upl)
	}

	for _, n := range cfg.Notifiers {
		not, err := notifier.NewNotifier(n)
		if err != nil {
			panic(err)
		}
		notifiers = append(notifiers, not)
	}

	go watcher.Watch(cfg, ctx, func(entry watcher.PollEntry) {
		rec, err := recorder.Record(ctx, &entry)
		if err != nil {
			fmt.Println("Error recording:", err)
			return
		}

		for _, upl := range uploaders {
			res, err := upl.Upload(ctx, rec)
			if err != nil {
				fmt.Println("Error uploading:", err)
				return
			}

			for _, not := range notifiers {
				not.NotifyUploaded(ctx, res)
			}
		}
	})

	// Handle interrupt
	c := make(chan os.Signal, 1)
	signal.Notify(c, os.Interrupt)
	<-c
}
