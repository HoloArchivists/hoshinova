package scraper

import (
	"context"
	"encoding/xml"
	"net/http"
)

type RSS struct{}

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

// Poll will fetch the latest list of videos from the channel's RSS feed and
// return the list of video IDs.
func (r *RSS) Scrape(ctx context.Context, channelId string) ([]Video, error) {
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
