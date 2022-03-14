package recorder

import (
	"bytes"
	"io"
)

type CallbackWriter struct {
	buffer   bytes.Buffer
	callback func(string)
}

// Validate that the CallbackWriter implements the io.Writer interface.
var _ io.Writer = &CallbackWriter{}

// NewCallbackWriter returns a new CallbackWriter.
func NewCallbackWriter(callback func(string)) *CallbackWriter {
	return &CallbackWriter{callback: callback}
}

// Write reads the bytes until a newline or carriage return is found. If not
// found, the bytes are buffered. If a newline or carriage return is found, the
// buffer is flushed to the callback.
func (w *CallbackWriter) Write(p []byte) (n int, err error) {
	for i, b := range p {
		if b == '\n' || b == '\r' {
			w.callback(w.buffer.String())
			w.buffer.Reset()
		} else {
			w.buffer.WriteByte(b)
		}
		n = i + 1
	}
	return
}
