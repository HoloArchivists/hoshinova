package atomic

import sa "sync/atomic"

// ABool is an atomic bool.
type ABool struct {
	v uint32
}

// Set sets the atomic bool to the given value.
func (a *ABool) Set(v bool) {
	if v {
		sa.StoreUint32(&a.v, 1)
	} else {
		sa.StoreUint32(&a.v, 0)
	}
}

// Get gets the current value of the atomic bool.
func (a *ABool) Get() bool {
	return sa.LoadUint32(&a.v) != 0
}

// Flip flips the atomic bool.
func (a *ABool) Flip() {
	sa.SwapUint32(&a.v, 1-sa.LoadUint32(&a.v))
}
