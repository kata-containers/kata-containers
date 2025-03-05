//go:build !linux && !unix && !darwin && !windows

package ext4

import (
	"fmt"
	"runtime"
)

func journalDevice(devicePath string) (deviceNumber uint32, err error) {
	return 0, fmt.Errorf("external journal device unsupported on filesystem %s", runtime.GOOS)
}
