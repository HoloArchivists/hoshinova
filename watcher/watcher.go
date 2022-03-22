package watcher

import (
	"context"
	"regexp"
	"sync"
	"time"

	"github.com/hizkifw/hoshinova/config"
	"github.com/hizkifw/hoshinova/scraper"
	"github.com/hizkifw/hoshinova/taskman"
	"github.com/hizkifw/hoshinova/util"
)

// Watch will create a goroutine for each channel in the configuration, where
// each goroutine will poll the channel every PollInterval seconds and insert
// a new task into the task manager if a new video is found that matches the
// configured filters.
//
// This function will start goroutines and immediately return with a WaitGroup.
func Watch(ctx context.Context) *sync.WaitGroup {
	wg := sync.WaitGroup{}
	cfg := util.GetConfig(ctx)
	lg := util.GetLogger(ctx)

	for _, c := range cfg.Channels {
		go func(channel config.Channel) {
			wg.Add(1)
			defer wg.Done()

			lg.Info("Watching channel:", channel.Name)
			defer lg.Info("Stopped watching channel:", channel.Name)

			watchChannel(ctx, &channel)
		}(c)
	}

	lg.Infof("Watching %d channels\n", len(cfg.Channels))
	return &wg
}

func watchChannel(ctx context.Context, channel *config.Channel) {
	var scrp scraper.Scraper = &scraper.RSS{}
	pollInterval := time.Duration(util.GetConfig(ctx).PollInterval) * time.Second
	lg := util.GetLogger(ctx)
	tm := util.GetTaskManager(ctx)

	util.LoopUntilCancelled(ctx, func() {
		lg.Debug("Polling channel:", channel.Name)
		videos, err := scrp.Scrape(ctx, channel.ChannelID)
		if err != nil {
			lg.Error("Error polling channel:", channel.Name, err)
			return
		}

		for _, video := range videos {
			for _, filter := range channel.Filters {
				filter := regexp.Regexp(filter)
				if filter.MatchString(video.Title) {
					lg.Debug("Video matched filter:", video.Title)
					tm.Insert(taskman.Video{
						Id:          taskman.VideoId(video.ID),
						Title:       video.Title,
						ChannelId:   channel.ChannelID,
						ChannelName: channel.Name,
					})
				}
			}
		}

		util.SleepContext(ctx, pollInterval)
	})
}
