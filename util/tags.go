package util

// Intersect returns true if the two slices have at least one element in common.
func Intersect(a, b []string) bool {
	for _, x := range a {
		for _, y := range b {
			if x == y {
				return true
			}
		}
	}
	return false
}
