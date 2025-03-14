//go:build windows

package ext4

import (
	"errors"
)

func journalDevice(devicePath string) (deviceNumber uint32, err error) {
	return 0, errors.New("external journal device unsupported on Windows")
}
