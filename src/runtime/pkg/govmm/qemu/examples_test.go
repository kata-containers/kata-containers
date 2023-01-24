// Copyright contributors to the Virtual Machine Manager for Go project
//
// SPDX-License-Identifier: Apache-2.0
//

package qemu_test

import (
	"time"

	"context"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/govmm/qemu"
)

func Example() {
	params := make([]string, 0, 32)

	// Rootfs
	params = append(params, "-drive", "file=/tmp/image.qcow2,if=virtio,aio=threads,format=qcow2")
	// Network
	params = append(params, "-net", "nic,model=virtio", "-net", "user")
	// kvm
	params = append(params, "-enable-kvm", "-cpu", "host")
	// qmp socket
	params = append(params, "-daemonize", "-qmp", "unix:/tmp/qmp-socket,server=on,wait=off")
	// resources
	params = append(params, "-m", "370", "-smp", "cpus=2")

	// LaunchCustomQemu should return immediately. We must then wait
	// the returned process to terminate as we are using the --daemonize
	// flag.
	// It will set up a unix domain socket called /tmp/qmp-socket that we
	// can use to manage the instance.
	proc, _, err := qemu.LaunchCustomQemu(context.Background(), "", params, nil, nil, nil)
	if err != nil {
		panic(err)
	}
	proc.Wait()

	// This channel will be closed when the instance dies.
	disconnectedCh := make(chan struct{})

	// Set up our options.  We don't want any logging or to receive any events.
	cfg := qemu.QMPConfig{}

	// Start monitoring the qemu instance.  This functon will block until we have
	// connect to the QMP socket and received the welcome message.
	q, _, err := qemu.QMPStart(context.Background(), "/tmp/qmp-socket", cfg, disconnectedCh)
	if err != nil {
		panic(err)
	}

	// This has to be the first command executed in a QMP session.
	err = q.ExecuteQMPCapabilities(context.Background())
	if err != nil {
		panic(err)
	}

	// Let's try to shutdown the VM.  If it hasn't shutdown in 10 seconds we'll
	// send a quit message.
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	err = q.ExecuteSystemPowerdown(ctx)
	cancel()
	if err != nil {
		err = q.ExecuteQuit(context.Background())
		if err != nil {
			panic(err)
		}
	}

	q.Shutdown()

	// disconnectedCh is closed when the VM exits. This line blocks until this
	// event occurs.
	<-disconnectedCh
}
