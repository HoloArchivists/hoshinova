package scraper

import "context"

// Scraper is an interface for modules that scrape a list of videos for a given
// YouTube channel.
type Scraper interface {
	// Scrape scrapes the given channel for videos.
	Scrape(ctx context.Context, channelId string) ([]Video, error)
}

type Video struct {
	ID    string
	Title string
}
