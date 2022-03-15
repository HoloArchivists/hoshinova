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

func NewDiscord(webhookURL string) Notifier {
	return &Discord{
		WebhookURL: webhookURL,
	}
}

func (d *Discord) NotifyUploaded(ctx context.Context, notification *uploader.UploadResult) error {
	lg := util.GetLogger(ctx)
	tm := util.GetTaskManager(ctx)
	task, ok := tm.Get(notification.VideoID)
	if !ok {
		return fmt.Errorf("task not found")
	}

	lg.Debug("notify uploaded", "video_id", notification.VideoID)

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

	lg.Debug("Encoding message", "message", message)

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

	lg.Debug("Sending message", "message", string(jsonMessage))

	// Send request
	resp, err := client.Do(req)
	if err != nil {
		return err
	}

	lg.Debug("Response", "status", resp.Status)

	if resp.StatusCode != 204 {
		return fmt.Errorf("Discord returned status code %d", resp.StatusCode)
	}

	return nil
}
