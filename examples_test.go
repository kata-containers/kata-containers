/*
// Copyright (c) 2016 Intel Corporation
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
*/

package qemu_test

import (
	"time"

	"context"

	"github.com/ciao-project/ciao/qemu"
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
	params = append(params, "-daemonize", "-qmp", "unix:/tmp/qmp-socket,server,nowait")
	// resources
	params = append(params, "-m", "370", "-smp", "cpus=2")

	// LaunchCustomQemu should return as soon as the instance has launched as we
	// are using the --daemonize flag.  It will set up a unix domain socket
	// called /tmp/qmp-socket that we can use to manage the instance.
	_, err := qemu.LaunchCustomQemu(context.Background(), "", params, nil, nil)
	if err != nil {
		panic(err)
	}

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
