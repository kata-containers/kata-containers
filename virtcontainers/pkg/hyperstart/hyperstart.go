//
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
//

package hyperstart

import (
	"encoding/binary"
	"encoding/json"
	"fmt"
	"math"
	"net"
	"sync"
	"time"

	"github.com/sirupsen/logrus"
)

// Control command IDs
// Need to be in sync with hyperstart/src/api.h
const (
	Version         = "version"
	StartPod        = "startpod"
	DestroyPod      = "destroypod"
	ExecCmd         = "execcmd"
	Ready           = "ready"
	Ack             = "ack"
	Error           = "error"
	WinSize         = "winsize"
	Ping            = "ping"
	FinishPod       = "finishpod"
	Next            = "next"
	WriteFile       = "writefile"
	ReadFile        = "readfile"
	NewContainer    = "newcontainer"
	KillContainer   = "killcontainer"
	OnlineCPUMem    = "onlinecpumem"
	SetupInterface  = "setupinterface"
	SetupRoute      = "setuproute"
	RemoveContainer = "removecontainer"
	PsContainer     = "pscontainer"
)

// CodeList is the map making the relation between a string command
// and its corresponding code.
var CodeList = map[string]uint32{
	Version:         VersionCode,
	StartPod:        StartPodCode,
	DestroyPod:      DestroyPodCode,
	ExecCmd:         ExecCmdCode,
	Ready:           ReadyCode,
	Ack:             AckCode,
	Error:           ErrorCode,
	WinSize:         WinsizeCode,
	Ping:            PingCode,
	Next:            NextCode,
	WriteFile:       WriteFileCode,
	ReadFile:        ReadFileCode,
	NewContainer:    NewContainerCode,
	KillContainer:   KillContainerCode,
	OnlineCPUMem:    OnlineCPUMemCode,
	SetupInterface:  SetupInterfaceCode,
	SetupRoute:      SetupRouteCode,
	RemoveContainer: RemoveContainerCode,
	PsContainer:     PsContainerCode,
}

// Values related to the communication on control channel.
const (
	CtlHdrSize      = 8
	CtlHdrLenOffset = 4
)

// Values related to the communication on tty channel.
const (
	TtyHdrSize      = 12
	TtyHdrLenOffset = 8
)

type connState struct {
	sync.Mutex
	opened bool
}

func (c *connState) close() {
	c.Lock()
	defer c.Unlock()

	c.opened = false
}

func (c *connState) open() {
	c.Lock()
	defer c.Unlock()

	c.opened = true
}

func (c *connState) closed() bool {
	c.Lock()
	defer c.Unlock()

	return !c.opened
}

// Hyperstart is the base structure for hyperstart.
type Hyperstart struct {
	ctlSerial, ioSerial string
	sockType            string
	ctl, io             net.Conn
	ctlState, ioState   connState

	// ctl access is arbitrated by ctlMutex. We can only allow a single
	// "transaction" (write command + read answer) at a time
	ctlMutex sync.Mutex

	ctlMulticast *multicast

	ctlChDone chan interface{}
}

var hyperLog = logrus.FieldLogger(logrus.New())

// SetLogger sets the logger for hyperstart package.
func SetLogger(logger logrus.FieldLogger) {
	hyperLog = logger.WithField("source", "virtcontainers/hyperstart")
}

// NewHyperstart returns a new hyperstart structure.
func NewHyperstart(ctlSerial, ioSerial, sockType string) *Hyperstart {
	return &Hyperstart{
		ctlSerial: ctlSerial,
		ioSerial:  ioSerial,
		sockType:  sockType,
	}
}

// GetCtlSock returns the internal CTL sock.
func (h *Hyperstart) GetCtlSock() net.Conn {
	return h.ctl
}

// GetIoSock returns the internal IO sock.
func (h *Hyperstart) GetIoSock() net.Conn {
	return h.io
}

// GetCtlSockPath returns the internal CTL sock path.
func (h *Hyperstart) GetCtlSockPath() string {
	return h.ctlSerial
}

// GetIoSockPath returns the internal IO sock path.
func (h *Hyperstart) GetIoSockPath() string {
	return h.ioSerial
}

// GetSockType returns the internal sock type.
func (h *Hyperstart) GetSockType() string {
	return h.sockType
}

// OpenSocketsNoMulticast opens both CTL and IO sockets, without
// starting the multicast.
func (h *Hyperstart) OpenSocketsNoMulticast() error {
	var err error

	h.ctl, err = net.Dial(h.sockType, h.ctlSerial)
	if err != nil {
		return err
	}
	h.ctlState.open()

	h.io, err = net.Dial(h.sockType, h.ioSerial)
	if err != nil {
		h.ctl.Close()
		return err
	}
	h.ioState.open()

	return nil
}

// OpenSockets opens both CTL and IO sockets.
func (h *Hyperstart) OpenSockets() error {
	if err := h.OpenSocketsNoMulticast(); err != nil {
		return err
	}

	h.ctlChDone = make(chan interface{})
	h.ctlMulticast = startCtlMonitor(h.ctl, h.ctlChDone)

	return nil
}

// CloseSockets closes both CTL and IO sockets.
func (h *Hyperstart) CloseSockets() error {
	if !h.ctlState.closed() {
		if h.ctlChDone != nil {
			// Wait for the CTL channel to be terminated.
			select {
			case <-h.ctlChDone:
				break
			case <-time.After(time.Duration(3) * time.Second):
				return fmt.Errorf("CTL channel did not end as expected")
			}
		}

		err := h.ctl.Close()
		if err != nil {
			return err
		}

		h.ctlState.close()
	}

	if !h.ioState.closed() {
		err := h.io.Close()
		if err != nil {
			return err
		}

		h.ioState.close()
	}

	h.ctlMulticast = nil

	return nil
}

// SetDeadline sets a timeout for CTL connection.
func (h *Hyperstart) SetDeadline(t time.Time) error {
	err := h.ctl.SetDeadline(t)
	if err != nil {
		return err
	}

	return nil
}

// IsStarted returns about connection status.
func (h *Hyperstart) IsStarted() bool {
	ret := false
	timeoutDuration := 1 * time.Second

	if h.ctlState.closed() {
		return ret
	}

	h.SetDeadline(time.Now().Add(timeoutDuration))

	_, err := h.SendCtlMessage(Ping, nil)
	if err == nil {
		ret = true
	}

	h.SetDeadline(time.Time{})

	if ret == false {
		h.CloseSockets()
	}

	return ret
}

// FormatMessage formats hyperstart messages.
func FormatMessage(payload interface{}) ([]byte, error) {
	var payloadSlice []byte
	var err error

	if payload != nil {
		switch p := payload.(type) {
		case string:
			payloadSlice = []byte(p)
		default:
			payloadSlice, err = json.Marshal(p)
			if err != nil {
				return nil, err
			}
		}
	}

	return payloadSlice, nil
}

// ReadCtlMessage reads an hyperstart message from conn and returns a decoded message.
//
// This is a low level function, for a full and safe transaction on the
// hyperstart control serial link, use SendCtlMessage.
func ReadCtlMessage(conn net.Conn) (*DecodedMessage, error) {
	needRead := CtlHdrSize
	length := 0
	read := 0
	buf := make([]byte, 512)
	res := []byte{}
	for read < needRead {
		want := needRead - read
		if want > 512 {
			want = 512
		}
		nr, err := conn.Read(buf[:want])
		if err != nil {
			return nil, err
		}

		res = append(res, buf[:nr]...)
		read = read + nr

		if length == 0 && read >= CtlHdrSize {
			length = int(binary.BigEndian.Uint32(res[CtlHdrLenOffset:CtlHdrSize]))
			if length > CtlHdrSize {
				needRead = length
			}
		}
	}

	return &DecodedMessage{
		Code:    binary.BigEndian.Uint32(res[:CtlHdrLenOffset]),
		Message: res[CtlHdrSize:],
	}, nil
}

// WriteCtlMessage writes an hyperstart message to conn.
//
// This is a low level function, for a full and safe transaction on the
// hyperstart control serial link, use SendCtlMessage.
func (h *Hyperstart) WriteCtlMessage(conn net.Conn, m *DecodedMessage) error {
	length := len(m.Message) + CtlHdrSize
	// XXX: Support sending messages by chunks to support messages over
	// 10240 bytes. That limit is from hyperstart src/init.c,
	// hyper_channel_ops, rbuf_size.
	if length > 10240 {
		return fmt.Errorf("message too long %d", length)
	}
	msg := make([]byte, length)
	binary.BigEndian.PutUint32(msg[:], uint32(m.Code))
	binary.BigEndian.PutUint32(msg[CtlHdrLenOffset:], uint32(length))
	copy(msg[CtlHdrSize:], m.Message)

	_, err := conn.Write(msg)
	if err != nil {
		return err
	}

	return nil
}

// ReadIoMessageWithConn returns data coming from the specified IO channel.
func ReadIoMessageWithConn(conn net.Conn) (*TtyMessage, error) {
	needRead := TtyHdrSize
	length := 0
	read := 0
	buf := make([]byte, 512)
	res := []byte{}
	for read < needRead {
		want := needRead - read
		if want > 512 {
			want = 512
		}
		nr, err := conn.Read(buf[:want])
		if err != nil {
			return nil, err
		}

		res = append(res, buf[:nr]...)
		read = read + nr

		if length == 0 && read >= TtyHdrSize {
			length = int(binary.BigEndian.Uint32(res[TtyHdrLenOffset:TtyHdrSize]))
			if length > TtyHdrSize {
				needRead = length
			}
		}
	}

	return &TtyMessage{
		Session: binary.BigEndian.Uint64(res[:TtyHdrLenOffset]),
		Message: res[TtyHdrSize:],
	}, nil
}

// ReadIoMessage returns data coming from the IO channel.
func (h *Hyperstart) ReadIoMessage() (*TtyMessage, error) {
	return ReadIoMessageWithConn(h.io)
}

// SendIoMessageWithConn sends data to the specified IO channel.
func SendIoMessageWithConn(conn net.Conn, ttyMsg *TtyMessage) error {
	length := len(ttyMsg.Message) + TtyHdrSize
	// XXX: Support sending messages by chunks to support messages over
	// 10240 bytes. That limit is from hyperstart src/init.c,
	// hyper_channel_ops, rbuf_size.
	if length > 10240 {
		return fmt.Errorf("message too long %d", length)
	}
	msg := make([]byte, length)
	binary.BigEndian.PutUint64(msg[:], ttyMsg.Session)
	binary.BigEndian.PutUint32(msg[TtyHdrLenOffset:], uint32(length))
	copy(msg[TtyHdrSize:], ttyMsg.Message)

	n, err := conn.Write(msg)
	if err != nil {
		return err
	}

	if n != length {
		return fmt.Errorf("%d bytes written out of %d expected", n, length)
	}

	return nil
}

// SendIoMessage sends data to the IO channel.
func (h *Hyperstart) SendIoMessage(ttyMsg *TtyMessage) error {
	return SendIoMessageWithConn(h.io, ttyMsg)
}

// CodeFromCmd translates a string command to its corresponding code.
func (h *Hyperstart) CodeFromCmd(cmd string) (uint32, error) {
	_, ok := CodeList[cmd]
	if ok == false {
		return math.MaxUint32, fmt.Errorf("unknown command '%s'", cmd)
	}

	return CodeList[cmd], nil
}

// CheckReturnedCode ensures we did not receive an ERROR code.
func (h *Hyperstart) CheckReturnedCode(recvMsg *DecodedMessage, expectedCode uint32) error {
	if recvMsg.Code != expectedCode {
		if recvMsg.Code == ErrorCode {
			return fmt.Errorf("ERROR received from VM agent, control msg received : %s", recvMsg.Message)
		}

		return fmt.Errorf("CMD ID received %d not matching expected %d, control msg received : %s", recvMsg.Code, expectedCode, recvMsg.Message)
	}

	return nil
}

// WaitForReady waits for a READY message on CTL channel.
func (h *Hyperstart) WaitForReady() error {
	if h.ctlMulticast == nil {
		return fmt.Errorf("No multicast available for CTL channel")
	}

	channel, err := h.ctlMulticast.listen("", "", replyType)
	if err != nil {
		return err
	}

	msg := <-channel

	err = h.CheckReturnedCode(msg, ReadyCode)
	if err != nil {
		return err
	}

	return nil
}

// WaitForPAE waits for a PROCESSASYNCEVENT message on CTL channel.
func (h *Hyperstart) WaitForPAE(containerID, processID string) (*PAECommand, error) {
	if h.ctlMulticast == nil {
		return nil, fmt.Errorf("No multicast available for CTL channel")
	}

	channel, err := h.ctlMulticast.listen(containerID, processID, eventType)
	if err != nil {
		return nil, err
	}

	msg := <-channel

	var paeData PAECommand
	err = json.Unmarshal(msg.Message, paeData)
	if err != nil {
		return nil, err
	}

	return &paeData, nil
}

// SendCtlMessage sends a message to the CTL channel.
//
// This function does a full transaction over the CTL channel: it will rely on the
// multicaster to register a listener reading over the CTL channel. Then it writes
// a command and waits for the multicaster to send hyperstart's answer back before
// it can return.
// Several concurrent calls to SendCtlMessage are allowed, the function ensuring
// proper serialization of the communication by making the listener registration
// and the command writing an atomic operation protected by a mutex.
// Waiting for the reply from multicaster doesn't need to be protected by this mutex.
func (h *Hyperstart) SendCtlMessage(cmd string, data []byte) (*DecodedMessage, error) {
	if h.ctlMulticast == nil {
		return nil, fmt.Errorf("No multicast available for CTL channel")
	}

	h.ctlMutex.Lock()

	channel, err := h.ctlMulticast.listen("", "", replyType)
	if err != nil {
		h.ctlMutex.Unlock()
		return nil, err
	}

	code, err := h.CodeFromCmd(cmd)
	if err != nil {
		h.ctlMutex.Unlock()
		return nil, err
	}

	msgSend := &DecodedMessage{
		Code:    code,
		Message: data,
	}
	err = h.WriteCtlMessage(h.ctl, msgSend)
	if err != nil {
		h.ctlMutex.Unlock()
		return nil, err
	}

	h.ctlMutex.Unlock()

	msgRecv := <-channel

	err = h.CheckReturnedCode(msgRecv, AckCode)
	if err != nil {
		return nil, err
	}

	return msgRecv, nil
}
