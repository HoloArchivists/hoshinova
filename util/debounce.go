package util

import "time"

// Debounce provides a function that will only return true once every
// d duration.
type Debounce struct {
	duration time.Duration
	lastCall time.Time
}

// NewDebounce returns a new debounce.
func NewDebounce(d time.Duration) *Debounce {
	return &Debounce{
		duration: d,
	}
}

// Check returns true if it has not been called in d duration.
func (d *Debounce) Check() bool {
	if time.Since(d.lastCall) > d.duration {
		d.lastCall = time.Now()
		return true
	}
	return false
}
