// Copyright 2018 CNI authors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

package testutils

import (
	"crypto/rand"
	"fmt"
	"os"
	"path"
	"runtime"
	"strings"
	"sync"
	"syscall"

	"github.com/containernetworking/plugins/pkg/ns"
	"golang.org/x/sys/unix"
)

func getNsRunDir() string {
	xdgRuntimeDir := os.Getenv("XDG_RUNTIME_DIR")

	/// If XDG_RUNTIME_DIR is set, check if the current user owns /var/run.  If
	// the owner is different, we are most likely running in a user namespace.
	// In that case use $XDG_RUNTIME_DIR/netns as runtime dir.
	if xdgRuntimeDir != "" {
		if s, err := os.Stat("/var/run"); err == nil {
			st, ok := s.Sys().(*syscall.Stat_t)
			if ok && int(st.Uid) != os.Geteuid() {
				return path.Join(xdgRuntimeDir, "netns")
			}
		}
	}

	return "/var/run/netns"
}

// Creates a new persistent (bind-mounted) network namespace and returns an object
// representing that namespace, without switching to it.
func NewNS() (ns.NetNS, error) {

	nsRunDir := getNsRunDir()

	b := make([]byte, 16)
	_, err := rand.Reader.Read(b)
	if err != nil {
		return nil, fmt.Errorf("failed to generate random netns name: %v", err)
	}

	// Create the directory for mounting network namespaces
	// This needs to be a shared mountpoint in case it is mounted in to
	// other namespaces (containers)
	err = os.MkdirAll(nsRunDir, 0755)
	if err != nil {
		return nil, err
	}

	// Remount the namespace directory shared. This will fail if it is not
	// already a mountpoint, so bind-mount it on to itself to "upgrade" it
	// to a mountpoint.
	err = unix.Mount("", nsRunDir, "none", unix.MS_SHARED|unix.MS_REC, "")
	if err != nil {
		if err != unix.EINVAL {
			return nil, fmt.Errorf("mount --make-rshared %s failed: %q", nsRunDir, err)
		}

		// Recursively remount /var/run/netns on itself. The recursive flag is
		// so that any existing netns bindmounts are carried over.
		err = unix.Mount(nsRunDir, nsRunDir, "none", unix.MS_BIND|unix.MS_REC, "")
		if err != nil {
			return nil, fmt.Errorf("mount --rbind %s %s failed: %q", nsRunDir, nsRunDir, err)
		}

		// Now we can make it shared
		err = unix.Mount("", nsRunDir, "none", unix.MS_SHARED|unix.MS_REC, "")
		if err != nil {
			return nil, fmt.Errorf("mount --make-rshared %s failed: %q", nsRunDir, err)
		}

	}

	nsName := fmt.Sprintf("cnitest-%x-%x-%x-%x-%x", b[0:4], b[4:6], b[6:8], b[8:10], b[10:])

	// create an empty file at the mount point
	nsPath := path.Join(nsRunDir, nsName)
	mountPointFd, err := os.Create(nsPath)
	if err != nil {
		return nil, err
	}
	mountPointFd.Close()

	// Ensure the mount point is cleaned up on errors; if the namespace
	// was successfully mounted this will have no effect because the file
	// is in-use
	defer os.RemoveAll(nsPath)

	var wg sync.WaitGroup
	wg.Add(1)

	// do namespace work in a dedicated goroutine, so that we can safely
	// Lock/Unlock OSThread without upsetting the lock/unlock state of
	// the caller of this function
	go (func() {
		defer wg.Done()
		runtime.LockOSThread()
		// Don't unlock. By not unlocking, golang will kill the OS thread when the
		// goroutine is done (for go1.10+)

		var origNS ns.NetNS
		origNS, err = ns.GetNS(getCurrentThreadNetNSPath())
		if err != nil {
			return
		}
		defer origNS.Close()

		// create a new netns on the current thread
		err = unix.Unshare(unix.CLONE_NEWNET)
		if err != nil {
			return
		}

		// Put this thread back to the orig ns, since it might get reused (pre go1.10)
		defer origNS.Set()

		// bind mount the netns from the current thread (from /proc) onto the
		// mount point. This causes the namespace to persist, even when there
		// are no threads in the ns.
		err = unix.Mount(getCurrentThreadNetNSPath(), nsPath, "none", unix.MS_BIND, "")
		if err != nil {
			err = fmt.Errorf("failed to bind mount ns at %s: %v", nsPath, err)
		}
	})()
	wg.Wait()

	if err != nil {
		return nil, fmt.Errorf("failed to create namespace: %v", err)
	}

	return ns.GetNS(nsPath)
}

// UnmountNS unmounts the NS held by the netns object
func UnmountNS(ns ns.NetNS) error {
	nsPath := ns.Path()
	// Only unmount if it's been bind-mounted (don't touch namespaces in /proc...)
	if strings.HasPrefix(nsPath, getNsRunDir()) {
		if err := unix.Unmount(nsPath, 0); err != nil {
			return fmt.Errorf("failed to unmount NS: at %s: %v", nsPath, err)
		}

		if err := os.Remove(nsPath); err != nil {
			return fmt.Errorf("failed to remove ns path %s: %v", nsPath, err)
		}
	}

	return nil
}

// getCurrentThreadNetNSPath copied from pkg/ns
func getCurrentThreadNetNSPath() string {
	// /proc/self/ns/net returns the namespace of the main thread, not
	// of whatever thread this goroutine is running on.  Make sure we
	// use the thread's net namespace since the thread is switching around
	return fmt.Sprintf("/proc/%d/task/%d/ns/net", os.Getpid(), unix.Gettid())
}
