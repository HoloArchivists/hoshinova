package uploader

import (
	"context"
	"os"
	"path/filepath"

	"github.com/hizkifw/hoshinova/recorder"
)

type Local struct {
	Path    string
	BaseURL string
}

// Verify interface compliance
var _ Uploader = &Local{}

func NewLocal(path string, baseURL string) *Local {
	return &Local{
		Path:    path,
		BaseURL: baseURL,
	}
}

func (l *Local) Upload(ctx context.Context, item *recorder.Recording) (*UploadResult, error) {
	basename := filepath.Base(item.FilePath)

	if err := os.MkdirAll(l.Path, 0755); err != nil {
		return nil, err
	}
	if err := os.Rename(item.FilePath, filepath.Join(l.Path, basename)); err != nil {
		return nil, err
	}

	return &UploadResult{
		Title:       item.Title,
		VideoID:     item.VideoID,
		PublicURL:   filepath.Join(l.BaseURL, basename),
		ChannelID:   item.ChannelID,
		ChannelName: item.ChannelName,
	}, nil
}
