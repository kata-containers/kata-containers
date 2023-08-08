// Copyright contributors to the Virtual Machine Manager for Go project
//
// SPDX-License-Identifier: Apache-2.0
//

package qemu

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"log"
	"os"
	"reflect"
	"sync"
	"testing"
	"time"

	"context"
)

const (
	microStr = "50"
	minorStr = "9"
	majorStr = "2"
	micro    = 50
	minor    = 9
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
	l.Infof(format, v...)
}

func (l qmpTestLogger) Errorf(format string, v ...interface{}) {
	l.Infof(format, v...)
}

// nolint: govet
type qmpTestCommand struct {
	name string
	args map[string]interface{}
}

// nolint: govet
type qmpTestEvent struct {
	name      string
	data      map[string]interface{}
	timestamp map[string]interface{}
	after     time.Duration
}

// nolint: govet
type qmpTestResult struct {
	result string
	data   interface{}
}

// nolint: govet
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

func newQMPTestCommandBufferNoGreeting(t *testing.T) *qmpTestCommandBuffer {
	b := &qmpTestCommandBuffer{
		newDataCh: make(chan []byte, 1),
		t:         t,
		buf:       bytes.NewBuffer([]byte{}),
		forceFail: make(chan struct{}),
	}
	b.cmds = make([]qmpTestCommand, 0, 8)
	b.events = make([]qmpTestEvent, 0, 8)
	b.results = make([]qmpTestResult, 0, 8)
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
	result string, data interface{}) {
	b.cmds = append(b.cmds, qmpTestCommand{name, args})
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

// Checks that a call to QMPStart with an invalid path exits gracefully.
//
// We call QMPStart with an invalid path.
//
// An error should be returned and the disconnected channel should be closed.
func TestQMPStartBadPath(t *testing.T) {
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	disconnectedCh := make(chan struct{})
	q, _, err := QMPStart(context.Background(), "", cfg, disconnectedCh)
	if err == nil {
		t.Errorf("Expected error")
		q.Shutdown()
	}
	<-disconnectedCh
}

// Checks that a call to QMPStartWithConn with a nil connection exits gracefully.
//
// We call QMPStartWithConn with a nil connection.
//
// An error should be returned and the disconnected channel should be closed.
func TestQMPStartWithConnNil(t *testing.T) {
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	disconnectedCh := make(chan struct{})
	q, _, err := QMPStartWithConn(context.Background(), nil, cfg, disconnectedCh)
	if err == nil {
		t.Errorf("Expected error")
		q.Shutdown()
	}
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

// Checks that an error returned by a QMP command is correctly handled.
//
// We start a QMPLoop, send the qmp_capabilities command and stop the
// loop.
//
// The qmp_capabilities command fails and yet we should exit gracefully.
func TestQMPBadCapabilities(t *testing.T) {
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	buf.AddCommand("qmp_capabilities", nil, "error", nil)
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	checkVersion(t, connectedCh)
	err := q.ExecuteQMPCapabilities(context.Background())
	if err == nil {
		t.Fatalf("Expected error")
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
	dev := BlockDevice{
		ID:       fmt.Sprintf("drive_%s", volumeUUID),
		File:     "/dev/rbd0",
		ReadOnly: false,
		AIO:      Native,
	}
	err := q.ExecuteBlockdevAdd(context.Background(), &dev)
	if err != nil {
		t.Fatalf("Unexpected error %v", err)
	}
	q.Shutdown()
	<-disconnectedCh
}

// Checks that the blockdev-add with cache options command is correctly sent.
//
// We start a QMPLoop, send the blockdev-add with cache options
// command and stop the loop.
//
// The blockdev-add with cache options command should be correctly sent and
// the QMP loop should exit gracefully.
func TestQMPBlockdevAddWithCache(t *testing.T) {
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	buf.AddCommand("blockdev-add", nil, "return", nil)
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	q.version = checkVersion(t, connectedCh)
	dev := BlockDevice{
		ID:       fmt.Sprintf("drive_%s", volumeUUID),
		File:     "/dev/rbd0",
		ReadOnly: false,
		AIO:      Native,
	}
	err := q.ExecuteBlockdevAddWithCache(context.Background(), &dev, true, true)
	if err != nil {
		t.Fatalf("Unexpected error %v", err)
	}
	q.Shutdown()
	<-disconnectedCh
}

// Checks that the netdev_add command is correctly sent.
//
// We start a QMPLoop, send the netdev_add command and stop the loop.
//
// The netdev_add command should be correctly sent and the QMP loop should
// exit gracefully.
func TestQMPNetdevAdd(t *testing.T) {
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	buf.AddCommand("netdev_add", nil, "return", nil)
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	q.version = checkVersion(t, connectedCh)
	err := q.ExecuteNetdevAdd(context.Background(), "tap", "br0", "tap0", "no", "no", 8)
	if err != nil {
		t.Fatalf("Unexpected error %v", err)
	}
	q.Shutdown()
	<-disconnectedCh
}

// Checks that the netdev_add command is correctly sent.
//
// We start a QMPLoop, send the netdev_add command and stop the loop.
//
// The netdev_add command should be correctly sent and the QMP loop should
// exit gracefully.
func TestQMPNetdevChardevAdd(t *testing.T) {
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	buf.AddCommand("netdev_add", nil, "return", nil)
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	q.version = checkVersion(t, connectedCh)
	err := q.ExecuteNetdevChardevAdd(context.Background(), "tap", "br0", "chr0", 8)
	if err != nil {
		t.Fatalf("Unexpected error %v", err)
	}
	q.Shutdown()
	<-disconnectedCh
}

// Checks that the netdev_add command with fds is correctly sent.
//
// We start a QMPLoop, send the netdev_add command with fds and stop the loop.
//
// The netdev_add command with fds should be correctly sent and the QMP loop should
// exit gracefully.
func TestQMPNetdevAddByFds(t *testing.T) {
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	buf.AddCommand("netdev_add", nil, "return", nil)
	buf.AddCommand("netdev_add", nil, "return", nil)
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	q.version = checkVersion(t, connectedCh)
	err := q.ExecuteNetdevAddByFds(context.Background(), "tap", "br0", nil, []string{})
	if err != nil {
		t.Fatalf("Unexpected error %v", err)
	}
	err = q.ExecuteNetdevAddByFds(context.Background(), "tap", "br1", nil, []string{"3"})
	if err != nil {
		t.Fatalf("Unexpected error %v", err)
	}
	q.Shutdown()
	<-disconnectedCh
}

// Checks that the netdev_del command is correctly sent.
//
// We start a QMPLoop, send the netdev_del command and stop the loop.
//
// The netdev_del command should be correctly sent and the QMP loop should
// exit gracefully.
func TestQMPNetdevDel(t *testing.T) {
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	buf.AddCommand("netdev_del", nil, "return", nil)
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	q.version = checkVersion(t, connectedCh)
	err := q.ExecuteNetdevDel(context.Background(), "br0")
	if err != nil {
		t.Fatalf("Unexpected error %v", err)
	}
	q.Shutdown()
	<-disconnectedCh
}

func TestQMPNetPCIDeviceAdd(t *testing.T) {
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	buf.AddCommand("device_add", nil, "return", nil)
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	checkVersion(t, connectedCh)
	err := q.ExecuteNetPCIDeviceAdd(context.Background(), "br0", "virtio-0", "02:42:ac:11:00:02", "0x7", "", "", 8, false)
	if err != nil {
		t.Fatalf("Unexpected error %v", err)
	}
	q.Shutdown()
	<-disconnectedCh
}

func TestQMPNetCCWDeviceAdd(t *testing.T) {
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	buf.AddCommand("device_add", nil, "return", nil)
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	checkVersion(t, connectedCh)
	err := q.ExecuteNetCCWDeviceAdd(context.Background(), "br0", "virtio-0", "02:42:ac:11:00:02", DevNo, 8)
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
	q.version = checkVersion(t, connectedCh)
	blockdevID := fmt.Sprintf("drive_%s", volumeUUID)
	devID := fmt.Sprintf("device_%s", volumeUUID)
	err := q.ExecuteDeviceAdd(context.Background(), blockdevID, devID,
		"virtio-blk-pci", "", "", true, false)
	if err != nil {
		t.Fatalf("Unexpected error %v", err)
	}
	q.Shutdown()
	<-disconnectedCh
}

// Checks that the device_add command for scsi is correctly sent.
//
// We start a QMPLoop, send the device_add command and stop the loop.
//
// The device_add command should be correctly sent and the QMP loop should
// exit gracefully.
func TestQMPSCSIDeviceAdd(t *testing.T) {
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	buf.AddCommand("device_add", nil, "return", nil)
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	q.version = checkVersion(t, connectedCh)
	blockdevID := fmt.Sprintf("drive_%s", volumeUUID)
	devID := fmt.Sprintf("device_%s", volumeUUID)
	err := q.ExecuteSCSIDeviceAdd(context.Background(), blockdevID, devID,
		"scsi-hd", "scsi0.0", "", 1, 2, true, false)
	if err != nil {
		t.Fatalf("Unexpected error %v", err)
	}
	q.Shutdown()
	<-disconnectedCh
}

// Checks that the blockdev-del command is correctly sent.
//
// We start a QMPLoop, send the blockdev-del command and stop the loop.
//
// The blockdev-del command should be correctly sent and the QMP loop should
// exit gracefully.
func TestQMPBlockdevDel(t *testing.T) {
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	buf.AddCommand("blockdev-del", nil, "return", nil)
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	q.version = checkVersion(t, connectedCh)
	err := q.ExecuteBlockdevDel(context.Background(),
		fmt.Sprintf("drive_%s", volumeUUID))
	if err != nil {
		t.Fatalf("Unexpected error %v", err)
	}
	q.Shutdown()
	<-disconnectedCh
}

// Checks that the chardev-remove command is correctly sent.
//
// We start a QMPLoop, send the chardev-remove command and stop the loop.
//
// The chardev-remove command should be correctly sent and the QMP loop should
// exit gracefully.
func TestQMPChardevDel(t *testing.T) {
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	buf.AddCommand("chardev-remove", nil, "return", nil)
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	q.version = checkVersion(t, connectedCh)
	err := q.ExecuteChardevDel(context.Background(), "chardev-0")
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
		seconds         = int64(1352167040730)
		microsecondsEv1 = 123456
		microsecondsEv2 = 123556
		device          = "device_" + volumeUUID
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
		fmt.Sprintf("device_%s", volumeUUID))
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
		fmt.Sprintf("device_%s", volumeUUID))
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
		seconds         = int64(1352167040730)
		microsecondsEv1 = 123456
	)

	var wg sync.WaitGroup
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	buf.AddCommand("system_powerdown", nil, "return", nil)
	buf.AddEvent("POWERDOWN", time.Millisecond*100,
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
		seconds         = int64(1352167040730)
		microsecondsEv1 = 123456
		microsecondsEv2 = 123556
		device          = "device_" + volumeUUID
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
			"device_"+volumeUUID, device)
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

// Checks that PCI devices are correctly added using device_add.
//
// We start a QMPLoop, send the device_add command and stop the loop.
//
// The device_add command should be correctly sent and the QMP loop should
// exit gracefully.
func TestQMPPCIDeviceAdd(t *testing.T) {
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	buf.AddCommand("device_add", nil, "return", nil)
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	q.version = checkVersion(t, connectedCh)
	blockdevID := fmt.Sprintf("drive_%s", volumeUUID)
	devID := fmt.Sprintf("device_%s", volumeUUID)
	err := q.ExecutePCIDeviceAdd(context.Background(), blockdevID, devID,
		"virtio-blk-pci", "0x1", "", "", 1, true, false)
	if err != nil {
		t.Fatalf("Unexpected error %v", err)
	}
	q.Shutdown()
	<-disconnectedCh
}

// Checks that PCI VFIO mediated devices are correctly added using device_add.
//
// We start a QMPLoop, send the device_add command and stop the loop.
//
// The device_add command should be correctly sent and the QMP loop should
// exit gracefully.
func TestQMPPCIVFIOMediatedDeviceAdd(t *testing.T) {
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	buf.AddCommand("device_add", nil, "return", nil)
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	checkVersion(t, connectedCh)
	sysfsDev := "/sys/bus/pci/devices/0000:00:02.0/a297db4a-f4c2-11e6-90f6-d3b88d6c9525"
	devID := fmt.Sprintf("device_%s", volumeUUID)
	err := q.ExecutePCIVFIOMediatedDeviceAdd(context.Background(), devID, sysfsDev, "0x1", "", "")
	if err != nil {
		t.Fatalf("Unexpected error %v", err)
	}
	q.Shutdown()
	<-disconnectedCh
}

func TestQMPPCIVFIOPCIeDeviceAdd(t *testing.T) {
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	buf.AddCommand("device_add", nil, "return", nil)
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	checkVersion(t, connectedCh)
	bdf := "04:00.0"
	bus := "rp0"
	addr := "0x1"
	romfile := ""
	devID := fmt.Sprintf("device_%s", volumeUUID)
	err := q.ExecutePCIVFIODeviceAdd(context.Background(), devID, bdf, addr, bus, romfile)
	if err != nil {
		t.Fatalf("Unexpected error %v", err)
	}
	q.Shutdown()
	<-disconnectedCh
}

func TestQMPAPVFIOMediatedDeviceAdd(t *testing.T) {
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	buf.AddCommand("device_add", nil, "return", nil)
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	checkVersion(t, connectedCh)
	sysfsDev := "/sys/devices/vfio_ap/matrix/a297db4a-f4c2-11e6-90f6-d3b88d6c9525"
	err := q.ExecuteAPVFIOMediatedDeviceAdd(context.Background(), sysfsDev, "test-id")
	if err != nil {
		t.Fatalf("Unexpected error %v", err)
	}
	q.Shutdown()
	<-disconnectedCh
}

// Checks that CPU are correctly added using device_add
func TestQMPCPUDeviceAdd(t *testing.T) {
	drivers := []string{"host-x86_64-cpu", "host-s390x-cpu", "host-powerpc64-cpu"}
	cpuID := "cpu-0"
	socketID := "0"
	dieID := "0"
	coreID := "1"
	threadID := "0"
	for _, d := range drivers {
		connectedCh := make(chan *QMPVersion)
		disconnectedCh := make(chan struct{})
		buf := newQMPTestCommandBuffer(t)
		buf.AddCommand("device_add", nil, "return", nil)
		cfg := QMPConfig{Logger: qmpTestLogger{}}
		q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
		checkVersion(t, connectedCh)
		err := q.ExecuteCPUDeviceAdd(context.Background(), d, cpuID, socketID, dieID, coreID, threadID, "")
		if err != nil {
			t.Fatalf("Unexpected error %v", err)
		}
		q.Shutdown()
		<-disconnectedCh
	}
}

// Checks that hotpluggable CPUs are listed correctly
func TestQMPExecuteQueryHotpluggableCPUs(t *testing.T) {
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	hotCPU := HotpluggableCPU{
		Type:       "host-x86",
		VcpusCount: 5,
		Properties: CPUProperties{
			Node:   1,
			Socket: 3,
			Die:    1,
			Core:   2,
			Thread: 4,
		},
		QOMPath: "/abc/123/rgb",
	}
	buf.AddCommand("query-hotpluggable-cpus", nil, "return", []interface{}{hotCPU})
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	checkVersion(t, connectedCh)
	hotCPUs, err := q.ExecuteQueryHotpluggableCPUs(context.Background())
	if err != nil {
		t.Fatalf("Unexpected error: %v", err)
	}
	if len(hotCPUs) != 1 {
		t.Fatalf("Expected hot CPUs length equals to 1\n")
	}
	if reflect.DeepEqual(hotCPUs[0], hotCPU) == false {
		t.Fatalf("Expected %v equals to %v", hotCPUs[0], hotCPU)
	}
	q.Shutdown()
	<-disconnectedCh
}

// Checks that memory devices are listed correctly
func TestQMPExecuteQueryMemoryDevices(t *testing.T) {
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	memoryDevices := MemoryDevices{
		Type: "dimm",
		Data: MemoryDevicesData{
			Slot:         1,
			Node:         0,
			Addr:         1234,
			Memdev:       "dimm1",
			ID:           "mem1",
			Hotpluggable: true,
			Hotplugged:   false,
			Size:         1234,
		},
	}
	buf.AddCommand("query-memory-devices", nil, "return", []interface{}{memoryDevices})
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	checkVersion(t, connectedCh)
	memDevices, err := q.ExecQueryMemoryDevices(context.Background())
	if err != nil {
		t.Fatalf("Unexpected error: %v", err)
	}
	if len(memDevices) != 1 {
		t.Fatalf("Expected memory devices length equals to 1\n")
	}
	if reflect.DeepEqual(memDevices[0], memoryDevices) == false {
		t.Fatalf("Expected %v equals to %v", memDevices[0], memoryDevices)
	}
	q.Shutdown()
	<-disconnectedCh
}

// Checks that cpus are listed correctly
func TestQMPExecuteQueryCpus(t *testing.T) {
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	cpuInfo := CPUInfo{
		CPU:      1,
		Current:  false,
		Halted:   false,
		Arch:     "x86_64",
		QomPath:  "/tmp/testQom",
		Pc:       123456,
		ThreadID: 123457,
		Props: CPUProperties{
			Node:   0,
			Socket: 1,
			Core:   1,
			Thread: 1966,
		},
	}
	buf.AddCommand("query-cpus", nil, "return", []interface{}{cpuInfo})
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	checkVersion(t, connectedCh)
	cpus, err := q.ExecQueryCpus(context.Background())
	if err != nil {
		t.Fatalf("Unexpected error: %v", err)
	}
	if len(cpus) != 1 {
		t.Fatalf("Expected memory devices length equals to 1\n")
	}
	if reflect.DeepEqual(cpus[0], cpuInfo) == false {
		t.Fatalf("Expected %v equals to %v", cpus[0], cpuInfo)
	}
	q.Shutdown()
	<-disconnectedCh
}

// Checks that cpus are listed correctly
func TestQMPExecuteQueryCpusFast(t *testing.T) {
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	cpuInfoFast := CPUInfoFast{
		CPUIndex: 1,
		Arch:     "x86",
		Target:   "x86_64",
		QomPath:  "/tmp/testQom",
		ThreadID: 123457,
		Props: CPUProperties{
			Node:   0,
			Socket: 1,
			Core:   1,
			Thread: 1966,
		},
	}
	buf.AddCommand("query-cpus-fast", nil, "return", []interface{}{cpuInfoFast})
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	checkVersion(t, connectedCh)
	cpus, err := q.ExecQueryCpusFast(context.Background())
	if err != nil {
		t.Fatalf("Unexpected error: %v", err)
	}
	if len(cpus) != 1 {
		t.Fatalf("Expected memory devices length equals to 1\n")
	}
	if reflect.DeepEqual(cpus[0], cpuInfoFast) == false {
		t.Fatalf("Expected %v equals to %v", cpus[0], cpuInfoFast)
	}
	q.Shutdown()
	<-disconnectedCh
}

// Checks that migrate capabilities can be set
func TestExecSetMigrationCaps(t *testing.T) {
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	buf.AddCommand("migrate-set-capabilities", nil, "return", nil)
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	checkVersion(t, connectedCh)
	caps := []map[string]interface{}{
		{
			"capability": "bypass-shared-memory",
			"state":      true,
		},
	}
	err := q.ExecSetMigrationCaps(context.Background(), caps)
	if err != nil {
		t.Fatalf("Unexpected error: %v", err)
	}
	q.Shutdown()
	<-disconnectedCh
}

// Checks that migrate arguments can be set
func TestExecSetMigrateArguments(t *testing.T) {
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	buf.AddCommand("migrate", nil, "return", nil)
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	checkVersion(t, connectedCh)
	err := q.ExecSetMigrateArguments(context.Background(), "exec:foobar")
	if err != nil {
		t.Fatalf("Unexpected error: %v", err)
	}
	q.Shutdown()
	<-disconnectedCh
}

// Checks add memory device
func TestExecMemdevAdd(t *testing.T) {
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	buf.AddCommand("object-add", nil, "return", nil)
	buf.AddCommand("device_add", nil, "return", nil)
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	checkVersion(t, connectedCh)
	err := q.ExecMemdevAdd(context.Background(), "memory-backend-ram", "mem0", "", 128, true, "virtio-mem-pci", "virtiomem0", "0x1", "pci-bridge-0")
	if err != nil {
		t.Fatalf("Unexpected error: %v", err)
	}
	q.Shutdown()
	<-disconnectedCh
}

// Checks hotplug memory
func TestExecHotplugMemory(t *testing.T) {
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	buf.AddCommand("object-add", nil, "return", nil)
	buf.AddCommand("device_add", nil, "return", nil)
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	checkVersion(t, connectedCh)
	err := q.ExecHotplugMemory(context.Background(), "memory-backend-ram", "mem0", "", 128, true)
	if err != nil {
		t.Fatalf("Unexpected error: %v", err)
	}
	q.Shutdown()
	<-disconnectedCh
}

// Checks vsock-pci hotplug
func TestExecutePCIVSockAdd(t *testing.T) {
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	buf.AddCommand("device_add", nil, "return", nil)
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	checkVersion(t, connectedCh)
	err := q.ExecutePCIVSockAdd(context.Background(), "vsock-pci0", "3", "1", "1", "1", "", true)
	if err != nil {
		t.Fatalf("Unexpected error %v", err)
	}
	q.Shutdown()
	<-disconnectedCh
}

// Checks vhost-user-pci hotplug
func TestExecutePCIVhostUserDevAdd(t *testing.T) {
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	buf.AddCommand("device_add", nil, "return", nil)
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	checkVersion(t, connectedCh)
	driver := "vhost-user-blk-pci"
	devID := "vhost-user-blk0"
	chardevID := "vhost-user-blk-char0"
	err := q.ExecutePCIVhostUserDevAdd(context.Background(), driver, devID, chardevID, "1", "1")
	if err != nil {
		t.Fatalf("Unexpected error %v", err)
	}
	q.Shutdown()
	<-disconnectedCh
}

// Checks getfd
func TestExecuteGetFdD(t *testing.T) {
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	buf.AddCommand("getfd", nil, "return", nil)
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	checkVersion(t, connectedCh)
	err := q.ExecuteGetFD(context.Background(), "foo", os.NewFile(0, "foo"))
	if err != nil {
		t.Fatalf("Unexpected error %v", err)
	}
	q.Shutdown()
	<-disconnectedCh
}

// Checks chardev-add unix socket
func TestExecuteCharDevUnixSocketAdd(t *testing.T) {
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	buf.AddCommand("chardev-add", nil, "return", nil)
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	checkVersion(t, connectedCh)
	err := q.ExecuteCharDevUnixSocketAdd(context.Background(), "foo", "foo.sock", false, true, 1)
	if err != nil {
		t.Fatalf("Unexpected error %v", err)
	}
	q.Shutdown()
	<-disconnectedCh
}

// Checks virtio serial port hotplug
func TestExecuteVirtSerialPortAdd(t *testing.T) {
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	buf.AddCommand("device_add", nil, "return", nil)
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	checkVersion(t, connectedCh)
	err := q.ExecuteVirtSerialPortAdd(context.Background(), "foo", "foo.channel", "foo")
	if err != nil {
		t.Fatalf("Unexpected error %v", err)
	}
	q.Shutdown()
	<-disconnectedCh
}

// Check migration incoming
func TestExecuteMigrationIncoming(t *testing.T) {
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	buf.AddCommand("migrate-incoming", nil, "return", nil)
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	checkVersion(t, connectedCh)
	err := q.ExecuteMigrationIncoming(context.Background(), "uri")
	if err != nil {
		t.Fatalf("Unexpected error %v", err)
	}
	q.Shutdown()
	<-disconnectedCh
}

// Checks migration status
func TestExecuteQueryMigration(t *testing.T) {
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	status := MigrationStatus{
		Status: "completed",
		RAM: MigrationRAM{
			Total:            100,
			Remaining:        101,
			Transferred:      101,
			TotalTime:        101,
			SetupTime:        101,
			ExpectedDowntime: 101,
			Duplicate:        101,
			Normal:           101,
			NormalBytes:      101,
			DirtySyncCount:   101,
		},
		Disk: MigrationDisk{
			Total:       200,
			Remaining:   200,
			Transferred: 200,
		},
		XbzrleCache: MigrationXbzrleCache{
			CacheSize:     300,
			Bytes:         300,
			Pages:         300,
			CacheMiss:     300,
			CacheMissRate: 300,
			Overflow:      300,
		},
	}
	caps := map[string]interface{}{"foo": true}
	status.Capabilities = append(status.Capabilities, caps)
	buf.AddCommand("query-migrate", nil, "return", interface{}(status))
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	checkVersion(t, connectedCh)
	s, err := q.ExecuteQueryMigration(context.Background())
	if err != nil {
		t.Fatalf("Unexpected error %v", err)
	}
	if !reflect.DeepEqual(s, status) {
		t.Fatalf("expected %v got %v", status, s)
	}
	q.Shutdown()
	<-disconnectedCh
}

// Checks balloon
func TestExecuteBalloon(t *testing.T) {
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	buf.AddCommand("balloon", nil, "return", nil)
	cfg := QMPConfig{Logger: qmpTestLogger{}}

	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	checkVersion(t, connectedCh)
	err := q.ExecuteBalloon(context.Background(), 1073741824)
	if err != nil {
		t.Fatalf("Unexpected error %v", err)
	}
	q.Shutdown()
	<-disconnectedCh
}

func TestErrorDesc(t *testing.T) {
	errDesc := "Somthing err messages"
	errData := map[string]string{
		"class": "GenericError",
		"desc":  errDesc,
	}

	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	checkVersion(t, connectedCh)

	desc, err := q.errorDesc(errData)
	if err != nil {
		t.Fatalf("Unexpected error '%v'", err)
	}
	if desc != errDesc {
		t.Fatalf("expected '%v'\n got '%v'", errDesc, desc)
	}

	q.Shutdown()
	<-disconnectedCh
}

func TestExecCommandFailed(t *testing.T) {
	errDesc := "unable to map backing store for guest RAM: Cannot allocate memory"
	errData := map[string]string{
		"class": "GenericError",
		"desc":  errDesc,
	}

	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	buf.AddCommand("object-add", nil, "error", errData)
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	checkVersion(t, connectedCh)

	_, err := q.executeCommandWithResponse(context.Background(), "object-add", nil, nil, nil)
	if err == nil {
		t.Fatalf("expected error but got nil")
	}

	expectedString := "QMP command failed: " + errDesc
	if err.Error() != expectedString {
		t.Fatalf("expected '%v' but got '%v'", expectedString, err)
	}

	q.Shutdown()
	<-disconnectedCh
}

func TestExecCommandFailedWithInnerError(t *testing.T) {
	errData := map[string]string{
		"class":            "GenericError",
		"descFieldInvalid": "Invalid",
	}

	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	buf.AddCommand("object-add", nil, "error", errData)
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	checkVersion(t, connectedCh)

	_, err := q.executeCommandWithResponse(context.Background(), "object-add", nil, nil, nil)
	if err == nil {
		t.Fatalf("expected error but got nil")
	}

	expectedString := "QMP command failed: "
	if err.Error() != expectedString {
		t.Fatalf("expected '%v' but got '%v'", expectedString, err)
	}

	q.Shutdown()
	<-disconnectedCh
}

// Checks NVDIMM device add
func TestExecuteNVDIMMDeviceAdd(t *testing.T) {
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	buf.AddCommand("object-add", nil, "return", nil)
	buf.AddCommand("device_add", nil, "return", nil)
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	checkVersion(t, connectedCh)
	pmem := true
	err := q.ExecuteNVDIMMDeviceAdd(context.Background(), "nvdimm0", "/dev/rbd0", 1024, &pmem)
	if err != nil {
		t.Fatalf("Unexpected error: %v", err)
	}
	q.Shutdown()
	<-disconnectedCh
}

func TestMainLoopEventBeforeGreeting(t *testing.T) {
	const (
		seconds      = int64(1352167040730)
		microseconds = 123456
	)

	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBufferNoGreeting(t)

	// Add events
	var wg sync.WaitGroup
	buf.AddEvent("VSERPORT_CHANGE", time.Millisecond*100,
		map[string]interface{}{
			"open": false,
			"id":   "channel0",
		},
		map[string]interface{}{
			"seconds":      seconds,
			"microseconds": microseconds,
		})
	buf.AddEvent("POWERDOWN", time.Millisecond*200, nil,
		map[string]interface{}{
			"seconds":      seconds,
			"microseconds": microseconds,
		})

	// register a channel to receive events
	eventCh := make(chan QMPEvent)
	cfg := QMPConfig{EventCh: eventCh, Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)

	// Start events, this will lead to a deadlock if mainLoop is not implemented
	// correctly
	buf.startEventLoop(&wg)
	wg.Wait()

	// Send greeting and check version
	buf.newDataCh <- []byte(qmpHello)
	checkVersion(t, connectedCh)

	q.Shutdown()
	<-disconnectedCh
}

func TestQMPExecQueryQmpSchema(t *testing.T) {
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	schemaInfo := []SchemaInfo{
		{
			MetaType: "command",
			Name:     "object-add",
		},
		{
			MetaType: "event",
			Name:     "VSOCK_RUNNING",
		},
	}
	buf.AddCommand("query-qmp-schema", nil, "return", schemaInfo)
	cfg := QMPConfig{
		Logger:      qmpTestLogger{},
		MaxCapacity: 1024,
	}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	checkVersion(t, connectedCh)
	info, err := q.ExecQueryQmpSchema(context.Background())
	if err != nil {
		t.Fatalf("Unexpected error: %v", err)
	}
	if len(schemaInfo) != 2 {
		t.Fatalf("Expected schema infos length equals to 2\n")
	}
	if reflect.DeepEqual(info, schemaInfo) == false {
		t.Fatalf("Expected %v equals to %v", info, schemaInfo)
	}
	q.Shutdown()
	<-disconnectedCh
}

func TestQMPExecQueryQmpStatus(t *testing.T) {
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	statusInfo := StatusInfo{
		Running:    true,
		SingleStep: false,
		Status:     "running",
	}
	buf.AddCommand("query-status", nil, "return", statusInfo)
	cfg := QMPConfig{
		Logger:      qmpTestLogger{},
		MaxCapacity: 1024,
	}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	checkVersion(t, connectedCh)
	info, err := q.ExecuteQueryStatus(context.Background())
	if err != nil {
		t.Fatalf("Unexpected error: %v", err)
	}
	if reflect.DeepEqual(info, statusInfo) == false {
		t.Fatalf("Expected %v equals to %v", info, statusInfo)
	}
	q.Shutdown()
	<-disconnectedCh
}

// Checks qom-set
func TestExecQomSet(t *testing.T) {
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	buf.AddCommand("qom-set", nil, "return", nil)
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	checkVersion(t, connectedCh)
	err := q.ExecQomSet(context.Background(), "virtiomem0", "requested-size", 1024)
	if err != nil {
		t.Fatalf("Unexpected error: %v", err)
	}
	q.Shutdown()
	<-disconnectedCh
}

// Checks qom-get
func TestExecQomGet(t *testing.T) {
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	buf.AddCommand("qom-get", nil, "return", "container")
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	checkVersion(t, connectedCh)
	val, err := q.ExecQomGet(context.Background(), "/", "type")
	if err != nil {
		t.Fatalf("Unexpected error: %v", err)
	}
	vals, ok := val.(string)
	if !ok {
		t.Fatalf("Unexpected type in qom-get")
	}
	if vals != "container" {
		t.Fatalf("Unpexected value in qom-get")
	}
	q.Shutdown()
	<-disconnectedCh
}

// Checks dump-guest-memory
func TestExecuteDumpGuestMemory(t *testing.T) {
	connectedCh := make(chan *QMPVersion)
	disconnectedCh := make(chan struct{})
	buf := newQMPTestCommandBuffer(t)
	buf.AddCommand("dump-guest-memory", nil, "return", nil)
	cfg := QMPConfig{Logger: qmpTestLogger{}}
	q := startQMPLoop(buf, cfg, connectedCh, disconnectedCh)
	checkVersion(t, connectedCh)

	err := q.ExecuteDumpGuestMemory(context.Background(), "file:/tmp/dump.xxx.yyy", false, "elf")
	if err != nil {
		t.Fatalf("Unexpected error: %v", err)
	}

	q.Shutdown()
	<-disconnectedCh
}
