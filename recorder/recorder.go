package recorder

import (
	"bytes"
	"context"
	"fmt"
	"os"
	"os/exec"

	"github.com/hizkifw/hoshinova/watcher"
)

type Recording struct {
	VideoID     string
	Title       string
	FilePath    string
	ChannelID   string
	ChannelName string
}

func WaitAndRecord(ctx context.Context, in chan *watcher.PollEntry, out chan<- *Recording) error {
	for {
		select {
		case <-ctx.Done():
			return nil
		case entry := <-in:
			go func(entry watcher.PollEntry) {
				rec, err := Record(ctx, &entry)
				if err != nil {
					fmt.Printf("Error recording %s: %s\n", entry.VideoID, err)
					return
				}
				fmt.Printf("Finished recording %s\n", entry.VideoID)
				out <- rec
			}(*entry)
		}
	}
}

func Record(ctx context.Context, entry *watcher.PollEntry) (*Recording, error) {
	url := "https://www.youtube.com/watch?v=" + entry.VideoID
	tempdir, err := os.MkdirTemp("", "rec")
	if err != nil {
		return nil, err
	}
	// Do not defer os.RemoveAll(tempdir) because we want to keep the recordings
	// in case of error.

	fmt.Printf("Downloading %s to %s\n", entry.VideoID, tempdir)

	cmd := exec.CommandContext(
		ctx,
		"ytarchive",
		"--wait", "--vp9",
		"--thumbnail", "--add-metadata",
		"--threads", "8",
		"--output", "%(id)s",
		url, "best",
	)
	cmd.Dir = tempdir

	var out bytes.Buffer
	cmd.Stdout = &out
	cmd.Stderr = &out

	if err := cmd.Start(); err != nil {
		return nil, err
	}

	// Check return code
	if err := cmd.Wait(); err != nil {
		return nil, err
	}

	return &Recording{
		VideoID:     entry.VideoID,
		Title:       entry.Title,
		FilePath:    tempdir,
		ChannelID:   entry.ChannelID,
		ChannelName: entry.ChannelName,
	}, nil
}
