package main

import (
	"context"
	"fmt"
	"os"
	"os/signal"
	"sync"

	"github.com/hizkifw/hoshinova/config"
	"github.com/hizkifw/hoshinova/notifier"
	"github.com/hizkifw/hoshinova/recorder"
	"github.com/hizkifw/hoshinova/taskman"
	"github.com/hizkifw/hoshinova/uploader"
	"github.com/hizkifw/hoshinova/util"
	"github.com/hizkifw/hoshinova/watcher"
)

func main() {
	cfg, err := config.LoadConfig()
	if err != nil {
		panic(err)
	}

	ctx, cancel := context.WithCancel(context.Background())

	tm := taskman.New()
	wg := sync.WaitGroup{}
	ctx = util.WithWaitGroup(ctx, &wg)
	ctx = util.WithConfig(ctx, cfg)
	ctx = util.WithTaskManager(ctx, tm)

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

	go watcher.Watch(ctx, func(task *taskman.Task) {
		rec, err := recorder.Record(ctx, task)
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

	// go func() {
	// for {
	// time.Sleep(time.Second)
	// tm.PrintTable()
	// }
	// }()

	// Handle interrupt
	c := make(chan os.Signal, 1)
	signal.Notify(c, os.Interrupt)
	<-c
	cancel()

	fmt.Println("Waiting for all goroutines to finish...")
	fmt.Println("Press Ctrl+C again to force exit")
	go func() {
		<-c
		os.Exit(1)
	}()
	wg.Wait()
}
