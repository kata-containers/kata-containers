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

package qemu

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"log"
	"sync"
	"testing"
	"time"

	"context"

	"github.com/ciao-project/ciao/testutil"
)

const (
	microStr = "50"
	minorStr = "6"
	majorStr = "2"
	micro    = 50
	minor    = 6
	major    = 2
	cap1     = "one"
	cap2     = "two"
	qmpHello = `{ "QMP": { "version": { "qemu": { "micro": ` + microStr + `, "minor": ` + minorStr + `, "major": ` + majorStr + ` }, "package": ""}, "capabilities": ["` + cap1 + `","` + cap2 + `"]}}` + "\n"
)

type qmpTestLogger struct{}

func (l qmpTestLogger) V(level int32) bool {
	return true
}

func (l qmpTestLogger) Infof(format string, v ...interface{}) {
	log.Printf(format, v...)
}

func (l qmpTestLogger) Warningf(format string, v ...interface{}) {
	l.Infof(format, v)
}

func (l qmpTestLogger) Errorf(format string, v ...interface{}) {
	l.Infof(format, v)
}

type qmpTestCommand struct {
	name string
	args map[string]interface{}
}

type qmpTestEvent struct {
	name      string
	data      map[string]interface{}
	timestamp map[string]interface{}
	after     time.Duration
}

type qmpTestResult struct {
	result string
	data   map[string]interface{}
}

type qmpTestCommandBuffer struct {
	newDataCh  chan []byte
	t          *testing.T
	buf        *bytes.Buffer
	cmds       []qmpTestCommand
	events     []qmpTestEvent
	results    []qmpTestResult
	currentCmd int
	forceFail  chan struct{}
}

func newQMPTestCommandBuffer(t *testing.T) *qmpTestCommandBuffer {
	b := &qmpTestCommandBuffer{
		newDataCh: make(chan []byte, 1),
		t:         t,
		buf:       bytes.NewBuffer([]byte{}),
		forceFail: make(chan struct{}),
	}
	b.cmds = make([]qmpTestCommand, 0, 8)
	b.events = make([]qmpTestEvent, 0, 8)
	b.results = make([]qmpTestResult, 0, 8)
	b.newDataCh <- []byte(qmpHello)
	return b
}

func (b *qmpTestCommandBuffer) startEventLoop(wg *sync.WaitGroup) {
	wg.Add(1)
	go func() {
		for _, ev := range b.events {
			time.Sleep(ev.after)
			eventMap := map[string]interface{}{
				"event": ev.name,
			}

			if ev.data != nil {
				eventMap["data"] = ev.data
			}

			if ev.timestamp != nil {
				eventMap["timestamp"] = ev.timestamp
			}

			encodedEvent, err := json.Marshal(&eventMap)
			if err != nil {
				b.t.Errorf("Unable to encode event: %v", err)
			}
			encodedEvent = append(encodedEvent, '\n')
			b.newDataCh <- encodedEvent
		}
		wg.Done()
	}()
}

func (b *qmpTestCommandBuffer) AddCommand(name string, args map[string]interface{},
	result string, data map[string]interface{}) {
	b.cmds = append(b.cmds, qmpTestCommand{name, args})
	if data == nil {
		data = make(map[string]interface{})
	}
	b.results = append(b.results, qmpTestResult{result, data})
}

func (b *qmpTestCommandBuffer) AddEvent(name string, after time.Duration,
	data map[string]interface{}, timestamp map[string]interface{}) {
	b.events = append(b.events, qmpTestEvent{
		name:      name,
		data:      data,
		timestamp: timestamp,
		after:     after,
	})
}

func (b *qmpTestCommandBuffer) Close() error {
	close(b.newDataCh)
	return nil
}

func (b *qmpTestCommandBuffer) Read(p []byte) (n int, err error) {
	if b.buf.Len() == 0 {
		ok := false
		var data []byte
		select {
		case <-b.forceFail:
			return 0, errors.New("Connection shutdown")
		case data, ok = <-b.newDataCh:
			select {
			case <-b.forceFail:
				return 0, errors.New("Connection shutdown")
			default:
			}
		}
		if !ok {
			return 0, nil
		}
		_, err := b.buf.Write(data)
		if err != nil {
			if err != nil {
				b.t.Errorf("Unable to buffer result: %v", err)
			}
		}
	}
	return b.buf.Read(p)
}

func (b *qmpTestCommandBuffer) Write(p []byte) (int, error) {
	var cmdJSON map[string]interface{}
	currentCmd := b.currentCmd
	b.currentCmd++
	if currentCmd >= len(b.cmds) {
		b.t.Fatalf("Unexpected command")
	}
	err := json.Unmarshal(p, &cmdJSON)
	if err != nil {
		b.t.Fatalf("Unexpected command")
	}
	cmdName := cmdJSON["execute"]
	gotCmdName := cmdName.(string)
	result := b.results[currentCmd].result
	if gotCmdName != b.cmds[currentCmd].name {
		b.t.Errorf("Unexpected command.  Expected %s found %s",
			b.cmds[currentCmd].name, gotCmdName)
		result = "error"
	}
	resultMap := make(map[string]interface{})
	resultMap[result] = b.results[currentCmd].data
	encodedRes, err := json.Marshal(&resultMap)
	if err != nil {
		b.t.Errorf("Unable to encode result: %v", err)
	}
	encodedRes = append(encodedRes, '\n')
	b.newDataCh <- encodedRes
	return len(p), nil
}

func checkVersion(t *testing.T, connectedCh <-chan *QMPVersion) *QMPVersion {
	var version *QMPVersion
	select {
	case <-time.After(time.Second):
		t.Fatal("Timed out waiting for qmp to connect")
	case version = <-connectedCh:
	}

	if version == nil {
		t.Fatal("Invalid version information received")
	}
	if version.Micro != micro || version.Minor != minor ||
		version.Major != major {
		t.Fatal("Invalid version number")
	}

	if len(version.Capabilities) != 2 {
		if version.Capabilities[0] != cap1 || version.Capabilities[1] != cap2 {
			t.Fatal("Invalid capabilities")
		}
	}

	return version
}

// Checks that a QMP Loop can be started and shutdown.
//
// We start a QMPLoop and shut it down.
//
// Loop should start up and shutdown correctly.  The version information
// returned from startQMPLoop should be correct.
func TestQMPStartStopLoop(t *testing.T) {
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	checkVersion(t, connectedCh)
	q.Shutdown()
	<-disconnectedCh
}

// Checks that the qmp_capabilities command is correctly sent.
//
// We start a QMPLoop, send the qmp_capabilities command and stop the
// loop.
//
// The qmp_capabilities should be correctly sent and the QMP loop
// should exit gracefully.
func TestQMPCapabilities(t *testing.T) {
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	buf.AddCommand("qmp_capabilities", nil, "return", nil)
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	checkVersion(t, connectedCh)
	err := q.ExecuteQMPCapabilities(context.Background())
	if err != nil {
		t.Fatalf("Unexpected error %v", err)
	}
	q.Shutdown()
	<-disconnectedCh
}

// Checks that the stop command is correctly sent.
//
// We start a QMPLoop, send the stop command and stop the
// loop.
//
// The stop command should be correctly sent and the QMP loop
// should exit gracefully.
func TestQMPStop(t *testing.T) {
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	buf.AddCommand("stop", nil, "return", nil)
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	checkVersion(t, connectedCh)
	err := q.ExecuteStop(context.Background())
	if err != nil {
		t.Fatalf("Unexpected error %v", err)
	}
	q.Shutdown()
	<-disconnectedCh
}

// Checks that the cont command is correctly sent.
//
// We start a QMPLoop, send the cont command and stop the
// loop.
//
// The cont command should be correctly sent and the QMP loop
// should exit gracefully.
func TestQMPCont(t *testing.T) {
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	buf.AddCommand("cont", nil, "return", nil)
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	checkVersion(t, connectedCh)
	err := q.ExecuteCont(context.Background())
	if err != nil {
		t.Fatalf("Unexpected error %v", err)
	}
	q.Shutdown()
	<-disconnectedCh
}

// Checks that the quit command is correctly sent.
//
// We start a QMPLoop, send the quit command and wait for the loop to exit.
//
// The quit command should be correctly sent and the QMP loop should exit
// gracefully without the test calling q.Shutdown().
func TestQMPQuit(t *testing.T) {
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	buf.AddCommand("quit", nil, "return", nil)
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	checkVersion(t, connectedCh)
	err := q.ExecuteQuit(context.Background())
	if err != nil {
		t.Fatalf("Unexpected error %v", err)
	}
	close(buf.forceFail)
	<-disconnectedCh
}

// Checks that the blockdev-add command is correctly sent.
//
// We start a QMPLoop, send the blockdev-add command and stop the loop.
//
// The blockdev-add command should be correctly sent and the QMP loop should
// exit gracefully.
func TestQMPBlockdevAdd(t *testing.T) {
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	buf.AddCommand("blockdev-add", nil, "return", nil)
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	q.version = checkVersion(t, connectedCh)
	err := q.ExecuteBlockdevAdd(context.Background(), "/dev/rbd0",
		fmt.Sprintf("drive_%s", testutil.VolumeUUID))
	if err != nil {
		t.Fatalf("Unexpected error %v", err)
	}
	q.Shutdown()
	<-disconnectedCh
}

// Checks that the device_add command is correctly sent.
//
// We start a QMPLoop, send the device_add command and stop the loop.
//
// The device_add command should be correctly sent and the QMP loop should
// exit gracefully.
func TestQMPDeviceAdd(t *testing.T) {
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	buf.AddCommand("device_add", nil, "return", nil)
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	checkVersion(t, connectedCh)
	blockdevID := fmt.Sprintf("drive_%s", testutil.VolumeUUID)
	devID := fmt.Sprintf("device_%s", testutil.VolumeUUID)
	err := q.ExecuteDeviceAdd(context.Background(), blockdevID, devID,
		"virtio-blk-pci", "")
	if err != nil {
		t.Fatalf("Unexpected error %v", err)
	}
	q.Shutdown()
	<-disconnectedCh
}

// Checks that the x-blockdev-del command is correctly sent.
//
// We start a QMPLoop, send the x-blockdev-del command and stop the loop.
//
// The x-blockdev-del command should be correctly sent and the QMP loop should
// exit gracefully.
func TestQMPXBlockdevDel(t *testing.T) {
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	buf.AddCommand("x-blockdev-del", nil, "return", nil)
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	q.version = checkVersion(t, connectedCh)
	err := q.ExecuteBlockdevDel(context.Background(),
		fmt.Sprintf("drive_%s", testutil.VolumeUUID))
	if err != nil {
		t.Fatalf("Unexpected error %v", err)
	}
	q.Shutdown()
	<-disconnectedCh
}

// Checks that the device_del command is correctly sent.
//
// We start a QMPLoop, send the device_del command and wait for it to complete.
// This command generates some events so we start a separate go routine to check
// that they are received.
//
// The device_del command should be correctly sent and the QMP loop should
// exit gracefully.  We should also receive two events on the eventCh.
func TestQMPDeviceDel(t *testing.T) {
	const (
		seconds         = 1352167040730
		microsecondsEv1 = 123456
		microsecondsEv2 = 123556
		device          = "device_" + testutil.VolumeUUID
		path            = "/dev/rbd0"
	)

	var wg sync.WaitGroup
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	buf.AddCommand("device_del", nil, "return", nil)
	buf.AddEvent("DEVICE_DELETED", time.Millisecond*200,
		map[string]interface{}{
			"path": path,
		},
		map[string]interface{}{
			"seconds":      seconds,
			"microseconds": microsecondsEv1,
		})
	buf.AddEvent("DEVICE_DELETED", time.Millisecond*200,
		map[string]interface{}{
			"device": device,
			"path":   path,
		},
		map[string]interface{}{
			"seconds":      seconds,
			"microseconds": microsecondsEv2,
		})
	eventCh := make(chan QMPEvent)
	cfg := QMPConfig{EventCh: eventCh, Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	wg.Add(1)
	go func() {
		for i := 0; i < 2; i++ {
			select {
			case <-eventCh:
			case <-time.After(time.Second):
				t.Error("Timedout waiting for event")
			}
		}
		wg.Done()
	}()
	checkVersion(t, connectedCh)
	buf.startEventLoop(&wg)
	err := q.ExecuteDeviceDel(context.Background(),
		fmt.Sprintf("device_%s", testutil.VolumeUUID))
	if err != nil {
		t.Fatalf("Unexpected error %v", err)
	}
	q.Shutdown()
	<-disconnectedCh
	wg.Wait()
}

// Checks that contexts can be used to timeout a command.
//
// We start a QMPLoop and send the device_del command with a context that times
// out after 1 second.  We don't however arrangefor any DEVICE_DELETED events
// to be sent so the device_del command should not complete normally.  We then
// shutdown the QMP loop.
//
// The device_del command should timeout after 1 second and the QMP loop
// should exit gracefully.
func TestQMPDeviceDelTimeout(t *testing.T) {
	var wg sync.WaitGroup
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	buf.AddCommand("device_del", nil, "return", nil)
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	checkVersion(t, connectedCh)
	ctx, cancel := context.WithTimeout(context.Background(), time.Second)
	err := q.ExecuteDeviceDel(ctx,
		fmt.Sprintf("device_%s", testutil.VolumeUUID))
	cancel()
	if err != context.DeadlineExceeded {
		t.Fatalf("Timeout expected found %v", err)
	}
	q.Shutdown()
	<-disconnectedCh
	wg.Wait()
}

// Checks that contexts can be used to cancel a command.
//
// We start a QMPLoop and send two qmp_capabilities commands, cancelling
// the first.  The second is allowed to proceed normally.
//
// The first call to ExecuteQMPCapabilities should fail with
// context.Canceled.  The second should succeed.
func TestQMPCancel(t *testing.T) {
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	buf.AddCommand("qmp_capabilities", nil, "return", nil)
	buf.AddCommand("qmp_capabilities", nil, "return", nil)
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	checkVersion(t, connectedCh)
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	err := q.ExecuteQMPCapabilities(ctx)
	if err != context.Canceled {
		t.Fatalf("Unexpected error %v", err)
	}
	err = q.ExecuteQMPCapabilities(context.Background())
	if err != nil {
		t.Fatalf("Unexpected error %v", err)
	}
	q.Shutdown()
	<-disconnectedCh
}

// Checks that the system_powerdown command is correctly sent.
//
// We start a QMPLoop, send the system_powerdown command and stop the loop.
//
// The system_powerdown command should be correctly sent and should return
// as we've provisioned a SHUTDOWN event.  The QMP loop should exit gracefully.
func TestQMPSystemPowerdown(t *testing.T) {
	const (
		seconds         = 1352167040730
		microsecondsEv1 = 123456
	)

	var wg sync.WaitGroup
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	buf.AddCommand("system_powerdown", nil, "return", nil)
	buf.AddEvent("SHUTDOWN", time.Millisecond*100,
		nil,
		map[string]interface{}{
			"seconds":      seconds,
			"microseconds": microsecondsEv1,
		})
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	checkVersion(t, connectedCh)
	buf.startEventLoop(&wg)
	err := q.ExecuteSystemPowerdown(context.Background())
	if err != nil {
		t.Fatalf("Unexpected error %v", err)
	}
	q.Shutdown()
	<-disconnectedCh
	wg.Wait()
}

// Checks that event commands can be cancelled.
//
// We start a QMPLoop, send the system_powerdown command.  This command
// will time out after 1 second as the SHUTDOWN event never arrives.
// We then send a quit command to terminate the session.
//
// The system_powerdown command should be correctly sent but should block
// waiting for the SHUTDOWN event and should be successfully cancelled.
// The quit command should be successfully received and the QMP loop should
// exit gracefully.
func TestQMPEventedCommandCancel(t *testing.T) {
	var wg sync.WaitGroup
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	buf.AddCommand("system_powerdown", nil, "return", nil)
	buf.AddCommand("quit", nil, "return", nil)
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	checkVersion(t, connectedCh)
	buf.startEventLoop(&wg)
	ctx, cancelFN := context.WithTimeout(context.Background(), time.Second)
	err := q.ExecuteSystemPowerdown(ctx)
	cancelFN()
	if err == nil {
		t.Fatalf("Expected SystemPowerdown to fail")
	}
	err = q.ExecuteQuit(context.Background())
	if err != nil {
		t.Fatalf("Unexpected error %v", err)
	}
	q.Shutdown()
	<-disconnectedCh
	wg.Wait()
}

// Checks that queued commands execute after an evented command is cancelled.
//
// This test is similar to the previous test with the exception that it
// tries to ensure that a second command is placed on the QMP structure's
// command queue before the evented command is cancelled.  This allows us
// to test a slightly different use case. We start a QMPLoop, send the
// system_powerdown command.  We do this by sending the command directly
// down the QMP.cmdCh rather than calling a higher level function as this
// allows us to ensure that we have another command queued before we
// timeout the first command.  We then send a qmp_capabilities command and
// then we shutdown.
//
// The system_powerdown command should be correctly sent but should block
// waiting for the SHUTDOWN event and should be successfully cancelled.
// The query_capabilities command should be successfully received and the
// QMP loop should exit gracefully.
func TestQMPEventedCommandCancelConcurrent(t *testing.T) {
	var wg sync.WaitGroup
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)

	buf.AddCommand("system_powerdown", nil, "error", nil)
	buf.AddCommand("qmp_capabilities", nil, "return", nil)

	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	checkVersion(t, connectedCh)
	buf.startEventLoop(&wg)

	resCh := make(chan qmpResult)
	ctx, cancelFn := context.WithTimeout(context.Background(), time.Second)
	q.cmdCh <- qmpCommand{
		ctx:  ctx,
		res:  resCh,
		name: "system_powerdown",
		filter: &qmpEventFilter{
			eventName: "SHUTDOWN",
		},
	}

	var cmdWg sync.WaitGroup
	cmdWg.Add(1)
	go func() {
		err := q.ExecuteQMPCapabilities(context.Background())
		if err != nil {
			t.Errorf("Unexpected error %v", err)
		}
		cmdWg.Done()
	}()

	<-resCh
	cancelFn()
	cmdWg.Wait()
	q.Shutdown()
	<-disconnectedCh
	wg.Wait()
}

// Checks that events can be received and parsed.
//
// Two events are provisioned and the QMPLoop is started with an valid eventCh.
// We wait for both events to be received and check that their contents are
// correct.  We then shutdown the QMP loop.
//
// Both events are received and their contents are correct.  The QMP loop should
// shut down gracefully.
func TestQMPEvents(t *testing.T) {
	const (
		seconds         = 1352167040730
		microsecondsEv1 = 123456
		microsecondsEv2 = 123556
		device          = "device_" + testutil.VolumeUUID
		path            = "/dev/rbd0"
	)
	var wg sync.WaitGroup
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	buf.AddEvent("DEVICE_DELETED", time.Millisecond*100,
		map[string]interface{}{
			"device": device,
			"path":   path,
		},
		map[string]interface{}{
			"seconds":      seconds,
			"microseconds": microsecondsEv1,
		})
	buf.AddEvent("POWERDOWN", time.Millisecond*200, nil,
		map[string]interface{}{
			"seconds":      seconds,
			"microseconds": microsecondsEv2,
		})
	eventCh := make(chan QMPEvent)
	cfg := QMPConfig{EventCh: eventCh, Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	checkVersion(t, connectedCh)
	buf.startEventLoop(&wg)

	ev := <-eventCh
	if ev.Name != "DEVICE_DELETED" {
		t.Errorf("incorrect event name received.  Expected %s, found %s",
			"DEVICE_DELETED", ev.Name)
	}
	if ev.Timestamp != time.Unix(seconds, microsecondsEv1) {
		t.Error("incorrect timestamp")
	}
	deviceName := ev.Data["device"].(string)
	if deviceName != device {
		t.Errorf("Unexpected device field.  Expected %s, found %s",
			"device_"+testutil.VolumeUUID, device)
	}
	pathName := ev.Data["path"].(string)
	if pathName != path {
		t.Errorf("Unexpected path field.  Expected %s, found %s",
			"/dev/rbd0", path)
	}

	ev = <-eventCh
	if ev.Name != "POWERDOWN" {
		t.Errorf("incorrect event name received.  Expected %s, found %s",
			"POWERDOWN", ev.Name)
	}
	if ev.Timestamp != time.Unix(seconds, microsecondsEv2) {
		t.Error("incorrect timestamp")
	}
	if ev.Data != nil {
		t.Errorf("event data expected to be nil")
	}

	q.Shutdown()

	select {
	case _, ok := <-eventCh:
		if ok {
			t.Errorf("Expected eventCh to be closed")
		}
	case <-time.After(time.Second):
		t.Error("Timed out waiting for eventCh to close")
	}

	<-disconnectedCh
	wg.Wait()
}

// Checks that commands issued after the QMP loop exits fail (and don't hang)
//
// We start the QMP loop but force it to fail immediately simulating a QEMU
// instance exit.  We then send two qmp_cabilities commands.
//
// Both commands should fail with an error.  The QMP loop should exit.
func TestQMPLostLoop(t *testing.T) {
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)

	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	checkVersion(t, connectedCh)
	close(buf.forceFail)
	buf.AddCommand("qmp_capabilities", nil, "return", nil)
	err := q.ExecuteQMPCapabilities(context.Background())
	if err == nil {
		t.Error("Expected executeQMPCapabilities to fail")
	}
	<-disconnectedCh
	buf.AddCommand("qmp_capabilities", nil, "return", nil)
	err = q.ExecuteQMPCapabilities(context.Background())
	if err == nil {
		t.Error("Expected executeQMPCapabilities to fail")
	}
}
