package notifier

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"net/http"

	"github.com/hizkifw/hoshinova/uploader"
	"github.com/hizkifw/hoshinova/util"
)

type Discord struct {
	WebhookURL string
}

// Verify interface compliance
var _ Notifier = &Discord{}

func NewDiscord(webhookURL string) *Discord {
	return &Discord{
		WebhookURL: webhookURL,
	}
}

func (d *Discord) NotifyUploaded(ctx context.Context, notification *uploader.UploadResult) error {
	tm := util.GetTaskManager(ctx)
	task, err := tm.Get(notification.VideoID)
	if err != nil {
		return err
	}

	// Set up http client
	client := &http.Client{}

	// Set up discord message
	message := map[string]interface{}{
		"content": "",
		"embeds": []map[string]interface{}{
			{
				"title":       "Video downloaded",
				"description": fmt.Sprintf("[%s](%s)", task.Video.Title, notification.PublicURL),
				"fields": []map[string]interface{}{
					{
						"name":   "Source",
						"value":  fmt.Sprintf("[%s](https://youtu.be/%s)", notification.VideoID, notification.VideoID),
						"inline": true,
					},
					{
						"name":   "Channel",
						"value":  fmt.Sprintf("[%s](https://www.youtube.com/channel/%s)", task.Video.ChannelName, task.Video.ChannelId),
						"inline": true,
					},
				},
			},
		},
	}

	// Encode message
	jsonMessage, err := json.Marshal(message)
	if err != nil {
		return err
	}

	// Send message
	req, err := http.NewRequest("POST", d.WebhookURL, bytes.NewBuffer(jsonMessage))
	if err != nil {
		return err
	}
	req.Header.Set("Content-Type", "application/json")

	// Attach context
	req = req.WithContext(ctx)

	// Send request
	resp, err := client.Do(req)
	if err != nil {
		return err
	}

	if resp.StatusCode != 200 {
		return fmt.Errorf("Discord returned status code %d", resp.StatusCode)
	}

	return nil
}
