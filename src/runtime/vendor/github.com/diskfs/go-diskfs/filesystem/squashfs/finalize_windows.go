package squashfs

import (
	"os"
	"syscall"
)

func getDeviceNumbers(path string) (uint32, uint32, error) {
	return 0, 0, syscall.EWINDOWS
}

func getFileProperties(fi os.FileInfo) (uint32, uint32, uint32) {
	return 0, 0, 0
}
