// Copyright (c) 2026 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package resourcecontrol

import (
	"context"
	"errors"
	"os"
	"os/exec"
	"os/signal"
	"syscall"
	"testing"
	"time"

	cgroup1 "github.com/containerd/cgroups/v3/cgroup1"
	"github.com/godbus/dbus/v5"
	"github.com/stretchr/testify/require"
)

type testSystemdStopConnection struct {
	stop   func(context.Context, string, string, chan<- string) (int, error)
	closed bool
}

func (conn *testSystemdStopConnection) StopUnitContext(ctx context.Context, unit, mode string, result chan<- string) (int, error) {
	return conn.stop(ctx, unit, mode, result)
}

func (conn *testSystemdStopConnection) Close() { conn.closed = true }

type testFilesystemCgroup struct {
	cgroup1.Cgroup
	deleted bool
}

func (cgroup *testFilesystemCgroup) Delete() error {
	cgroup.deleted = true
	return nil
}

func TestDeleteFilesystemCgroup(t *testing.T) {
	for _, sandboxOnly := range []bool{false, true} {
		cgroup := &testFilesystemCgroup{}
		controller := &LinuxCgroup{
			path:              "/kata/sandbox",
			sandboxCgroupOnly: sandboxOnly,
			cgroup:            cgroup,
		}
		require.NoError(t, controller.Delete())
		require.True(t, cgroup.deleted)
	}
}

func TestDeleteSystemdScope(t *testing.T) {
	for _, testCase := range []struct {
		name      string
		err       error
		wantError bool
	}{
		{name: "stop accepted"},
		{name: "already removed", err: dbus.Error{Name: "org.freedesktop.systemd1.NoSuchUnit"}},
		{name: "stop failed", err: errors.New("DBus unavailable"), wantError: true},
	} {
		t.Run(testCase.name, func(t *testing.T) {
			originalConnection := newSystemdStopConnection
			originalIgnored := signal.Ignored(syscall.SIGTERM)
			t.Cleanup(func() {
				newSystemdStopConnection = originalConnection
				if !originalIgnored {
					signal.Reset(syscall.SIGTERM)
				}
			})
			calls := 0
			conn := &testSystemdStopConnection{stop: func(ctx context.Context, unit, mode string, result chan<- string) (int, error) {
				calls++
				require.Equal(t, "cri-containerd-sandbox.scope", unit)
				require.Equal(t, "replace", mode)
				require.Nil(t, result)
				require.True(t, signal.Ignored(syscall.SIGTERM))
				deadline, ok := ctx.Deadline()
				require.True(t, ok)
				require.WithinDuration(t, time.Now().Add(10*time.Second), deadline, time.Second)
				return 1, testCase.err
			}}
			newSystemdStopConnection = func(context.Context) (systemdStopConnection, error) { return conn, nil }
			controller := &LinuxCgroup{path: "pod.slice:cri-containerd:sandbox", sandboxCgroupOnly: true}
			err := controller.Delete()
			if testCase.wantError {
				require.ErrorIs(t, err, testCase.err)
			} else {
				require.NoError(t, err)
			}
			require.Equal(t, 1, calls)
			require.True(t, conn.closed)
		})
	}
}

func TestDeleteSystemdScopeSignal(t *testing.T) {
	if os.Getenv("KATA_SYSTEMD_SIGNAL_TEST") == "1" {
		received := make(chan os.Signal, 1)
		signal.Notify(received, syscall.SIGTERM)
		defer signal.Stop(received)
		conn := &testSystemdStopConnection{stop: func(ctx context.Context, unit, mode string, result chan<- string) (int, error) {
			require.Nil(t, result)
			require.NoError(t, syscall.Kill(os.Getpid(), syscall.SIGTERM))
			select {
			case <-received:
				t.Fatal("scope SIGTERM reached the framework signal handler")
			case <-time.After(20 * time.Millisecond):
			}
			return 1, nil
		}}
		newSystemdStopConnection = func(context.Context) (systemdStopConnection, error) { return conn, nil }
		require.NoError(t, deleteCgroupsSystemd("pod.slice:cri-containerd:sandbox"))
		require.True(t, conn.closed)
		return
	}
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, os.Args[0], "-test.run=^TestDeleteSystemdScopeSignal$")
	command.Env = append(os.Environ(), "KATA_SYSTEMD_SIGNAL_TEST=1")
	command.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}
	output, err := command.CombinedOutput()
	require.NoError(t, err, "%s", output)
}
