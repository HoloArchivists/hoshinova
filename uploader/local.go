package uploader

import (
	"context"
	"io"
	"net/url"
	"os"
	"path/filepath"
	"strings"

	"github.com/HoloArchivists/hoshinova/recorder"
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

	// Try to rename the file to the destination
	err := os.Rename(item.FilePath, filepath.Join(l.Path, basename))

	// If rename fails, try to copy instead.
	if err != nil {
		fileIn, err := os.Open(item.FilePath)
		if err != nil {
			return nil, err
		}

		fileOut, err := os.Create(filepath.Join(l.Path, basename))
		if err != nil {
			fileIn.Close()
			return nil, err
		}
		defer fileOut.Close()

		_, err = io.Copy(fileOut, fileIn)
		fileIn.Close()
		if err != nil {
			return nil, err
		}

		if err = os.Remove(item.FilePath); err != nil {
			return nil, err
		}
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
