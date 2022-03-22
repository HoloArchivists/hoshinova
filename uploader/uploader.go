package uploader

import (
	"context"
	"fmt"

	"github.com/HoloArchivists/hoshinova/config"
	"github.com/HoloArchivists/hoshinova/recorder"
	"github.com/HoloArchivists/hoshinova/taskman"
)

type Uploader interface {
	Upload(ctx context.Context, recording *recorder.Recording) (*UploadResult, error)
}

type UploadResult struct {
	VideoID   taskman.VideoId
	PublicURL string
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
