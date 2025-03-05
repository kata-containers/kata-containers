package squashfs

import (
	"strings"
)

const (
	// KB represents one KB
	KB int64 = 1024
	// MB represents one MB
	MB int64 = 1024 * KB
	// GB represents one GB
	GB int64 = 1024 * MB
	// TB represents one TB
	TB int64 = 1024 * GB
	// max value of uint32
	uint32max uint64 = 0xffffffff
)

func universalizePath(p string) string {
	// globalize the separator
	return strings.ReplaceAll(p, `\`, "/")
}
func splitPath(p string) []string {
	ps := universalizePath(p)
	// we need to split such that each one ends in "/", except possibly the last one
	parts := strings.Split(ps, "/")
	// eliminate empty parts
	ret := make([]string, 0)
	for _, sub := range parts {
		if sub != "" {
			ret = append(ret, sub)
		}
	}
	return ret
}
