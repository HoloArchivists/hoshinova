package notifier

import (
	"context"
	"fmt"

	"github.com/HoloArchivists/hoshinova/config"
	"github.com/HoloArchivists/hoshinova/module/uploader"
)

type Notifier interface {
	NotifyUploaded(ctx context.Context, notification *uploader.UploadResult) error
}

func NewNotifier(n config.Notifier) (Notifier, error) {
	switch n.Type {
	case "discord":
		webhook_url, ok := n.Config["webhook_url"]
		if !ok {
			return nil, fmt.Errorf("webhook_url is not set")
		}
		return NewDiscord(webhook_url), nil
	default:
		return nil, fmt.Errorf("unknown notifier type: %s", n.Type)
	}
}
