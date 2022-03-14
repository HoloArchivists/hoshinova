package watcher

import (
	"context"
	"fmt"
	"regexp"
	"sync"
	"time"

	"github.com/hizkifw/hoshinova/config"
	"github.com/hizkifw/hoshinova/rss"
)

// Watch will create a goroutine for each channel in the configuration
func Watch(cfg *config.Config, ctx context.Context, callback func(PollEntry)) {
	wg := &sync.WaitGroup{}

	for _, c := range cfg.Channels {
		wg.Add(1)
		go func(channel config.Channel) {
			defer wg.Done()
			p := Poller{Channel: &channel}
			for {
				select {
				case <-ctx.Done():
					return
				default:
					latest, err := p.Poll()
					if err != nil {
						fmt.Println("Error polling channel:", err)
					}
					if latest != nil {
						fmt.Printf("New video (%s): %s\n", latest.VideoID, latest.Title)
						go callback(*latest)
					}
					time.Sleep(time.Duration(cfg.PollInterval) * time.Second)
				}
			}
		}(c)
	}

	fmt.Printf("Watching %d channels\n", len(cfg.Channels))
	wg.Wait()
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

// Poll will poll the channel and return the latest video that matches the
// configured filters
func (p *Poller) Poll() (*PollEntry, error) {
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
			return &PollEntry{
				VideoID:     latest.VideoID,
				Title:       latest.Title,
				ChannelID:   p.Channel.ChannelID,
				ChannelName: p.Channel.Name,
			}, nil
		}
	}

	return nil, nil
}
