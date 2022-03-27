package uploader

import (
	"context"
	"net/url"
	"os"
	"path/filepath"
	"strings"

	"github.com/HoloArchivists/hoshinova/module/recorder"
	"github.com/HoloArchivists/hoshinova/taskman"
	"github.com/HoloArchivists/hoshinova/util"
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
	tm.UpdateStep(item.VideoID, taskman.StepUploading)

	basename := filepath.Base(item.FilePath)

	// Create the destination directory if it doesn't exist
	if err := os.MkdirAll(l.Path, 0755); err != nil {
		return nil, err
	}

	// Move the file to the destination
	if err := os.Rename(item.FilePath, filepath.Join(l.Path, basename)); err != nil {
		return nil, err
	}

	publicURL := l.BaseURL
	if !strings.HasSuffix(publicURL, "/") {
		publicURL += "/"
	}
	publicURL += url.PathEscape(basename)

	return &UploadResult{
		VideoID:   item.VideoID,
		PublicURL: publicURL,
	}, nil
}
