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
	"bufio"
	"container/list"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net"
	"time"

	"context"
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

// CPUProperties contains the properties to be used for hotplugging a CPU instance
type CPUProperties struct {
	Node   int `json:"node-id"`
	Socket int `json:"socket-id"`
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

func (q *QMP) readLoop(fromVMCh chan<- []byte) {
	scanner := bufio.NewScanner(q.conn)
	for scanner.Scan() {
		line := scanner.Bytes()
		if q.cfg.Logger.V(1) {
			q.cfg.Logger.Infof("%s", string(line))
		}
		fromVMCh <- line
	}
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
			cmd.res <- qmpResult{err: fmt.Errorf("QMP command failed")}
		}
	}
	if cmdQueue.Len() > 0 {
		q.writeNextQMPCommand(cmdQueue)
	}
}

func (q *QMP) finaliseCommand(cmdEl *list.Element, cmdQueue *list.List, succeeded bool) {
	q.finaliseCommandWithResponse(cmdEl, cmdQueue, succeeded, nil)
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
	_, failed := vmData["error"]

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
			err: fmt.Errorf("Unable to marhsall command %s: %v",
				cmd.name, err),
		}
		cmdQueue.Remove(cmdEl)
	}
	q.cfg.Logger.Infof("%s", string(encodedCmd))
	encodedCmd = append(encodedCmd, '\n')
	_, err = q.conn.Write(encodedCmd)
	if err != nil {
		cmd.res <- qmpResult{
			err: fmt.Errorf("Unable to write command to qmp socket %v", err),
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
			q.cfg.Logger.Errorf("Invalid QMP greeting: %s", string(version))
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
		_ = q.conn.Close()
		_ = <-fromVMCh
		failOutstandingCommands(cmdQueue)
		close(q.disconnectedCh)
	}()

	version := []byte{}
	var cmdDoneCh <-chan struct{}

DONE:
	for {
		var ok bool
		select {
		case cmd, ok := <-q.cmdCh:
			if !ok {
				return
			}
			_ = cmdQueue.PushBack(&cmd)
		case version, ok = <-fromVMCh:
			if !ok {
				return
			}
			if cmdQueue.Len() >= 1 {
				q.writeNextQMPCommand(cmdQueue)
				cmdDoneCh = currentCommandDoneCh(cmdQueue)
			}
			break DONE
		}
	}

	q.connectedCh <- q.parseVersion(version)

	for {
		select {
		case cmd, ok := <-q.cmdCh:
			if !ok {
				return
			}
			_ = cmdQueue.PushBack(&cmd)

			// We only want to execute the new cmd if there
			// are no other commands pending.  If there are
			// commands pending our new command will get
			// run when the pending commands complete.

			if cmdQueue.Len() == 1 {
				q.writeNextQMPCommand(cmdQueue)
				cmdDoneCh = currentCommandDoneCh(cmdQueue)
			}
		case line, ok := <-fromVMCh:
			if !ok {
				return
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
	filter *qmpEventFilter) (interface{}, error) {
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

	_, err := q.executeCommandWithResponse(ctx, name, args, filter)
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
		return nil, nil, fmt.Errorf("Canceled by caller")
	case <-disconnectedCh:
		return nil, nil, fmt.Errorf("Lost connection to VM")
	case q.version = <-connectedCh:
		if q.version == nil {
			return nil, nil, fmt.Errorf("Failed to find QMP version information")
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

// ExecuteBlockdevAdd sends a blockdev-add to the QEMU instance.  device is the
// path of the device to add, e.g., /dev/rdb0, and blockdevID is an identifier
// used to name the device.  As this identifier will be passed directly to QMP,
// it must obey QMP's naming rules, e,g., it must start with a letter.
func (q *QMP) ExecuteBlockdevAdd(ctx context.Context, device, blockdevID string) error {
	var args map[string]interface{}

	blockdevArgs := map[string]interface{}{
		"driver": "raw",
		"file": map[string]interface{}{
			"driver":   "file",
			"filename": device,
		},
	}

	if q.version.Major > 2 || (q.version.Major == 2 && q.version.Minor >= 9) {
		blockdevArgs["node-name"] = blockdevID
		args = blockdevArgs
	} else {
		blockdevArgs["id"] = blockdevID
		args = map[string]interface{}{
			"options": blockdevArgs,
		}
	}

	return q.executeCommand(ctx, "blockdev-add", args, nil)
}

// ExecuteDeviceAdd adds the guest portion of a device to a QEMU instance
// using the device_add command.  blockdevID should match the blockdevID passed
// to a previous call to ExecuteBlockdevAdd.  devID is the id of the device to
// add.  Both strings must be valid QMP identifiers.  driver is the name of the
// driver,e.g., virtio-blk-pci, and bus is the name of the bus.  bus is optional.
func (q *QMP) ExecuteDeviceAdd(ctx context.Context, blockdevID, devID, driver, bus string) error {
	args := map[string]interface{}{
		"id":     devID,
		"driver": driver,
		"drive":  blockdevID,
	}
	if bus != "" {
		args["bus"] = bus
	}
	return q.executeCommand(ctx, "device_add", args, nil)
}

// ExecuteSCSIDeviceAdd adds the guest portion of a block device to a QEMU instance
// using a SCSI driver with the device_add command.  blockdevID should match the
// blockdevID passed to a previous call to ExecuteBlockdevAdd.  devID is the id of
// the device to add.  Both strings must be valid QMP identifiers.  driver is the name of the
// scsi driver,e.g., scsi-hd, and bus is the name of a SCSI controller bus.
// scsiID is the SCSI id, lun is logical unit number. scsiID and lun are optional, a negative value
// for scsiID and lun is ignored.
func (q *QMP) ExecuteSCSIDeviceAdd(ctx context.Context, blockdevID, devID, driver, bus string, scsiID, lun int) error {
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
		return fmt.Errorf("Invalid SCSI driver provided %s", driver)
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

	args["id"] = blockdevID
	return q.executeCommand(ctx, "x-blockdev-del", args, nil)
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
// device address on its parent bus. bus is optional.
func (q *QMP) ExecutePCIDeviceAdd(ctx context.Context, blockdevID, devID, driver, addr, bus string) error {
	args := map[string]interface{}{
		"id":     devID,
		"driver": driver,
		"drive":  blockdevID,
		"addr":   addr,
	}
	if bus != "" {
		args["bus"] = bus
	}
	return q.executeCommand(ctx, "device_add", args, nil)
}

// ExecuteVFIODeviceAdd adds a VFIO device to a QEMU instance
// using the device_add command. devID is the id of the device to add.
// Must be valid QMP identifier. bdf is the PCI bus-device-function
// of the pci device.
func (q *QMP) ExecuteVFIODeviceAdd(ctx context.Context, devID, bdf string) error {
	args := map[string]interface{}{
		"id":     devID,
		"driver": "vfio-pci",
		"host":   bdf,
	}
	return q.executeCommand(ctx, "device_add", args, nil)
}

// ExecutePCIVFIODeviceAdd adds a VFIO device to a QEMU instance using the device_add command.
// This function can be used to hot plug VFIO devices on PCI(E) bridges, unlike
// ExecuteVFIODeviceAdd this function receives the bus and the device address on its parent bus.
// bus is optional. devID is the id of the device to add.Must be valid QMP identifier. bdf is the
// PCI bus-device-function of the pci device.
func (q *QMP) ExecutePCIVFIODeviceAdd(ctx context.Context, devID, bdf, addr, bus string) error {
	args := map[string]interface{}{
		"id":     devID,
		"driver": "vfio-pci",
		"host":   bdf,
		"addr":   addr,
	}
	if bus != "" {
		args["bus"] = bus
	}
	return q.executeCommand(ctx, "device_add", args, nil)
}

// ExecuteCPUDeviceAdd adds a CPU to a QEMU instance using the device_add command.
// driver is the CPU model, cpuID must be a unique ID to identify the CPU, socketID is the socket number within
// node/board the CPU belongs to, coreID is the core number within socket the CPU belongs to, threadID is the
// thread number within core the CPU belongs to.
func (q *QMP) ExecuteCPUDeviceAdd(ctx context.Context, driver, cpuID, socketID, coreID, threadID string) error {
	args := map[string]interface{}{
		"driver":    driver,
		"id":        cpuID,
		"socket-id": socketID,
		"core-id":   coreID,
		"thread-id": threadID,
	}
	return q.executeCommand(ctx, "device_add", args, nil)
}

// ExecuteQueryHotpluggableCPUs returns a slice with the list of hotpluggable CPUs
func (q *QMP) ExecuteQueryHotpluggableCPUs(ctx context.Context) ([]HotpluggableCPU, error) {
	response, err := q.executeCommandWithResponse(ctx, "query-hotpluggable-cpus", nil, nil)
	if err != nil {
		return nil, err
	}

	// convert response to json
	data, err := json.Marshal(response)
	if err != nil {
		return nil, fmt.Errorf("Unable to extract CPU information: %v", err)
	}

	var cpus []HotpluggableCPU
	// convert json to []HotpluggableCPU
	if err = json.Unmarshal(data, &cpus); err != nil {
		return nil, fmt.Errorf("Unable to convert json to hotpluggable CPU: %v", err)
	}

	return cpus, nil
}
