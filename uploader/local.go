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

// Verify interface compliance
var _ Uploader = &Local{}

func NewLocal(path string, baseURL string) *Local {
	return &Local{
		Path:    path,
		BaseURL: baseURL,
	}
}

func (l *Local) Upload(ctx context.Context, item *recorder.Recording) (*UploadResult, error) {
	tm := util.GetTaskManager(ctx)
	tm.LogEvent(item.VideoID, "uploading")

	basename := filepath.Base(item.FilePath)

	if err := os.MkdirAll(l.Path, 0755); err != nil {
		return nil, err
	}
	if err := os.Rename(item.FilePath, filepath.Join(l.Path, basename)); err != nil {
		return nil, err
	}

	return &UploadResult{
		VideoID:   item.VideoID,
		PublicURL: filepath.Join(l.BaseURL, basename),
	}, nil
}
