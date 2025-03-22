package util

func Uniqify[T comparable](s []T) []T {
	m := make(map[T]bool)
	for _, v := range s {
		m[v] = true
	}
	var result = make([]T, 0, len(m))
	for k := range m {
		result = append(result, k)
	}
	return result
}
