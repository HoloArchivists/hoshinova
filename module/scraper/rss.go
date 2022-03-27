package scraper

import (
	"context"
	"encoding/xml"
	"net/http"
	"time"

	"github.com/HoloArchivists/hoshinova/config"
	"github.com/HoloArchivists/hoshinova/task"
	"github.com/HoloArchivists/hoshinova/util"
	"github.com/HoloArchivists/hoshinova/util/kv"
)

type RSS struct {
	c *config.Scraper[RSSConfig]
	// kv to track which videos have been submitted to the queue.
	kv kv.KV[string, bool]
}

var _ Scraper = &RSS{}

// rssFeed represents the YouTube RSS rssFeed.
type rssFeed struct {
	XMLName xml.Name   `xml:"feed"`
	Entries []rssEntry `xml:"entry"`
}

type rssEntry struct {
	XMLName xml.Name `xml:"entry"`
	VideoID string   `xml:"videoId"`
	Title   string   `xml:"title"`
}

// RSSConfig is the configuration for the RSS scraper.
type RSSConfig struct {
	PollIntervalSeconds int `json:"poll_interval_seconds"`
}

func NewRSS(c *config.Scraper[RSSConfig]) *RSS {
	return &RSS{c: c}
}

func (r *RSS) Watch(ctx context.Context, channels []config.Channel, ps task.PubSub) error {
	for _, channel := range channels {
		go r.watchChannel(ctx, channel)
	}

	return nil
}

func (r *RSS) watchChannel(ctx context.Context, channel config.Channel) error {
	pollInterval := time.Duration(r.c.Config.PollIntervalSeconds) * time.Second
	// Failure count for exponential backoff.
	failures := 0
	lg := util.GetLogger(ctx)

	return util.LoopUntilCancelled(ctx, func() error {
		videos, err := r.scrape(ctx, channel.Id)
		if err != nil {
			lg.Errorf("Failed to scrape RSS feed: %v", err)
			failures++
			util.SleepContext(ctx, time.Duration(2^failures)*time.Second)
			return err
		}
		failures = 0

		// Check if the video matches any of the configured filters.
		for _, video := range videos {
			if _, err := r.kv.Get(ctx, video.ID); err == nil {
				// Video has already been submitted to the queue.
				continue
			}

			// Check if the video matches any of the configured filters.

			// TODO

			// Add to the KV
			r.kv.Set(ctx, video.ID, true, time.Hour*24*7*365)
		}
	})
}

// scrape will fetch the latest list of videos from the channel's RSS feed and
// return the list of video IDs.
func (r *RSS) scrape(ctx context.Context, channelId string) ([]Video, error) {
	client := &http.Client{}
	req, err := http.NewRequest(
		"GET",
		"https://www.youtube.com/feeds/videos.xml?channel_id="+channelId,
		nil,
	)
	if err != nil {
		return nil, err
	}

	resp, err := client.Do(req.WithContext(ctx))
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()

	feed := rssFeed{}
	err = xml.NewDecoder(resp.Body).Decode(&feed)
	if err != nil {
		return nil, err
	}

	// Transform the feed into a []Video.
	videos := make([]Video, len(feed.Entries))
	for i, entry := range feed.Entries {
		videos[i] = Video{
			ID:    entry.VideoID,
			Title: entry.Title,
		}
	}

	return videos, nil
}
