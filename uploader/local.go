package uploader

import (
	"context"
	"os"
	"path/filepath"

	"github.com/hizkifw/hoshinova/recorder"
	"github.com/hizkifw/hoshinova/util"
)

type Local struct {
	Path    string
	BaseURL string
}

func NewLocal(path string, baseURL string) Uploader {
	return &Local{
		Path:    path,
		BaseURL: baseURL,
	}
}

func (l *Local) Upload(ctx context.Context, item *recorder.Recording) (*UploadResult, error) {
	tm := util.GetTaskManager(ctx)
	tm.LogEvent(item.VideoID, "uploading")

	basename := filepath.Base(item.FilePath)

	// Create the destination directory if it doesn't exist
	if err := os.MkdirAll(l.Path, 0755); err != nil {
		return nil, err
	}

	// Move the file to the destination
	if err := os.Rename(item.FilePath, filepath.Join(l.Path, basename)); err != nil {
		return nil, err
	}

	return &UploadResult{
		VideoID:   item.VideoID,
		PublicURL: filepath.Join(l.BaseURL, basename),
	}, nil
}
