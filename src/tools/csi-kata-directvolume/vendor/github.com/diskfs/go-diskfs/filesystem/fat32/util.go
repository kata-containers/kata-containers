package fat32

import (
	"errors"
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
	// Fat32MaxSize is maximum size of a FAT32 filesystem in bytes
	Fat32MaxSize int64 = 2198754099200
)

func universalizePath(p string) (string, error) {
	// globalize the separator
	ps := strings.ReplaceAll(p, "\\", "/")
	if ps[0] != '/' {
		return "", errors.New("must use absolute paths")
	}
	return ps, nil
}
func splitPath(p string) ([]string, error) {
	ps, err := universalizePath(p)
	if err != nil {
		return nil, err
	}
	// we need to split such that each one ends in "/", except possibly the last one
	parts := strings.Split(ps, "/")
	// eliminate empty parts
	ret := make([]string, 0)
	for _, sub := range parts {
		if sub != "" {
			ret = append(ret, sub)
		}
	}
	return ret, nil
}
