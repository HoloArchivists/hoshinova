package watcher

import (
	"context"
	"fmt"
	"sync"
	"time"

	"github.com/hizkifw/hoshinova/config"
	"github.com/hizkifw/hoshinova/rss"
)

// Watch will create a goroutine for each channel in the configuration
func Watch(cfg *config.Config, ctx context.Context) {
	// Create WaitGroup
	wg := &sync.WaitGroup{}

	for _, c := range cfg.Channels {
		wg.Add(1)
		go func(channel *config.Channel) {
			defer wg.Done()
			p := Poller{Channel: channel}
			for {
				select {
				case <-ctx.Done():
					return
				default:
					p.Poll()
					time.Sleep(time.Duration(cfg.PollInterval) * time.Second)
				}
			}
		}(&c)
	}

	fmt.Printf("Watching %d channels\n", len(cfg.Channels))
	wg.Wait()
}

type Poller struct {
	Channel     *config.Channel
	lastVideoID string
}

func (p *Poller) Poll() error {
	entries, err := rss.Poll(p.Channel.ChannelID)
	if err != nil {
		fmt.Println("Error polling channel:", err)
		return err
	}

	latest := entries[0]
	if p.lastVideoID == latest.VideoID {
		return nil
	}

	p.lastVideoID = latest.VideoID
	fmt.Println("New video:", latest.Title)
	return nil
}
