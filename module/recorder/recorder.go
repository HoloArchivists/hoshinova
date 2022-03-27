package recorder

import "github.com/HoloArchivists/hoshinova/taskman"

type Recording struct {
	VideoID  taskman.VideoId
	FilePath string
}
