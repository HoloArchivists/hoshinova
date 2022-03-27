package scraper

import (
	"context"
	"fmt"

	"github.com/HoloArchivists/hoshinova/config"
	"github.com/HoloArchivists/hoshinova/task"
)

// Scraper is an interface for modules that scrape a list of videos for a given
// YouTube channel. Scrapers are responsible for contiuously monitoring the
// channel and notifying the pubsub system when new videos are available.
type Scraper interface {
	// Watch starts the scraper. It should block until the context is canceled,
	// and only return when a fatal error occurs.
	Watch(ctx context.Context, channels []config.Channel, ps task.PubSub) error
}

type Video struct {
	ID    string
	Title string
}

func New(s *config.Scraper) (Scraper, error) {
	switch s.Type {
	case "rss":
		return NewRSS(s), nil
	default:
		return nil, fmt.Errorf("unknown scraper type: %s", s.Type)
	}
}
