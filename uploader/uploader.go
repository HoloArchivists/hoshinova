package uploader

import (
	"context"
	"fmt"

	"github.com/hizkifw/hoshinova/config"
	"github.com/hizkifw/hoshinova/recorder"
)

type Uploader interface {
	Upload(ctx context.Context, recording *recorder.Recording) (*UploadResult, error)
}

type UploadResult struct {
	Title       string `json:"title"`
	VideoID     string `json:"video_id"`
	PublicURL   string `json:"public_url"`
	ChannelID   string `json:"channel_id"`
	ChannelName string `json:"channel_name"`
}

func NewUploader(u config.Uploader) (Uploader, error) {
	switch u.Type {
	case "local":
		path, ok := u.Config["path"]
		if !ok {
			return nil, fmt.Errorf("local uploader requires path")
		}
		base_url, ok := u.Config["base_url"]
		if !ok {
			return nil, fmt.Errorf("local uploader requires base_url")
		}
		return NewLocal(path, base_url), nil
	default:
		return nil, fmt.Errorf("unknown uploader type: %s", u.Type)
	}
}
