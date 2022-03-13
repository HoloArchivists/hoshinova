package rss

import (
	"encoding/xml"
	"net/http"
)

// Feed represents the YouTube RSS feed.
type Feed struct {
	XMLName xml.Name `xml:"feed"`
	Entries []Entry  `xml:"entry"`
}

type Entry struct {
	XMLName xml.Name `xml:"entry"`
	VideoID string   `xml:"videoId"`
	Title   string   `xml:"title"`
}

// Poll will fetch the latest list of videos from the channel's RSS feed and
// return the list of video IDs.
func Poll(channelId string) ([]Entry, error) {
	client := &http.Client{}
	req, err := http.NewRequest(
		"GET",
		"https://www.youtube.com/feeds/videos.xml?channel_id="+channelId,
		nil,
	)
	if err != nil {
		return nil, err
	}

	resp, err := client.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()

	feed := Feed{}
	err = xml.NewDecoder(resp.Body).Decode(&feed)
	if err != nil {
		return nil, err
	}

	return feed.Entries, nil
}
