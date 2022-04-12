package scraper

import (
	"context"
	"encoding/xml"
	"fmt"
	"net/http"
	"regexp"
	"time"

	"github.com/HoloArchivists/hoshinova/config"
	"github.com/HoloArchivists/hoshinova/task"
	"github.com/HoloArchivists/hoshinova/util"
	"github.com/HoloArchivists/hoshinova/util/kv"
)

type RSS struct {
	cfgScraper config.Scraper
	cfgRSS     RSSConfig
	// kv to track which videos have been submitted to the queue.
	kv kv.KV[string, bool]
}

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
	PollIntervalSeconds int       `yaml:"poll_interval_seconds"`
	Channels            []Channel `yaml:"channels"`
}

type Channel struct {
	Name    string      `yaml:"name"`
	Id      string      `yaml:"id"`
	Tags    config.Tags `yaml:"tags"`
	Filters []Filter    `yaml:"filters"`
}

type Filter struct {
	Regex Regexp      `yaml:"regex"`
	Tags  config.Tags `yaml:"tags"`
}

type Regexp regexp.Regexp

type Video struct {
	ID    string
	Title string
}

func (r *Regexp) UnmarshalYAML(unmarshal func(interface{}) error) error {
	var s string
	if err := unmarshal(&s); err != nil {
		return err
	}
	re, err := regexp.Compile(s)
	if err != nil {
		return err
	}
	*r = Regexp(*re)
	return nil
}

func NewRSS(cfgScraper *config.Scraper) (Scraper, error) {
	var cfgRSS RSSConfig
	if err := cfgScraper.Config.Unmarshal(&cfgRSS); err != nil {
		return nil, fmt.Errorf("failed to parse RSS config: %w", err)
	}

	kv := kv.NewMemoryKV[string, bool]()
	return &RSS{*cfgScraper, cfgRSS, kv}, nil
}

func (r *RSS) Start(ctx context.Context, ps task.PubSub) error {
	pollInterval := time.Duration(r.cfgRSS.PollIntervalSeconds) * time.Second
	lg := util.GetLogger(ctx)

	return util.LoopUntilCancelled(ctx, func() error {
		for _, channel := range r.cfgRSS.Channels {
			videos, err := r.scrape(ctx, channel.Id)
			if err != nil {
				lg.Errorf("Error scraping %s: %w", channel.Name, err)
				continue
			}

		video:
			for _, video := range videos {
				if _, err := r.kv.Get(ctx, video.ID); err == nil {
					// Video has already been submitted to the queue.
					continue
				}

				// Check if the video matches any of the configured filters.
				for _, filter := range channel.Filters {
					re := regexp.Regexp(filter.Regex)
					if re.MatchString(video.Title) {
						// Add to the KV
						r.kv.Set(ctx, video.ID, true, time.Hour*24*7*365)

						// Create a new task.
						task := task.New(video.Title, video.ID, channel.Name, channel.Id)
						task.Tags = append(task.Tags, filter.Tags.Pub...)
						task.Tags = append(task.Tags, channel.Tags.Pub...)
						task.Tags = append(task.Tags, r.cfgScraper.Tags.Pub...)

						// Publish the task.
						if err := ps.Publish("scraper", task); err != nil {
							lg.Errorf("Failed to publish task: %v", err)
						}

						continue video
					}
				}
			}
		}

		return util.SleepContext(ctx, pollInterval)
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
