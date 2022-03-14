package recorder

import (
	"strconv"
	"strings"
)

type YTAState string

const (
	YTAStateIdle      YTAState = "idle"
	YTAStateWaiting   YTAState = "waiting"
	YTAStateRecording YTAState = "recording"
	YTAStateMuxing    YTAState = "muxing"
	YTAStateFinished  YTAState = "finished"
	YTAStateError     YTAState = "error"
)

type YTA struct {
	// The version of ytarchive that is currently running. If unknown, this
	// will be set to an empty string.
	Version string `json:"version"`

	// The current state of the process.
	//
	// Possible values:
	//   "idle" - The process has just started.
	//   "waiting" - The live stream has not yet started.
	//   "recording" - The live stream is currently being recorded.
	//   "muxing" - The recording is currently being muxed.
	//   "finished" - The recording has finished.
	//   "error" - An error occurred during the recording process.
	State YTAState `json:"state"`

	// The latest process output.
	LastOutput string `json:"last_output"`

	// The number of video fragments that have been recorded.
	VideoFragments int `json:"video_fragments"`
	// The number of audio fragments that have been recorded.
	AudioFragments int `json:"audio_fragments"`
	// The total size of all fragments that have been recorded.
	TotalSize string `json:"total_size"`
	// The selected video quality.
	VideoQuality string `json:"video_quality"`
}

func NewYTA() *YTA {
	return &YTA{
		State: "idle",
	}
}

// ParseLine parses a line of output from the ytarchive process.
//
// Sample output:
//
//   ytarchive 0.3.1-15663af
//   Stream starts at 2022-03-14T14:00:00+00:00 in 11075 seconds. Waiting for this time to elapse...
//   Stream is 30 seconds late...
//   Selected quality: 1080p60 (h264)
//   Video Fragments: 1215; Audio Fragments: 1215; Total Downloaded: 133.12MiB
//   Download Finished
//   Muxing final file...
//   Final file: /path/to/output.mp4
func (y *YTA) ParseLine(line string) {
	y.LastOutput = line

	// Check for the version.
	if y.Version == "" {
		if strings.HasPrefix(line, "ytarchive ") {
			y.Version = strings.TrimPrefix(line, "ytarchive ")
		}
	}

	if strings.HasPrefix(line, "Stream starts at ") ||
		strings.HasPrefix(line, "Stream is ") {
		y.State = YTAStateWaiting
	}

	if strings.HasPrefix(line, "Selected quality: ") {
		y.State = YTAStateRecording
		y.VideoQuality = strings.TrimPrefix(line, "Selected quality: ")
	}

	// Parse the video and audio fragment counts.
	if strings.HasPrefix(line, "Video Fragments: ") {
		parts := strings.Split(line, "; ")
		for _, part := range parts {
			if strings.HasPrefix(part, "Video Fragments: ") {
				y.VideoFragments, _ = strconv.Atoi(
					strings.TrimPrefix(part, "Video Fragments: "),
				)
			} else if strings.HasPrefix(part, "Audio Fragments: ") {
				y.AudioFragments, _ = strconv.Atoi(
					strings.TrimPrefix(part, "Audio Fragments: "),
				)
			} else if strings.HasPrefix(part, "Total Downloaded: ") {
				y.TotalSize = strings.TrimPrefix(part, "Total Downloaded: ")
			}
		}
	}

	if strings.HasPrefix(line, "Muxing final file...") {
		y.State = YTAStateMuxing
	}

	if strings.HasPrefix(line, "Final file: ") {
		y.State = YTAStateFinished
	}

	if strings.HasPrefix(line, "Livestream has been processed.") {
		y.State = YTAStateError
	}
}
