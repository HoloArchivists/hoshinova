package task

import (
	"errors"

	"github.com/HoloArchivists/hoshinova/pubsub"
	"github.com/google/uuid"
)

const (
	StepAdded          Step = "added"
	StepWaitingForLive Step = "waiting for live"
	StepRecording      Step = "recording"
	StepMuxing         Step = "muxing"
	StepMuxed          Step = "muxed"
	StepUploading      Step = "uploading"
	StepUploaded       Step = "uploaded"
	StepDone           Step = "done"
	StepErrored        Step = "errored"
	StepCancelled      Step = "cancelled"
)

var (
	ErrTaskAlreadyExists = errors.New("task already exists")
	ErrTaskNotFound      = errors.New("task not found")
)

type Step string
type VideoID string
type PubSub = pubsub.PubSub[Task]

type Task struct {
	// ID is a unique identifier for the task. It's a UUID.
	ID uuid.UUID `json:"id"`

	// Title is the title of the video being archived.
	// Example: "Never Gonna Give You Up"
	Title string `json:"title"`
	// VideoID is the ID of the video being archived.
	// Example: "dQw4w9WgXcQ"
	VideoID VideoID `json:"video_id"`
	// ChannelName is the name of the channel the video is from.
	// Example: "Rick Astley"
	ChannelName string `json:"channel_name"`
	// ChannelID is the ID of the channel the video is from.
	// Example: "UC-lHJZR3Gqxm24_Vd_AJ5Yw"
	ChannelID string `json:"channel_id"`

	// Step is the current step of the task.
	Step Step `json:"status"`

	// Tags are the list of tags that the task is associated with. Each module
	// can add its own tags to the task as configured, and each module can also
	// filter which tasks it wants to run by these tags.
	Tags []string `json:"tags"`

	// CreatedAt is the time the task was first created. Usually this is the time
	// the task was added to the queue by the Scraper module.
	CreatedAt int64 `json:"created_at"`
}
