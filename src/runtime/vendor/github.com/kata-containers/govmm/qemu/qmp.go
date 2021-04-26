/*
// Copyright contributors to the Virtual Machine Manager for Go project
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
	"bufio"
	"container/list"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net"
	"os"
	"strconv"
	"syscall"
	"time"

	"context"
	"strings"
)

// QMPLog is a logging interface used by the qemu package to log various
// interesting pieces of information.  Rather than introduce a dependency
// on a given logging package, qemu presents this interface that allows
// clients to provide their own logging type which they can use to
// seamlessly integrate qemu's logs into their own logs.  A QMPLog
// implementation can be specified in the QMPConfig structure.
type QMPLog interface {
	// V returns true if the given argument is less than or equal
	// to the implementation's defined verbosity level.
	V(int32) bool

	// Infof writes informational output to the log.  A newline will be
	// added to the output if one is not provided.
	Infof(string, ...interface{})

	// Warningf writes warning output to the log.  A newline will be
	// added to the output if one is not provided.
	Warningf(string, ...interface{})

	// Errorf writes error output to the log.  A newline will be
	// added to the output if one is not provided.
	Errorf(string, ...interface{})
}

type qmpNullLogger struct{}

func (l qmpNullLogger) V(level int32) bool {
	return false
}

func (l qmpNullLogger) Infof(format string, v ...interface{}) {
}

func (l qmpNullLogger) Warningf(format string, v ...interface{}) {
}

func (l qmpNullLogger) Errorf(format string, v ...interface{}) {
}

// QMPConfig is a configuration structure that can be used to specify a
// logger and a channel to which logs and  QMP events are to be sent.  If
// neither of these fields are specified, or are set to nil, no logs will be
// written and no QMP events will be reported to the client.
type QMPConfig struct {
	// eventCh can be specified by clients who wish to receive QMP
	// events.
	EventCh chan<- QMPEvent

	// logger is used by the qmpStart function and all the go routines
	// it spawns to log information.
	Logger QMPLog

	// specify the capacity of buffer used by receive QMP response.
	MaxCapacity int
}

type qmpEventFilter struct {
	eventName string
	dataKey   string
	dataValue string
}

// QMPEvent contains a single QMP event, sent on the QMPConfig.EventCh channel.
type QMPEvent struct {
	// The name of the event, e.g., DEVICE_DELETED
	Name string

	// The data associated with the event.  The contents of this map are
	// unprocessed by the qemu package.  It is simply the result of
	// unmarshalling the QMP json event.  Here's an example map
	// map[string]interface{}{
	//	"driver": "virtio-blk-pci",
	//	"drive":  "drive_3437843748734873483",
	// }
	Data map[string]interface{}

	// The event's timestamp converted to a time.Time object.
	Timestamp time.Time
}

type qmpResult struct {
	response interface{}
	err      error
}

type qmpCommand struct {
	ctx            context.Context
	res            chan qmpResult
	name           string
	args           map[string]interface{}
	filter         *qmpEventFilter
	resultReceived bool
	oob            []byte
}

// QMP is a structure that contains the internal state used by startQMPLoop and
// the go routines it spwans.  All the contents of this structure are private.
type QMP struct {
	cmdCh          chan qmpCommand
	conn           io.ReadWriteCloser
	cfg            QMPConfig
	connectedCh    chan<- *QMPVersion
	disconnectedCh chan struct{}
	version        *QMPVersion
}

// QMPVersion contains the version number and the capabailities of a QEMU
// instance, as reported in the QMP greeting message.
type QMPVersion struct {
	Major        int
	Minor        int
	Micro        int
	Capabilities []string
}

// CPUProperties contains the properties of a CPU instance
type CPUProperties struct {
	Node   int `json:"node-id"`
	Socket int `json:"socket-id"`
	Die    int `json:"die-id"`
	Core   int `json:"core-id"`
	Thread int `json:"thread-id"`
}

// HotpluggableCPU represents a hotpluggable CPU
type HotpluggableCPU struct {
	Type       string        `json:"type"`
	VcpusCount int           `json:"vcpus-count"`
	Properties CPUProperties `json:"props"`
	QOMPath    string        `json:"qom-path"`
}

// MemoryDevicesData cotains the data describes a memory device
type MemoryDevicesData struct {
	Slot         int    `json:"slot"`
	Node         int    `json:"node"`
	Addr         uint64 `json:"addr"`
	Memdev       string `json:"memdev"`
	ID           string `json:"id"`
	Hotpluggable bool   `json:"hotpluggable"`
	Hotplugged   bool   `json:"hotplugged"`
	Size         uint64 `json:"size"`
}

// MemoryDevices represents memory devices of vm
type MemoryDevices struct {
	Data MemoryDevicesData `json:"data"`
	Type string            `json:"type"`
}

// CPUInfo represents information about each virtual CPU
type CPUInfo struct {
	CPU      int           `json:"CPU"`
	Current  bool          `json:"current"`
	Halted   bool          `json:"halted"`
	QomPath  string        `json:"qom_path"`
	Arch     string        `json:"arch"`
	Pc       int           `json:"pc"`
	ThreadID int           `json:"thread_id"`
	Props    CPUProperties `json:"props"`
}

// CPUInfoFast represents information about each virtual CPU
type CPUInfoFast struct {
	CPUIndex int           `json:"cpu-index"`
	QomPath  string        `json:"qom-path"`
	Arch     string        `json:"arch"`
	ThreadID int           `json:"thread-id"`
	Target   string        `json:"target"`
	Props    CPUProperties `json:"props"`
}

// MigrationRAM represents migration ram status
type MigrationRAM struct {
	Total            int64 `json:"total"`
	Remaining        int64 `json:"remaining"`
	Transferred      int64 `json:"transferred"`
	TotalTime        int64 `json:"total-time"`
	SetupTime        int64 `json:"setup-time"`
	ExpectedDowntime int64 `json:"expected-downtime"`
	Duplicate        int64 `json:"duplicate"`
	Normal           int64 `json:"normal"`
	NormalBytes      int64 `json:"normal-bytes"`
	DirtySyncCount   int64 `json:"dirty-sync-count"`
}

// MigrationDisk represents migration disk status
type MigrationDisk struct {
	Total       int64 `json:"total"`
	Remaining   int64 `json:"remaining"`
	Transferred int64 `json:"transferred"`
}

// MigrationXbzrleCache represents migration XbzrleCache status
type MigrationXbzrleCache struct {
	CacheSize     int64 `json:"cache-size"`
	Bytes         int64 `json:"bytes"`
	Pages         int64 `json:"pages"`
	CacheMiss     int64 `json:"cache-miss"`
	CacheMissRate int64 `json:"cache-miss-rate"`
	Overflow      int64 `json:"overflow"`
}

// MigrationStatus represents migration status of a vm
type MigrationStatus struct {
	Status       string                   `json:"status"`
	Capabilities []map[string]interface{} `json:"capabilities,omitempty"`
	RAM          MigrationRAM             `json:"ram,omitempty"`
	Disk         MigrationDisk            `json:"disk,omitempty"`
	XbzrleCache  MigrationXbzrleCache     `json:"xbzrle-cache,omitempty"`
}

// SchemaInfo represents all QMP wire ABI
type SchemaInfo struct {
	MetaType string `json:"meta-type"`
	Name     string `json:"name"`
}

// StatusInfo represents guest running status
type StatusInfo struct {
	Running    bool   `json:"running"`
	SingleStep bool   `json:"singlestep"`
	Status     string `json:"status"`
}

func (q *QMP) readLoop(fromVMCh chan<- []byte) {
	scanner := bufio.NewScanner(q.conn)
	if q.cfg.MaxCapacity > 0 {
		buffer := make([]byte, q.cfg.MaxCapacity)
		scanner.Buffer(buffer, q.cfg.MaxCapacity)
	}

	for scanner.Scan() {
		line := scanner.Bytes()
		// Since []byte channel type transfer slice info(include slice underlying array pointer, len, cap)
		// between channel sender and receiver. scanner.Bytes() returned slice's underlying array
		// may point to data that will be overwritten by a subsequent call to Scan(reference from:
		// https://golang.org/pkg/bufio/#Scanner.Bytes), which may make receiver read mixed data,
		// so we need to copy line to new allocated space and then send to channel receiver
		sendLine := make([]byte, len(line))
		copy(sendLine, line)

		fromVMCh <- sendLine
	}
	q.cfg.Logger.Infof("scanner return error: %v", scanner.Err())
	close(fromVMCh)
}

func (q *QMP) processQMPEvent(cmdQueue *list.List, name interface{}, data interface{},
	timestamp interface{}) {

	strname, ok := name.(string)
	if !ok {
		return
	}

	var eventData map[string]interface{}
	if data != nil {
		eventData, _ = data.(map[string]interface{})
	}

	cmdEl := cmdQueue.Front()
	if cmdEl != nil {
		cmd := cmdEl.Value.(*qmpCommand)
		filter := cmd.filter
		if filter != nil {
			if filter.eventName == strname {
				match := filter.dataKey == ""
				if !match && eventData != nil {
					match = eventData[filter.dataKey] == filter.dataValue
				}
				if match {
					if cmd.resultReceived {
						q.finaliseCommand(cmdEl, cmdQueue, true)
					} else {
						cmd.filter = nil
					}
				}
			}
		}
	}

	if q.cfg.EventCh != nil {
		ev := QMPEvent{
			Name: strname,
			Data: eventData,
		}
		if timestamp != nil {
			timestamp, ok := timestamp.(map[string]interface{})
			if ok {
				seconds, _ := timestamp["seconds"].(float64)
				microseconds, _ := timestamp["microseconds"].(float64)
				ev.Timestamp = time.Unix(int64(seconds), int64(microseconds))
			}
		}

		q.cfg.EventCh <- ev
	}
}

func (q *QMP) finaliseCommandWithResponse(cmdEl *list.Element, cmdQueue *list.List, succeeded bool, response interface{}) {
	cmd := cmdEl.Value.(*qmpCommand)
	cmdQueue.Remove(cmdEl)
	select {
	case <-cmd.ctx.Done():
	default:
		if succeeded {
			cmd.res <- qmpResult{response: response}
		} else {
			cmd.res <- qmpResult{err: fmt.Errorf("QMP command failed: %v", response)}
		}
	}
	if cmdQueue.Len() > 0 {
		q.writeNextQMPCommand(cmdQueue)
	}
}

func (q *QMP) finaliseCommand(cmdEl *list.Element, cmdQueue *list.List, succeeded bool) {
	q.finaliseCommandWithResponse(cmdEl, cmdQueue, succeeded, nil)
}

func (q *QMP) errorDesc(errorData interface{}) (string, error) {
	// convert error to json
	data, err := json.Marshal(errorData)
	if err != nil {
		return "", fmt.Errorf("unable to extract error information: %v", err)
	}

	// see: https://github.com/qemu/qemu/blob/stable-2.12/qapi/qmp-dispatch.c#L125
	var qmpErr map[string]string
	// convert json to qmpError
	if err = json.Unmarshal(data, &qmpErr); err != nil {
		return "", fmt.Errorf("unable to convert json to qmpError: %v", err)
	}

	return qmpErr["desc"], nil
}

func (q *QMP) processQMPInput(line []byte, cmdQueue *list.List) {
	var vmData map[string]interface{}
	err := json.Unmarshal(line, &vmData)
	if err != nil {
		q.cfg.Logger.Warningf("Unable to decode response [%s] from VM: %v",
			string(line), err)
		return
	}
	if evname, found := vmData["event"]; found {
		q.processQMPEvent(cmdQueue, evname, vmData["data"], vmData["timestamp"])
		return
	}

	response, succeeded := vmData["return"]
	errData, failed := vmData["error"]

	if !succeeded && !failed {
		return
	}

	cmdEl := cmdQueue.Front()
	if cmdEl == nil {
		q.cfg.Logger.Warningf("Unexpected command response received [%s] from VM",
			string(line))
		return
	}
	cmd := cmdEl.Value.(*qmpCommand)
	if failed || cmd.filter == nil {
		if errData != nil {
			desc, err := q.errorDesc(errData)
			if err != nil {
				q.cfg.Logger.Infof("Get error description failed: %v", err)
			} else {
				response = desc
			}
		}
		q.finaliseCommandWithResponse(cmdEl, cmdQueue, succeeded, response)
	} else {
		cmd.resultReceived = true
	}
}

func currentCommandDoneCh(cmdQueue *list.List) <-chan struct{} {
	cmdEl := cmdQueue.Front()
	if cmdEl == nil {
		return nil
	}
	cmd := cmdEl.Value.(*qmpCommand)
	return cmd.ctx.Done()
}

func (q *QMP) writeNextQMPCommand(cmdQueue *list.List) {
	cmdEl := cmdQueue.Front()
	cmd := cmdEl.Value.(*qmpCommand)
	cmdData := make(map[string]interface{})
	cmdData["execute"] = cmd.name
	if cmd.args != nil {
		cmdData["arguments"] = cmd.args
	}
	encodedCmd, err := json.Marshal(&cmdData)
	if err != nil {
		cmd.res <- qmpResult{
			err: fmt.Errorf("unable to marhsall command %s: %v",
				cmd.name, err),
		}
		cmdQueue.Remove(cmdEl)
	}
	encodedCmd = append(encodedCmd, '\n')
	if unixConn, ok := q.conn.(*net.UnixConn); ok && len(cmd.oob) > 0 {
		_, _, err = unixConn.WriteMsgUnix(encodedCmd, cmd.oob, nil)
	} else {
		_, err = q.conn.Write(encodedCmd)
	}

	if err != nil {
		cmd.res <- qmpResult{
			err: fmt.Errorf("unable to write command to qmp socket %v", err),
		}
		cmdQueue.Remove(cmdEl)
	}
}

func failOutstandingCommands(cmdQueue *list.List) {
	for e := cmdQueue.Front(); e != nil; e = e.Next() {
		cmd := e.Value.(*qmpCommand)
		select {
		case cmd.res <- qmpResult{
			err: errors.New("exitting QMP loop, command cancelled"),
		}:
		case <-cmd.ctx.Done():
		}
	}
}

func (q *QMP) cancelCurrentCommand(cmdQueue *list.List) {
	cmdEl := cmdQueue.Front()
	cmd := cmdEl.Value.(*qmpCommand)
	if cmd.resultReceived {
		q.finaliseCommand(cmdEl, cmdQueue, false)
	} else {
		cmd.filter = nil
	}
}

func (q *QMP) parseVersion(version []byte) *QMPVersion {
	var qmp map[string]interface{}
	err := json.Unmarshal(version, &qmp)
	if err != nil {
		q.cfg.Logger.Errorf("Invalid QMP greeting: %s", string(version))
		return nil
	}

	versionMap := qmp
	for _, k := range []string{"QMP", "version", "qemu"} {
		versionMap, _ = versionMap[k].(map[string]interface{})
		if versionMap == nil {
			return nil
		}
	}

	micro, _ := versionMap["micro"].(float64)
	minor, _ := versionMap["minor"].(float64)
	major, _ := versionMap["major"].(float64)
	capabilities, _ := qmp["QMP"].(map[string]interface{})["capabilities"].([]interface{})
	stringcaps := make([]string, 0, len(capabilities))
	for _, c := range capabilities {
		if cap, ok := c.(string); ok {
			stringcaps = append(stringcaps, cap)
		}
	}
	return &QMPVersion{Major: int(major),
		Minor:        int(minor),
		Micro:        int(micro),
		Capabilities: stringcaps,
	}
}

// The qemu package allows multiple QMP commands to be submitted concurrently
// from different Go routines.  Unfortunately, QMP doesn't really support parallel
// commands as there is no way reliable way to associate a command response
// with a request.  For this reason we need to submit our commands to
// QMP serially.  The qemu package performs this serialisation using a
// queue (cmdQueue owned by mainLoop).  We use a queue rather than a simple
// mutex so we can support cancelling of commands (see below) and ordered
// execution of commands, i.e., if command B is issued before command C,
// it should be executed before command C even if both commands are initially
// blocked waiting for command A to finish.  This would be hard to achieve with
// a simple mutex.
//
// Cancelling is a little tricky.  Commands such as ExecuteQMPCapabilities
// can be cancelled by cancelling or timing out their contexts.  When a
// command is cancelled the calling function, e.g., ExecuteQMPCapabilities,
// will return but we may not be able to remove the command's entry from
// the command queue or issue the next command.  There are two scenarios
// here.
//
// 1. The command has been processed by QMP, i.e., we have received a
// return or an error, but is still blocking as it is waiting for
// an event.  For example, the ExecuteDeviceDel blocks until a DEVICE_DELETED
// event is received.  When such a command is cancelled we can remove it
// from the queue and start issuing the next command.  When the DEVICE_DELETED
// event eventually arrives it will just be ignored.
//
// 2. The command has not been processed by QMP.  In this case the command
// needs to remain on the cmdQueue until the response to this command is
// received from QMP.  During this time no new commands can be issued.  When the
// response is received, it is discarded (as no one is interested in the result
// any more), the entry is removed from the cmdQueue and we can proceed to
// execute the next command.

func (q *QMP) mainLoop() {
	cmdQueue := list.New().Init()
	fromVMCh := make(chan []byte)
	go q.readLoop(fromVMCh)

	defer func() {
		if q.cfg.EventCh != nil {
			close(q.cfg.EventCh)
		}
		/* #nosec */
		_ = q.conn.Close()
		<-fromVMCh
		failOutstandingCommands(cmdQueue)
		close(q.disconnectedCh)
	}()

	var cmdDoneCh <-chan struct{}
	var version *QMPVersion
	ready := false

	for {
		select {
		case cmd, ok := <-q.cmdCh:
			if !ok {
				return
			}
			_ = cmdQueue.PushBack(&cmd)

			// We only want to execute the new cmd if QMP is
			// ready and there are no other commands pending.
			// If there are commands pending our new command
			// will get run when the pending commands complete.
			if ready && cmdQueue.Len() == 1 {
				q.writeNextQMPCommand(cmdQueue)
				cmdDoneCh = currentCommandDoneCh(cmdQueue)
			}

		case line, ok := <-fromVMCh:
			if !ok {
				return
			}

			if !ready {
				// Not ready yet. Check if line is the QMP version.
				// Sometimes QMP events are thrown before the QMP version,
				// hence it's not a guarantee that the first data read from
				// the channel is the QMP version.
				version = q.parseVersion(line)
				if version != nil {
					q.connectedCh <- version
					ready = true
				}
				// Do not process QMP input to avoid deadlocks.
				break
			}

			q.processQMPInput(line, cmdQueue)
			cmdDoneCh = currentCommandDoneCh(cmdQueue)

		case <-cmdDoneCh:
			q.cancelCurrentCommand(cmdQueue)
			cmdDoneCh = currentCommandDoneCh(cmdQueue)
		}
	}
}

func startQMPLoop(conn io.ReadWriteCloser, cfg QMPConfig,
	connectedCh chan<- *QMPVersion, disconnectedCh chan struct{}) *QMP {
	q := &QMP{
		cmdCh:          make(chan qmpCommand),
		conn:           conn,
		cfg:            cfg,
		connectedCh:    connectedCh,
		disconnectedCh: disconnectedCh,
	}
	go q.mainLoop()
	return q
}

func (q *QMP) executeCommandWithResponse(ctx context.Context, name string, args map[string]interface{},
	oob []byte, filter *qmpEventFilter) (interface{}, error) {
	var err error
	var response interface{}
	resCh := make(chan qmpResult)
	select {
	case <-q.disconnectedCh:
		err = errors.New("exitting QMP loop, command cancelled")
	case q.cmdCh <- qmpCommand{
		ctx:    ctx,
		res:    resCh,
		name:   name,
		args:   args,
		filter: filter,
		oob:    oob,
	}:
	}

	if err != nil {
		return response, err
	}

	select {
	case res := <-resCh:
		err = res.err
		response = res.response
	case <-ctx.Done():
		err = ctx.Err()
	}

	return response, err
}

func (q *QMP) executeCommand(ctx context.Context, name string, args map[string]interface{},
	filter *qmpEventFilter) error {

	_, err := q.executeCommandWithResponse(ctx, name, args, nil, filter)
	return err
}

// QMPStart connects to a unix domain socket maintained by a QMP instance.  It
// waits to receive the QMP welcome message via the socket and spawns some go
// routines to manage the socket.  The function returns a *QMP which can be
// used by callers to send commands to the QEMU instance or to close the
// socket and all the go routines that have been spawned to monitor it.  A
// *QMPVersion is also returned.  This structure contains the version and
// capabilities information returned by the QEMU instance in its welcome
// message.
//
// socket contains the path to the domain socket. cfg contains some options
// that can be specified by the caller, namely where the qemu package should
// send logs and QMP events.  disconnectedCh is a channel that must be supplied
// by the caller.  It is closed when an error occurs openning or writing to
// or reading from the unix domain socket.  This implies that the QEMU instance
// that opened the socket has closed.
//
// If this function returns without error, callers should call QMP.Shutdown
// when they wish to stop monitoring the QMP instance.  This is not strictly
// necessary if the QEMU instance exits and the disconnectedCh is closed, but
// doing so will not cause any problems.
//
// Commands can be sent to the QEMU instance via the QMP.Execute methods.
// These commands are executed serially, even if the QMP.Execute methods
// are called from different go routines.  The QMP.Execute methods will
// block until they have received a success or failure message from QMP,
// i.e., {"return": {}} or {"error":{}}, and in some cases certain events
// are received.
//
// QEMU currently requires that the "qmp_capabilties" command is sent before any
// other command. Therefore you must call qmp.ExecuteQMPCapabilities() before
// you execute any other command.
func QMPStart(ctx context.Context, socket string, cfg QMPConfig, disconnectedCh chan struct{}) (*QMP, *QMPVersion, error) {
	if cfg.Logger == nil {
		cfg.Logger = qmpNullLogger{}
	}
	dialer := net.Dialer{Cancel: ctx.Done()}
	conn, err := dialer.Dial("unix", socket)
	if err != nil {
		cfg.Logger.Warningf("Unable to connect to unix socket (%s): %v", socket, err)
		close(disconnectedCh)
		return nil, nil, err
	}

	connectedCh := make(chan *QMPVersion)

	q := startQMPLoop(conn, cfg, connectedCh, disconnectedCh)
	select {
	case <-ctx.Done():
		q.Shutdown()
		<-disconnectedCh
		return nil, nil, fmt.Errorf("canceled by caller")
	case <-disconnectedCh:
		return nil, nil, fmt.Errorf("lost connection to VM")
	case q.version = <-connectedCh:
		if q.version == nil {
			return nil, nil, fmt.Errorf("failed to find QMP version information")
		}
	}

	return q, q.version, nil
}

// Shutdown closes the domain socket used to monitor a QEMU instance and
// terminates all the go routines spawned by QMPStart to manage that instance.
// QMP.Shutdown does not shut down the running instance.  Calling QMP.Shutdown
// will result in the disconnectedCh channel being closed, indicating that we
// have lost connection to the QMP instance.  In this case it does not indicate
// that the instance has quit.
//
// QMP.Shutdown should not be called concurrently with other QMP methods.  It
// should not be called twice on the same QMP instance.
//
// Calling QMP.Shutdown after the disconnectedCh channel is closed is permitted but
// will not have any effect.
func (q *QMP) Shutdown() {
	close(q.cmdCh)
}

// ExecuteQMPCapabilities executes the qmp_capabilities command on the instance.
func (q *QMP) ExecuteQMPCapabilities(ctx context.Context) error {
	return q.executeCommand(ctx, "qmp_capabilities", nil, nil)
}

// ExecuteStop sends the stop command to the instance.
func (q *QMP) ExecuteStop(ctx context.Context) error {
	return q.executeCommand(ctx, "stop", nil, nil)
}

// ExecuteCont sends the cont command to the instance.
func (q *QMP) ExecuteCont(ctx context.Context) error {
	return q.executeCommand(ctx, "cont", nil, nil)
}

// ExecuteSystemPowerdown sends the system_powerdown command to the instance.
// This function will block until the SHUTDOWN event is received.
func (q *QMP) ExecuteSystemPowerdown(ctx context.Context) error {
	filter := &qmpEventFilter{
		eventName: "SHUTDOWN",
	}
	return q.executeCommand(ctx, "system_powerdown", nil, filter)
}

// ExecuteQuit sends the quit command to the instance, terminating
// the QMP instance immediately.
func (q *QMP) ExecuteQuit(ctx context.Context) error {
	return q.executeCommand(ctx, "quit", nil, nil)
}

func (q *QMP) blockdevAddBaseArgs(device, blockdevID string, ro bool) (map[string]interface{}, map[string]interface{}) {
	var args map[string]interface{}

	blockdevArgs := map[string]interface{}{
		"driver":    "raw",
		"read-only": ro,
		"file": map[string]interface{}{
			"driver":   "file",
			"filename": device,
		},
	}

	if q.version.Major > 2 || (q.version.Major == 2 && q.version.Minor >= 8) {
		blockdevArgs["node-name"] = blockdevID
		args = blockdevArgs
	} else {
		blockdevArgs["id"] = blockdevID
		args = map[string]interface{}{
			"options": blockdevArgs,
		}
	}

	return args, blockdevArgs
}

// ExecuteBlockdevAdd sends a blockdev-add to the QEMU instance.  device is the
// path of the device to add, e.g., /dev/rdb0, and blockdevID is an identifier
// used to name the device.  As this identifier will be passed directly to QMP,
// it must obey QMP's naming rules, e,g., it must start with a letter.
func (q *QMP) ExecuteBlockdevAdd(ctx context.Context, device, blockdevID string, ro bool) error {
	args, _ := q.blockdevAddBaseArgs(device, blockdevID, ro)

	return q.executeCommand(ctx, "blockdev-add", args, nil)
}

// ExecuteBlockdevAddWithCache has two more parameters direct and noFlush
// than ExecuteBlockdevAdd.
// They are cache-related options for block devices that are described in
// https://github.com/qemu/qemu/blob/master/qapi/block-core.json.
// direct denotes whether use of O_DIRECT (bypass the host page cache)
// is enabled.  noFlush denotes whether flush requests for the device are
// ignored.
func (q *QMP) ExecuteBlockdevAddWithCache(ctx context.Context, device, blockdevID string, direct, noFlush, ro bool) error {
	args, blockdevArgs := q.blockdevAddBaseArgs(device, blockdevID, ro)

	if q.version.Major < 2 || (q.version.Major == 2 && q.version.Minor < 9) {
		return fmt.Errorf("versions of qemu (%d.%d) older than 2.9 do not support set cache-related options for block devices",
			q.version.Major, q.version.Minor)
	}

	blockdevArgs["cache"] = map[string]interface{}{
		"direct":   direct,
		"no-flush": noFlush,
	}

	return q.executeCommand(ctx, "blockdev-add", args, nil)
}

// ExecuteDeviceAdd adds the guest portion of a device to a QEMU instance
// using the device_add command.  blockdevID should match the blockdevID passed
// to a previous call to ExecuteBlockdevAdd.  devID is the id of the device to
// add.  Both strings must be valid QMP identifiers.  driver is the name of the
// driver,e.g., virtio-blk-pci, and bus is the name of the bus.  bus is optional.
// shared denotes if the drive can be shared allowing it to be passed more than once.
// disableModern indicates if virtio version 1.0 should be replaced by the
// former version 0.9, as there is a KVM bug that occurs when using virtio
// 1.0 in nested environments.
func (q *QMP) ExecuteDeviceAdd(ctx context.Context, blockdevID, devID, driver, bus, romfile string, shared, disableModern bool) error {
	args := map[string]interface{}{
		"id":     devID,
		"driver": driver,
		"drive":  blockdevID,
	}

	var transport VirtioTransport

	if transport.isVirtioCCW(nil) {
		args["devno"] = bus
	} else if bus != "" {
		args["bus"] = bus
	}

	if shared && (q.version.Major > 2 || (q.version.Major == 2 && q.version.Minor >= 10)) {
		args["share-rw"] = "on"
	}
	if transport.isVirtioPCI(nil) {
		args["romfile"] = romfile

		if disableModern {
			args["disable-modern"] = disableModern
		}
	}

	return q.executeCommand(ctx, "device_add", args, nil)
}

// ExecuteSCSIDeviceAdd adds the guest portion of a block device to a QEMU instance
// using a SCSI driver with the device_add command.  blockdevID should match the
// blockdevID passed to a previous call to ExecuteBlockdevAdd.  devID is the id of
// the device to add.  Both strings must be valid QMP identifiers.  driver is the name of the
// scsi driver,e.g., scsi-hd, and bus is the name of a SCSI controller bus.
// scsiID is the SCSI id, lun is logical unit number. scsiID and lun are optional, a negative value
// for scsiID and lun is ignored. shared denotes if the drive can be shared allowing it
// to be passed more than once.
// disableModern indicates if virtio version 1.0 should be replaced by the
// former version 0.9, as there is a KVM bug that occurs when using virtio
// 1.0 in nested environments.
func (q *QMP) ExecuteSCSIDeviceAdd(ctx context.Context, blockdevID, devID, driver, bus, romfile string, scsiID, lun int, shared, disableModern bool) error {
	// TBD: Add drivers for scsi passthrough like scsi-generic and scsi-block
	drivers := []string{"scsi-hd", "scsi-cd", "scsi-disk"}

	isSCSIDriver := false
	for _, d := range drivers {
		if driver == d {
			isSCSIDriver = true
			break
		}
	}

	if !isSCSIDriver {
		return fmt.Errorf("invalid SCSI driver provided %s", driver)
	}

	args := map[string]interface{}{
		"id":     devID,
		"driver": driver,
		"drive":  blockdevID,
		"bus":    bus,
	}

	if scsiID >= 0 {
		args["scsi-id"] = scsiID
	}
	if lun >= 0 {
		args["lun"] = lun
	}
	if shared && (q.version.Major > 2 || (q.version.Major == 2 && q.version.Minor >= 10)) {
		args["share-rw"] = "on"
	}

	return q.executeCommand(ctx, "device_add", args, nil)
}

// ExecuteBlockdevDel deletes a block device by sending a x-blockdev-del command
// for qemu versions < 2.9. It sends the updated blockdev-del command for qemu>=2.9.
// blockdevID is the id of the block device to be deleted.  Typically, this will
// match the id passed to ExecuteBlockdevAdd.  It must be a valid QMP id.
func (q *QMP) ExecuteBlockdevDel(ctx context.Context, blockdevID string) error {
	args := map[string]interface{}{}

	if q.version.Major > 2 || (q.version.Major == 2 && q.version.Minor >= 9) {
		args["node-name"] = blockdevID
		return q.executeCommand(ctx, "blockdev-del", args, nil)
	}

	if q.version.Major == 2 && q.version.Minor == 8 {
		args["node-name"] = blockdevID
	} else {
		args["id"] = blockdevID
	}

	return q.executeCommand(ctx, "x-blockdev-del", args, nil)
}

// ExecuteChardevDel deletes a char device by sending a chardev-remove command.
// chardevID is the id of the char device to be deleted. Typically, this will
// match the id passed to ExecuteCharDevUnixSocketAdd. It must be a valid QMP id.
func (q *QMP) ExecuteChardevDel(ctx context.Context, chardevID string) error {
	args := map[string]interface{}{
		"id": chardevID,
	}

	return q.executeCommand(ctx, "chardev-remove", args, nil)
}

// ExecuteNetdevAdd adds a Net device to a QEMU instance
// using the netdev_add command. netdevID is the id of the device to add.
// Must be valid QMP identifier.
func (q *QMP) ExecuteNetdevAdd(ctx context.Context, netdevType, netdevID, ifname, downscript, script string, queues int) error {
	args := map[string]interface{}{
		"type":       netdevType,
		"id":         netdevID,
		"ifname":     ifname,
		"downscript": downscript,
		"script":     script,
	}
	if queues > 1 {
		args["queues"] = queues
	}

	return q.executeCommand(ctx, "netdev_add", args, nil)
}

// ExecuteNetdevChardevAdd adds a Net device to a QEMU instance
// using the netdev_add command. netdevID is the id of the device to add.
// Must be valid QMP identifier.
func (q *QMP) ExecuteNetdevChardevAdd(ctx context.Context, netdevType, netdevID, chardev string, queues int) error {
	args := map[string]interface{}{
		"type":    netdevType,
		"id":      netdevID,
		"chardev": chardev,
	}
	if queues > 1 {
		args["queues"] = queues
	}

	return q.executeCommand(ctx, "netdev_add", args, nil)
}

// ExecuteNetdevAddByFds adds a Net device to a QEMU instance
// using the netdev_add command by fds and vhostfds. netdevID is the id of the device to add.
// Must be valid QMP identifier.
func (q *QMP) ExecuteNetdevAddByFds(ctx context.Context, netdevType, netdevID string, fdNames, vhostFdNames []string) error {
	fdNameStr := strings.Join(fdNames, ":")
	args := map[string]interface{}{
		"type": netdevType,
		"id":   netdevID,
		"fds":  fdNameStr,
	}
	if len(vhostFdNames) > 0 {
		vhostFdNameStr := strings.Join(vhostFdNames, ":")
		args["vhost"] = true
		args["vhostfds"] = vhostFdNameStr
	}

	return q.executeCommand(ctx, "netdev_add", args, nil)
}

// ExecuteNetdevDel deletes a Net device from a QEMU instance
// using the netdev_del command. netdevID is the id of the device to delete.
func (q *QMP) ExecuteNetdevDel(ctx context.Context, netdevID string) error {
	args := map[string]interface{}{
		"id": netdevID,
	}
	return q.executeCommand(ctx, "netdev_del", args, nil)
}

// ExecuteNetPCIDeviceAdd adds a Net PCI device to a QEMU instance
// using the device_add command. devID is the id of the device to add.
// Must be valid QMP identifier. netdevID is the id of nic added by previous netdev_add.
// queues is the number of queues of a nic.
// disableModern indicates if virtio version 1.0 should be replaced by the
// former version 0.9, as there is a KVM bug that occurs when using virtio
// 1.0 in nested environments.
func (q *QMP) ExecuteNetPCIDeviceAdd(ctx context.Context, netdevID, devID, macAddr, addr, bus, romfile string, queues int, disableModern bool) error {
	args := map[string]interface{}{
		"id":      devID,
		"driver":  VirtioNetPCI,
		"romfile": romfile,
	}

	if bus != "" {
		args["bus"] = bus
	}
	if addr != "" {
		args["addr"] = addr
	}
	if macAddr != "" {
		args["mac"] = macAddr
	}
	if netdevID != "" {
		args["netdev"] = netdevID
	}
	if disableModern {
		args["disable-modern"] = disableModern
	}

	if queues > 0 {
		// (2N+2 vectors, N for tx queues, N for rx queues, 1 for config, and one for possible control vq)
		// -device virtio-net-pci,mq=on,vectors=2N+2...
		// enable mq in guest by 'ethtool -L eth0 combined $queue_num'
		// Clearlinux automatically sets up the queues properly
		// The agent implementation should do this to ensure that it is
		// always set
		args["mq"] = "on"
		args["vectors"] = 2*queues + 2
	}

	return q.executeCommand(ctx, "device_add", args, nil)
}

// ExecuteNetCCWDeviceAdd adds a Net CCW device to a QEMU instance
// using the device_add command. devID is the id of the device to add.
// Must be valid QMP identifier. netdevID is the id of nic added by previous netdev_add.
// queues is the number of queues of a nic.
func (q *QMP) ExecuteNetCCWDeviceAdd(ctx context.Context, netdevID, devID, macAddr, bus string, queues int) error {
	args := map[string]interface{}{
		"id":     devID,
		"driver": VirtioNetCCW,
		"netdev": netdevID,
		"mac":    macAddr,
		"devno":  bus,
	}

	if queues > 0 {
		args["mq"] = "on"
	}

	return q.executeCommand(ctx, "device_add", args, nil)
}

// ExecuteDeviceDel deletes guest portion of a QEMU device by sending a
// device_del command.   devId is the identifier of the device to delete.
// Typically it would match the devID parameter passed to an earlier call
// to ExecuteDeviceAdd.  It must be a valid QMP identidier.
//
// This method blocks until a DEVICE_DELETED event is received for devID.
func (q *QMP) ExecuteDeviceDel(ctx context.Context, devID string) error {
	args := map[string]interface{}{
		"id": devID,
	}
	filter := &qmpEventFilter{
		eventName: "DEVICE_DELETED",
		dataKey:   "device",
		dataValue: devID,
	}
	return q.executeCommand(ctx, "device_del", args, filter)
}

// ExecutePCIDeviceAdd is the PCI version of ExecuteDeviceAdd. This function can be used
// to hot plug PCI devices on PCI(E) bridges, unlike ExecuteDeviceAdd this function receive the
// device address on its parent bus. bus is optional. queues specifies the number of queues of
// a block device. shared denotes if the drive can be shared allowing it to be passed more than once.
// disableModern indicates if virtio version 1.0 should be replaced by the
// former version 0.9, as there is a KVM bug that occurs when using virtio
// 1.0 in nested environments.
func (q *QMP) ExecutePCIDeviceAdd(ctx context.Context, blockdevID, devID, driver, addr, bus, romfile string, queues int, shared, disableModern bool) error {
	args := map[string]interface{}{
		"id":     devID,
		"driver": driver,
		"drive":  blockdevID,
		"addr":   addr,
	}
	if bus != "" {
		args["bus"] = bus
	}
	if shared && (q.version.Major > 2 || (q.version.Major == 2 && q.version.Minor >= 10)) {
		args["share-rw"] = "on"
	}
	if queues > 0 {
		args["num-queues"] = strconv.Itoa(queues)
	}

	var transport VirtioTransport

	if transport.isVirtioPCI(nil) {
		args["romfile"] = romfile

		if disableModern {
			args["disable-modern"] = disableModern
		}
	}

	return q.executeCommand(ctx, "device_add", args, nil)
}

// ExecutePCIVhostUserDevAdd adds a vhost-user device to a QEMU instance using the device_add command.
// This function can be used to hot plug vhost-user devices on PCI(E) bridges.
// It receives the bus and the device address on its parent bus. bus is optional.
// devID is the id of the device to add.Must be valid QMP identifier. chardevID
// is the QMP identifier of character device using a unix socket as backend.
// driver is the name of vhost-user driver, like vhost-user-blk-pci.
func (q *QMP) ExecutePCIVhostUserDevAdd(ctx context.Context, driver, devID, chardevID, addr, bus string) error {
	args := map[string]interface{}{
		"driver":  driver,
		"id":      devID,
		"chardev": chardevID,
		"addr":    addr,
	}

	if bus != "" {
		args["bus"] = bus
	}

	return q.executeCommand(ctx, "device_add", args, nil)
}

// ExecuteVFIODeviceAdd adds a VFIO device to a QEMU instance using the device_add command.
// devID is the id of the device to add. Must be valid QMP identifier.
// bdf is the PCI bus-device-function of the pci device.
// bus is optional. When hot plugging a PCIe device, the bus can be the ID of the pcie-root-port.
func (q *QMP) ExecuteVFIODeviceAdd(ctx context.Context, devID, bdf, bus, romfile string) error {
	var driver string
	var transport VirtioTransport

	if transport.isVirtioCCW(nil) {
		driver = string(VfioCCW)
	} else {
		driver = string(VfioPCI)
	}

	args := map[string]interface{}{
		"id":      devID,
		"driver":  driver,
		"host":    bdf,
		"romfile": romfile,
	}
	if bus != "" {
		args["bus"] = bus
	}
	return q.executeCommand(ctx, "device_add", args, nil)
}

// ExecutePCIVFIODeviceAdd adds a VFIO device to a QEMU instance using the device_add command.
// This function can be used to hot plug VFIO devices on PCI(E) bridges, unlike
// ExecuteVFIODeviceAdd this function receives the bus and the device address on its parent bus.
// bus is optional. devID is the id of the device to add.Must be valid QMP identifier. bdf is the
// PCI bus-device-function of the pci device.
func (q *QMP) ExecutePCIVFIODeviceAdd(ctx context.Context, devID, bdf, addr, bus, romfile string) error {
	args := map[string]interface{}{
		"id":      devID,
		"driver":  VfioPCI,
		"host":    bdf,
		"addr":    addr,
		"romfile": romfile,
	}

	if bus != "" {
		args["bus"] = bus
	}
	return q.executeCommand(ctx, "device_add", args, nil)
}

// ExecutePCIVFIOMediatedDeviceAdd adds a VFIO mediated device to a QEMU instance using the device_add command.
// This function can be used to hot plug VFIO mediated devices on PCI(E) bridges or root bus, unlike
// ExecuteVFIODeviceAdd this function receives the bus and the device address on its parent bus.
// devID is the id of the device to add. Must be valid QMP identifier. sysfsdev is the VFIO mediated device.
// Both bus and addr are optional. If they are both set to be empty, the system will pick up an empty slot on root bus.
func (q *QMP) ExecutePCIVFIOMediatedDeviceAdd(ctx context.Context, devID, sysfsdev, addr, bus, romfile string) error {
	args := map[string]interface{}{
		"id":       devID,
		"driver":   VfioPCI,
		"sysfsdev": sysfsdev,
		"romfile":  romfile,
	}

	if bus != "" {
		args["bus"] = bus
	}
	if addr != "" {
		args["addr"] = addr
	}
	return q.executeCommand(ctx, "device_add", args, nil)
}

// ExecuteAPVFIOMediatedDeviceAdd adds a VFIO mediated AP device to a QEMU instance using the device_add command.
func (q *QMP) ExecuteAPVFIOMediatedDeviceAdd(ctx context.Context, sysfsdev string) error {
	args := map[string]interface{}{
		"driver":   VfioAP,
		"sysfsdev": sysfsdev,
	}
	return q.executeCommand(ctx, "device_add", args, nil)
}

// isSocketIDSupported returns if the cpu driver supports the socket id option
func isSocketIDSupported(driver string) bool {
	if driver == "host-s390x-cpu" || driver == "host-powerpc64-cpu" {
		return false
	}
	return true
}

// isThreadIDSupported returns if the cpu driver supports the thread id option
func isThreadIDSupported(driver string) bool {
	if driver == "host-s390x-cpu" || driver == "host-powerpc64-cpu" {
		return false
	}
	return true
}

// isDieIDSupported returns if the cpu driver and the qemu version support the die id option
func (q *QMP) isDieIDSupported(driver string) bool {
	if (q.version.Major > 4 || (q.version.Major == 4 && q.version.Minor >= 1)) && driver == "host-x86_64-cpu" {
		return true
	}
	return false
}

// ExecuteCPUDeviceAdd adds a CPU to a QEMU instance using the device_add command.
// driver is the CPU model, cpuID must be a unique ID to identify the CPU, socketID is the socket number within
// node/board the CPU belongs to, coreID is the core number within socket the CPU belongs to, threadID is the
// thread number within core the CPU belongs to. Note that socketID and threadID are not a requirement for
// architecures like ppc64le.
func (q *QMP) ExecuteCPUDeviceAdd(ctx context.Context, driver, cpuID, socketID, dieID, coreID, threadID, romfile string) error {
	args := map[string]interface{}{
		"driver":  driver,
		"id":      cpuID,
		"core-id": coreID,
	}

	if socketID != "" && isSocketIDSupported(driver) {
		args["socket-id"] = socketID
	}

	if threadID != "" && isThreadIDSupported(driver) {
		args["thread-id"] = threadID
	}

	if q.isDieIDSupported(driver) {
		if dieID != "" {
			args["die-id"] = dieID
		}
	}

	return q.executeCommand(ctx, "device_add", args, nil)
}

// ExecuteQueryHotpluggableCPUs returns a slice with the list of hotpluggable CPUs
func (q *QMP) ExecuteQueryHotpluggableCPUs(ctx context.Context) ([]HotpluggableCPU, error) {
	response, err := q.executeCommandWithResponse(ctx, "query-hotpluggable-cpus", nil, nil, nil)
	if err != nil {
		return nil, err
	}

	// convert response to json
	data, err := json.Marshal(response)
	if err != nil {
		return nil, fmt.Errorf("unable to extract CPU information: %v", err)
	}

	var cpus []HotpluggableCPU
	// convert json to []HotpluggableCPU
	if err = json.Unmarshal(data, &cpus); err != nil {
		return nil, fmt.Errorf("unable to convert json to hotpluggable CPU: %v", err)
	}

	return cpus, nil
}

// ExecSetMigrationCaps sets migration capabilities
func (q *QMP) ExecSetMigrationCaps(ctx context.Context, caps []map[string]interface{}) error {
	args := map[string]interface{}{
		"capabilities": caps,
	}

	return q.executeCommand(ctx, "migrate-set-capabilities", args, nil)
}

// ExecSetMigrateArguments sets the command line used for migration
func (q *QMP) ExecSetMigrateArguments(ctx context.Context, url string) error {
	args := map[string]interface{}{
		"uri": url,
	}

	return q.executeCommand(ctx, "migrate", args, nil)
}

// ExecQueryMemoryDevices returns a slice with the list of memory devices
func (q *QMP) ExecQueryMemoryDevices(ctx context.Context) ([]MemoryDevices, error) {
	response, err := q.executeCommandWithResponse(ctx, "query-memory-devices", nil, nil, nil)
	if err != nil {
		return nil, err
	}

	// convert response to json
	data, err := json.Marshal(response)
	if err != nil {
		return nil, fmt.Errorf("unable to extract memory devices information: %v", err)
	}

	var memoryDevices []MemoryDevices
	// convert json to []MemoryDevices
	if err = json.Unmarshal(data, &memoryDevices); err != nil {
		return nil, fmt.Errorf("unable to convert json to memory devices: %v", err)
	}

	return memoryDevices, nil
}

// ExecQueryCpus returns a slice with the list of `CpuInfo`
// Since qemu 2.12, we have `query-cpus-fast` as a better choice in production
// we can still choose `ExecQueryCpus` for compatibility though not recommended.
func (q *QMP) ExecQueryCpus(ctx context.Context) ([]CPUInfo, error) {
	response, err := q.executeCommandWithResponse(ctx, "query-cpus", nil, nil, nil)
	if err != nil {
		return nil, err
	}

	// convert response to json
	data, err := json.Marshal(response)
	if err != nil {
		return nil, fmt.Errorf("unable to extract memory devices information: %v", err)
	}

	var cpuInfo []CPUInfo
	// convert json to []CPUInfo
	if err = json.Unmarshal(data, &cpuInfo); err != nil {
		return nil, fmt.Errorf("unable to convert json to CPUInfo: %v", err)
	}

	return cpuInfo, nil
}

// ExecQueryCpusFast returns a slice with the list of `CpuInfoFast`
// This is introduced since 2.12, it does not incur a performance penalty and
// should be used in production instead of query-cpus.
func (q *QMP) ExecQueryCpusFast(ctx context.Context) ([]CPUInfoFast, error) {
	response, err := q.executeCommandWithResponse(ctx, "query-cpus-fast", nil, nil, nil)
	if err != nil {
		return nil, err
	}

	// convert response to json
	data, err := json.Marshal(response)
	if err != nil {
		return nil, fmt.Errorf("unable to extract memory devices information: %v", err)
	}

	var cpuInfoFast []CPUInfoFast
	// convert json to []CPUInfoFast
	if err = json.Unmarshal(data, &cpuInfoFast); err != nil {
		return nil, fmt.Errorf("unable to convert json to CPUInfoFast: %v", err)
	}

	return cpuInfoFast, nil
}

// ExecMemdevAdd adds size of MiB memory device to the guest
func (q *QMP) ExecMemdevAdd(ctx context.Context, qomtype, id, mempath string, size int, share bool, driver, driverID string) error {
	props := map[string]interface{}{"size": uint64(size) << 20}
	args := map[string]interface{}{
		"qom-type": qomtype,
		"id":       id,
		"props":    props,
	}
	if mempath != "" {
		props["mem-path"] = mempath
	}
	if share {
		props["share"] = true
	}
	err := q.executeCommand(ctx, "object-add", args, nil)
	if err != nil {
		return err
	}

	defer func() {
		if err != nil {
			q.cfg.Logger.Errorf("Unable to add memory device %s: %v", id, err)
			err = q.executeCommand(ctx, "object-del", map[string]interface{}{"id": id}, nil)
			if err != nil {
				q.cfg.Logger.Warningf("Unable to clean up memory object %s: %v", id, err)
			}
		}
	}()

	args = map[string]interface{}{
		"driver": driver,
		"id":     driverID,
		"memdev": id,
	}
	err = q.executeCommand(ctx, "device_add", args, nil)

	return err
}

// ExecHotplugMemory adds size of MiB memory to the guest
func (q *QMP) ExecHotplugMemory(ctx context.Context, qomtype, id, mempath string, size int, share bool) error {
	return q.ExecMemdevAdd(ctx, qomtype, id, mempath, size, share, "pc-dimm", "dimm"+id)
}

// ExecuteNVDIMMDeviceAdd adds a block device to a QEMU instance using
// a NVDIMM driver with the device_add command.
// id is the id of the device to add.  It must be a valid QMP identifier.
// mempath is the path of the device to add, e.g., /dev/rdb0.  size is
// the data size of the device. pmem is to guarantee the persistence of QEMU writes
// to the vNVDIMM backend.
func (q *QMP) ExecuteNVDIMMDeviceAdd(ctx context.Context, id, mempath string, size int64, pmem *bool) error {
	args := map[string]interface{}{
		"qom-type": "memory-backend-file",
		"id":       "nvdimmbackmem" + id,
		"props": map[string]interface{}{
			"mem-path": mempath,
			"size":     size,
			"share":    true,
		},
	}

	if q.version.Major > 4 || (q.version.Major == 4 && q.version.Minor >= 1) {
		if pmem != nil {
			props := args["props"].(map[string]interface{})
			props["pmem"] = *pmem
		}
	}

	err := q.executeCommand(ctx, "object-add", args, nil)
	if err != nil {
		return err
	}

	args = map[string]interface{}{
		"driver": "nvdimm",
		"id":     "nvdimm" + id,
		"memdev": "nvdimmbackmem" + id,
	}
	if err = q.executeCommand(ctx, "device_add", args, nil); err != nil {
		q.cfg.Logger.Errorf("Unable to hotplug NVDIMM device: %v", err)
		err2 := q.executeCommand(ctx, "object-del", map[string]interface{}{"id": "nvdimmbackmem" + id}, nil)
		if err2 != nil {
			q.cfg.Logger.Warningf("Unable to clean up memory object: %v", err2)
		}
	}

	return err
}

// ExecuteBalloon sets the size of the balloon, hence updates the memory
// allocated for the VM.
func (q *QMP) ExecuteBalloon(ctx context.Context, bytes uint64) error {
	args := map[string]interface{}{
		"value": bytes,
	}
	return q.executeCommand(ctx, "balloon", args, nil)
}

// ExecutePCIVSockAdd adds a vhost-vsock-pci bus
// disableModern indicates if virtio version 1.0 should be replaced by the
// former version 0.9, as there is a KVM bug that occurs when using virtio
// 1.0 in nested environments.
func (q *QMP) ExecutePCIVSockAdd(ctx context.Context, id, guestCID, vhostfd, addr, bus, romfile string, disableModern bool) error {
	args := map[string]interface{}{
		"driver":    VHostVSockPCI,
		"id":        id,
		"guest-cid": guestCID,
		"vhostfd":   vhostfd,
		"addr":      addr,
		"romfile":   romfile,
	}

	if bus != "" {
		args["bus"] = bus
	}

	if disableModern {
		args["disable-modern"] = disableModern
	}

	return q.executeCommand(ctx, "device_add", args, nil)
}

// ExecuteGetFD sends a file descriptor via SCM rights and assigns it a name
func (q *QMP) ExecuteGetFD(ctx context.Context, fdname string, fd *os.File) error {
	oob := syscall.UnixRights(int(fd.Fd()))
	args := map[string]interface{}{
		"fdname": fdname,
	}

	_, err := q.executeCommandWithResponse(ctx, "getfd", args, oob, nil)
	return err
}

// ExecuteCharDevUnixSocketAdd adds a character device using as backend a unix socket,
// id is an identifier for the device, path specifies the local path of the unix socket,
// wait is to block waiting for a client to connect, server specifies that the socket is a listening socket.
func (q *QMP) ExecuteCharDevUnixSocketAdd(ctx context.Context, id, path string, wait, server bool) error {
	args := map[string]interface{}{
		"id": id,
		"backend": map[string]interface{}{
			"type": "socket",
			"data": map[string]interface{}{
				"wait":   wait,
				"server": server,
				"addr": map[string]interface{}{
					"type": "unix",
					"data": map[string]interface{}{
						"path": path,
					},
				},
			},
		},
	}
	return q.executeCommand(ctx, "chardev-add", args, nil)
}

// ExecuteVirtSerialPortAdd adds a virtserialport.
// id is an identifier for the virtserialport, name is a name for the virtserialport and
// it will be visible in the VM, chardev is the character device id previously added.
func (q *QMP) ExecuteVirtSerialPortAdd(ctx context.Context, id, name, chardev string) error {
	args := map[string]interface{}{
		"driver":  VirtioSerialPort,
		"id":      id,
		"name":    name,
		"chardev": chardev,
	}

	return q.executeCommand(ctx, "device_add", args, nil)
}

// ExecuteQueryMigration queries migration progress.
func (q *QMP) ExecuteQueryMigration(ctx context.Context) (MigrationStatus, error) {
	response, err := q.executeCommandWithResponse(ctx, "query-migrate", nil, nil, nil)
	if err != nil {
		return MigrationStatus{}, err
	}

	data, err := json.Marshal(response)
	if err != nil {
		return MigrationStatus{}, fmt.Errorf("unable to extract migrate status information: %v", err)
	}

	var status MigrationStatus
	if err = json.Unmarshal(data, &status); err != nil {
		return MigrationStatus{}, fmt.Errorf("unable to convert migrate status information: %v", err)
	}

	return status, nil
}

// ExecuteMigrationIncoming start migration from incoming uri.
func (q *QMP) ExecuteMigrationIncoming(ctx context.Context, uri string) error {
	args := map[string]interface{}{
		"uri": uri,
	}
	return q.executeCommand(ctx, "migrate-incoming", args, nil)
}

// ExecQueryQmpSchema query all QMP wire ABI and returns a slice
func (q *QMP) ExecQueryQmpSchema(ctx context.Context) ([]SchemaInfo, error) {
	response, err := q.executeCommandWithResponse(ctx, "query-qmp-schema", nil, nil, nil)
	if err != nil {
		return nil, err
	}

	// convert response to json
	data, err := json.Marshal(response)
	if err != nil {
		return nil, fmt.Errorf("unable to extract memory devices information: %v", err)
	}

	var schemaInfo []SchemaInfo
	if err = json.Unmarshal(data, &schemaInfo); err != nil {
		return nil, fmt.Errorf("unable to convert json to schemaInfo: %v", err)
	}

	return schemaInfo, nil
}

// ExecuteQueryStatus queries guest status
func (q *QMP) ExecuteQueryStatus(ctx context.Context) (StatusInfo, error) {
	response, err := q.executeCommandWithResponse(ctx, "query-status", nil, nil, nil)
	if err != nil {
		return StatusInfo{}, err
	}

	data, err := json.Marshal(response)
	if err != nil {
		return StatusInfo{}, fmt.Errorf("unable to extract migrate status information: %v", err)
	}

	var status StatusInfo
	if err = json.Unmarshal(data, &status); err != nil {
		return StatusInfo{}, fmt.Errorf("unable to convert migrate status information: %v", err)
	}

	return status, nil
}

// ExecQomSet qom-set path property value
func (q *QMP) ExecQomSet(ctx context.Context, path, property string, value uint64) error {
	args := map[string]interface{}{
		"path":     path,
		"property": property,
		"value":    value,
	}

	return q.executeCommand(ctx, "qom-set", args, nil)
}

// ExecQomGet qom-get path property
func (q *QMP) ExecQomGet(ctx context.Context, path, property string) (interface{}, error) {
	args := map[string]interface{}{
		"path":     path,
		"property": property,
	}

	response, err := q.executeCommandWithResponse(ctx, "qom-get", args, nil, nil)
	if err != nil {
		return "", err
	}

	return response, nil
}

// ExecuteDumpGuestMemory dump guest memory to host
func (q *QMP) ExecuteDumpGuestMemory(ctx context.Context, protocol string, paging bool, format string) error {
	args := map[string]interface{}{
		"protocol": protocol,
		"paging":   paging,
		"format":   format,
	}

	return q.executeCommand(ctx, "dump-guest-memory", args, nil)
}
