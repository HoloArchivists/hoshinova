package watcher

import (
	"context"
	"regexp"
	"time"

	"github.com/hizkifw/hoshinova/config"
	"github.com/hizkifw/hoshinova/rss"
	"github.com/hizkifw/hoshinova/taskman"
	"github.com/hizkifw/hoshinova/util"
)

// Watch will create a goroutine for each channel in the configuration
func Watch(ctx context.Context, callback func(*taskman.Task)) {
	wg := util.GetWaitGroup(ctx)
	cfg := util.GetConfig(ctx)
	lg := util.GetLogger(ctx)

	for _, c := range cfg.Channels {
		go func(channel config.Channel) {
			wg.Add(1)
			defer wg.Done()

			p := Poller{Channel: &channel}
			p.WatchChannel(ctx, callback)
		}(c)
	}

	lg.Infof("Watching %d channels\n", len(cfg.Channels))
}

type PollEntry struct {
	VideoID     string
	Title       string
	ChannelID   string
	ChannelName string
}

type Poller struct {
	Channel     *config.Channel
	lastVideoID string
}

// WatchChannel will poll a single channel and call the callback function
// with the Poll function returns a PollEntry
func (p *Poller) WatchChannel(ctx context.Context, callback func(*taskman.Task)) {
	util.RunLoopContext(ctx, func() {
		lg := util.GetLogger(ctx)

		latest, err := p.Poll()
		if err != nil {
			lg.Error("Error polling channel:", err)
		}

		if latest != nil {
			lg.Infof("New video (%s): %s\n", latest.VideoID, latest.Title)

			tm := util.GetTaskManager(ctx)
			task, err := tm.Insert(taskman.Video{
				Id:          taskman.VideoId(latest.VideoID),
				Title:       latest.Title,
				ChannelId:   p.Channel.ChannelID,
				ChannelName: p.Channel.Name,
			})

			if err != nil {
				lg.Error("Error creating new task:", err)
			} else {
				callback(task)
			}
		}

		cfg := util.GetConfig(ctx)
		util.SleepContext(ctx, time.Duration(cfg.PollInterval)*time.Second)
	})
}

// Poll will poll the channel and return the latest video that matches the
// configured filters
func (p *Poller) Poll() (*rss.Entry, error) {
	entries, err := rss.Poll(p.Channel.ChannelID)
	if err != nil {
		return nil, err
	}

	latest := entries[0]
	if p.lastVideoID == latest.VideoID {
		return nil, nil
	}
	p.lastVideoID = latest.VideoID

	for _, filter := range p.Channel.Filters {
		filter := regexp.Regexp(filter)
		if filter.MatchString(latest.Title) {
			return &latest, nil
		}
	}

	return nil, nil
}
