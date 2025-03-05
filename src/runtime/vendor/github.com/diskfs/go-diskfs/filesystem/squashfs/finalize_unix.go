//go:build aix || darwin || dragonfly || freebsd || (js && wasm) || linux || nacl || netbsd || openbsd || solaris
// +build aix darwin dragonfly freebsd js,wasm linux nacl netbsd openbsd solaris

//nolint:unconvert // linter gets confused in this file
package squashfs

import (
	"os"

	"golang.org/x/sys/unix"
)

func getDeviceNumbers(path string) (major, minor uint32, err error) {
	stat := unix.Stat_t{}
	err = unix.Stat(path, &stat)
	if err != nil {
		return 0, 0, err
	}
	return uint32(stat.Rdev / 256), uint32(stat.Rdev % 256), nil
}

func getFileProperties(fi os.FileInfo) (links, uid, gid uint32) {
	if sys := fi.Sys(); sys != nil {
		if stat, ok := sys.(*unix.Stat_t); ok {
			links = uint32(stat.Nlink)
			uid = stat.Uid
			gid = stat.Gid
		}
	}
	return links, uid, gid
}

//nolint:deadcode // this is here solely so that linter does not complain on darwin about unconvert
func unused() uint32 {
	var f uint32 = 25
	return uint32(f)
}
