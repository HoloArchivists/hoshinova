package util

import "time"

type Debounce struct {
	d time.Duration
	t time.Time
}

func NewDebounce(d time.Duration) *Debounce {
	return &Debounce{
		d: d,
	}
}

func (d *Debounce) Check() bool {
	if time.Since(d.t) > d.d {
		d.t = time.Now()
		return true
	}
	return false
}
