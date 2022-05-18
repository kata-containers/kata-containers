package virtcontainers

import (
	"syscall"
)

// cleanUp Removes any stale sandbox/container state that can affect
// the next test to run.
func cleanUp() {
	syscall.Unmount(GetSharePath(testSandboxID), syscall.MNT_DETACH|UmountNoFollow)
	os.RemoveAll(testDir)
	os.MkdirAll(testDir, DirMode)

	setup()
}
