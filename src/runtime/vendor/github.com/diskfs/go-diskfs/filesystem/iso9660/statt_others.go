//go:build !windows
// +build !windows

//nolint:unconvert // linter gets confused in this file
package iso9660

import (
	"os"
	"syscall"
)

func statt(fi os.FileInfo) (links, uid, gid uint32) {
	if sys := fi.Sys(); sys != nil {
		if stat, ok := sys.(*syscall.Stat_t); ok {
			links, uid, gid = uint32(stat.Nlink), stat.Uid, stat.Gid
		}
	}

	return links, uid, gid
}

//nolint:deadcode // this is here solely so that linter does not complain on darwin about unconvert
func unused() uint32 {
	var f uint32 = 25
	return uint32(f)
}
