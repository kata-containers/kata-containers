// SPDX-License-Identifier: Apache-2.0
//

package utils

import (
	"errors"
	"strings"
	"testing"

	mountutils "k8s.io/mount-utils"
)

// stubGetDiskFormat replaces the device-format probe for the duration
// of one test. It mutates the package-level getDiskFormat variable, so
// tests using it must remain sequential (no t.Parallel()).
func stubGetDiskFormat(t *testing.T, format string, err error) {
	t.Helper()
	old := getDiskFormat
	getDiskFormat = func(*SafeMountFormater, string) (string, error) {
		return format, err
	}
	t.Cleanup(func() { getDiskFormat = old })
}

func newTestMounter() *SafeMountFormater {
	safeMnt := NewSafeMountFormater()
	return &safeMnt
}

// A formatted disk staged with a different requested fstype used to
// crash the driver: the FilesystemMismatch branch called err.Error()
// with err guaranteed nil, panicking the whole process on any mismatch.
func TestSafeFormatWithFstypeFilesystemMismatchNoPanic(t *testing.T) {
	mounter := newTestMounter()

	// Disk holds ext4, the volume requests xfs.
	stubGetDiskFormat(t, "ext4", nil)

	defer func() {
		if r := recover(); r != nil {
			t.Fatalf("SafeFormatWithFstype panicked on filesystem mismatch: %v", r)
		}
	}()
	err := mounter.SafeFormatWithFstype("/dev/fake-disk", "xfs", []string{"rw"})
	if err == nil {
		t.Fatal("filesystem mismatch must be reported as an error")
	}
	var mountErr mountutils.MountError
	if !errors.As(err, &mountErr) {
		t.Fatalf("expected a MountError, got %T: %v", err, err)
	}
	if mountErr.Type != mountutils.FilesystemMismatch {
		t.Fatalf("expected FilesystemMismatch, got %v", mountErr.Type)
	}
	if !strings.Contains(err.Error(), "ext4") || !strings.Contains(err.Error(), "xfs") {
		t.Fatalf("error should name both filesystems, got: %v", err)
	}
}

// The probe failing is already handled: it must surface
// GetDiskFormatFailed rather than proceed to format.
func TestSafeFormatWithFstypeProbeFailure(t *testing.T) {
	mounter := newTestMounter()

	stubGetDiskFormat(t, "", errors.New("blkid failed"))

	err := mounter.SafeFormatWithFstype("/dev/fake-disk", "ext4", []string{"rw"})
	if err == nil {
		t.Fatal("probe failure must be reported")
	}
	var mountErr mountutils.MountError
	if !errors.As(err, &mountErr) || mountErr.Type != mountutils.GetDiskFormatFailed {
		t.Fatalf("expected GetDiskFormatFailed, got %v", err)
	}
}

// Unformatted read-only disks keep returning UnformattedReadOnly.
func TestSafeFormatWithFstypeUnformattedReadOnly(t *testing.T) {
	mounter := newTestMounter()

	stubGetDiskFormat(t, "", nil)

	err := mounter.SafeFormatWithFstype("/dev/fake-disk", "ext4", []string{"ro"})
	if err == nil {
		t.Fatal("unformatted read-only mount must be reported")
	}
	var mountErr mountutils.MountError
	if !errors.As(err, &mountErr) || mountErr.Type != mountutils.UnformattedReadOnly {
		t.Fatalf("expected UnformattedReadOnly, got %v", err)
	}
}

// Formatting an unformatted read-write disk runs mkfs: exercise the
// happy path far enough to prove the mismatch early-return is the
// only branch that changed. mkfs is invoked through the real executor,
// which fails to find a mkfs.fake binary: the FormatFailed error proves
// the flow reached formatting instead of panic'ing.
func TestSafeFormatWithFstypeUnformattedRunsMkfs(t *testing.T) {
	mounter := newTestMounter()

	stubGetDiskFormat(t, "", nil)

	err := mounter.SafeFormatWithFstype("/dev/fake-disk", "fakefs", []string{"rw"})
	if err == nil {
		t.Fatal("formatting with a fake filesystem must fail (no mkfs.fake binary)")
	}
	var mountErr mountutils.MountError
	if !errors.As(err, &mountErr) || mountErr.Type != mountutils.FormatFailed {
		t.Fatalf("expected FormatFailed, got %v", err)
	}
}
