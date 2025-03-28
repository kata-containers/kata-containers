//go:build wasip1
// +build wasip1

//nolint:unconvert // linter gets confused in this file
package squashfs

import (
	"errors"
	"os"
)

func getDeviceNumbers(path string) (major, minor uint32, err error) {
	return 0, 0, errors.New("not implemented")
}

func getFileProperties(fi os.FileInfo) (links, uid, gid uint32) {
	return 0, 0, 0
}
