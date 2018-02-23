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

package mock

import (
	"encoding/binary"
	"encoding/hex"
	"fmt"
	"net"
	"os"
	"path/filepath"
	"sync"
	"testing"

	hyper "github.com/containers/virtcontainers/pkg/hyperstart"
	"github.com/stretchr/testify/assert"
)

// Control command string IDs
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
	RemoveContainer = "removecontainer"
	OnlineCPUMem    = "onlinecpumem"
	SetupInterface  = "setupinterface"
	SetupRoute      = "setuproute"
)

var codeList = map[int]string{
	hyper.VersionCode:         Version,
	hyper.StartPodCode:        StartPod,
	hyper.DestroyPodCode:      DestroyPod,
	hyper.ExecCmdCode:         ExecCmd,
	hyper.ReadyCode:           Ready,
	hyper.AckCode:             Ack,
	hyper.ErrorCode:           Error,
	hyper.WinsizeCode:         WinSize,
	hyper.PingCode:            Ping,
	hyper.NextCode:            Next,
	hyper.WriteFileCode:       WriteFile,
	hyper.ReadFileCode:        ReadFile,
	hyper.NewContainerCode:    NewContainer,
	hyper.KillContainerCode:   KillContainer,
	hyper.OnlineCPUMemCode:    OnlineCPUMem,
	hyper.SetupInterfaceCode:  SetupInterface,
	hyper.SetupRouteCode:      SetupRoute,
	hyper.RemoveContainerCode: RemoveContainer,
}

// Hyperstart is an object mocking the hyperstart agent.
type Hyperstart struct {
	t                           *testing.T
	ctlSocketPath, ioSocketPath string
	ctlListener, ioListener     *net.UnixListener
	ctl, io                     net.Conn

	// Start() will launch two goroutines to accept connections on the ctl
	// and io sockets. Those goroutine will exit once the first connection
	// is accepted or when the listening socket is closed. wgConnected can
	// be used to make sure we've accepted connections to both sockets
	wgConnected sync.WaitGroup

	// We then have two other goroutines to handle communication on those
	// sockets.
	wg sync.WaitGroup

	// Keep the list of messages received by hyperstart, older first, for
	// later inspection with GetLastMessages()
	lastMessages []hyper.DecodedMessage
}

func newMessageList() []hyper.DecodedMessage {
	return make([]hyper.DecodedMessage, 0, 10)
}

// NewHyperstart creates a new hyperstart instance.
func NewHyperstart(t *testing.T) *Hyperstart {
	dir := os.TempDir()
	ctlSocketPath := filepath.Join(dir, "mock.hyper."+nextSuffix()+".0.sock")
	ioSocketPath := filepath.Join(dir, "mock.hyper."+nextSuffix()+".1.sock")

	return &Hyperstart{
		t:             t,
		ctlSocketPath: ctlSocketPath,
		ioSocketPath:  ioSocketPath,
		lastMessages:  newMessageList(),
	}
}

// GetSocketPaths returns the ctl and io socket paths, respectively
func (h *Hyperstart) GetSocketPaths() (string, string) {
	return h.ctlSocketPath, h.ioSocketPath

}

// GetLastMessages returns list of messages received by hyperstart, older
// first. This function only returns the messages:
//  - since Start on the first invocation
//  - since the last GetLastMessages for subsequent invocations
func (h *Hyperstart) GetLastMessages() []hyper.DecodedMessage {
	msgs := h.lastMessages
	h.lastMessages = newMessageList()
	return msgs
}

func (h *Hyperstart) log(s string) {
	h.logf("%s\n", s)
}

func (h *Hyperstart) logf(format string, args ...interface{}) {
	h.t.Logf("[hyperstart] "+format, args...)
}

func (h *Hyperstart) logData(data []byte) {
	h.t.Log(hex.Dump(data))
}

//
// ctl channel
//

const ctlHeaderSize = 8

func (h *Hyperstart) writeCtl(data []byte) error {
	h.wgConnected.Wait()

	n, err := h.ctl.Write(data)
	if err != nil {
		return fmt.Errorf("Connection broken, cannot send data")
	}
	assert.Equal(h.t, n, len(data))

	return nil
}

// SendMessage makes hyperstart send the hyper command cmd along with optional
// data on the control channel
func (h *Hyperstart) SendMessage(cmd int, data []byte) {
	length := ctlHeaderSize + len(data)
	header := make([]byte, ctlHeaderSize)

	binary.BigEndian.PutUint32(header[:], uint32(cmd))
	binary.BigEndian.PutUint32(header[4:], uint32(length))

	err := h.writeCtl(header)
	if err != nil {
		return
	}

	if len(data) == 0 {
		return
	}

	h.writeCtl(data)
}

func (h *Hyperstart) readCtl(data []byte) error {
	h.wgConnected.Wait()

	n, err := h.ctl.Read(data)

	if err != nil {
		return err
	}
	assert.Equal(h.t, n, len(data))
	return nil
}

func (h *Hyperstart) ackData(nBytes int) {
	data := make([]byte, 4)
	binary.BigEndian.PutUint32(data[:], uint32(nBytes))
	h.SendMessage(hyper.NextCode, data)
}

func (h *Hyperstart) readMessage() (int, []byte, error) {
	buf := make([]byte, ctlHeaderSize)
	if err := h.readCtl(buf); err != nil {
		return -1, buf, err
	}

	h.ackData(len(buf))

	cmd := int(binary.BigEndian.Uint32(buf[:4]))
	length := int(binary.BigEndian.Uint32(buf[4:8]))
	assert.True(h.t, length >= 8)
	length -= 8
	if length == 0 {
		return cmd, nil, nil
	}

	data := make([]byte, length)
	if err := h.readCtl(data); err != nil {
		return -1, buf, err
	}

	h.ackData(len(data))

	return cmd, data, nil
}

func cmdToString(cmd int) (string, error) {
	_, ok := codeList[cmd]
	if ok == false {
		return "", fmt.Errorf("unknown command '%d'", cmd)
	}

	return codeList[cmd], nil
}

func (h *Hyperstart) handleCtl() {
	for {
		cmd, data, err := h.readMessage()
		if err != nil {
			break
		}
		cmdName, err := cmdToString(cmd)
		assert.Nil(h.t, err)
		h.logf("ctl: --> command %s, payload_len=%d\n", cmdName, len(data))
		if len(data) != 0 {
			h.logData(data)
		}

		h.lastMessages = append(h.lastMessages, hyper.DecodedMessage{
			Code:    uint32(cmd),
			Message: data,
		})

		// answer back with the message exit status
		// XXX: may be interesting to be able to configure the mock
		// hyperstart to fail and test the reaction of proxy/clients
		h.logf("ctl: <-- command %s executed successfully\n", cmdName)

		h.SendMessage(hyper.AckCode, nil)

	}

	h.wg.Done()
}

//
// io channel
//

const ioHeaderSize = 12

func (h *Hyperstart) writeIo(data []byte) {
	h.wgConnected.Wait()

	n, err := h.io.Write(data)
	assert.Nil(h.t, err)
	assert.Equal(h.t, n, len(data))
}

// SendIo sends a packet of I/O data to a client connected the I/O channel.
// Multiple I/O streams are multiplexed on that channel. seq specifies which
// steam the data belongs to.
func (h *Hyperstart) SendIo(seq uint64, data []byte) {
	length := ioHeaderSize + len(data)
	header := make([]byte, ioHeaderSize)

	h.logf("io: <-- writing %d bytes for seq %d\n", len(data), seq)

	binary.BigEndian.PutUint64(header[:], uint64(seq))
	binary.BigEndian.PutUint32(header[8:], uint32(length))
	h.writeIo(header)

	if len(data) == 0 {
		return
	}

	h.writeIo(data)
}

// SendIoString sends a string a client connected the I/O channel.
// Multiple I/O streams are multiplexed on that channel. seq specifies which
// steam the data belongs to.
func (h *Hyperstart) SendIoString(seq uint64, data string) {
	h.SendIo(seq, []byte(data))
}

// CloseIo closes the I/O stream specified by seq.
func (h *Hyperstart) CloseIo(seq uint64) {
	h.SendIo(seq, nil)
}

// SendExitStatus sends the exit status on the I/O streams specified by seq.
// The exit status should only be sent after the stream has been closed with
// CloseIo.
func (h *Hyperstart) SendExitStatus(seq uint64, exitStatus uint8) {
	status := []byte{exitStatus}
	h.SendIo(seq, status)
}

// ReadIo reads data that has been sent on the I/O channel by a client. It
// returns the full packet (header & data) as well as the seq number decoded
// from the header.
func (h *Hyperstart) ReadIo(buf []byte) (n int, seq uint64) {
	h.wgConnected.Wait()

	n, err := h.io.Read(buf)
	assert.Nil(h.t, err)

	seq = binary.BigEndian.Uint64(buf[:8])
	return
}

type acceptCb func(c net.Conn)

func (h *Hyperstart) startListening(path string, cb acceptCb) *net.UnixListener {

	addr := &net.UnixAddr{Name: path, Net: "unix"}
	l, err := net.ListenUnix("unix", addr)
	assert.Nil(h.t, err)

	go func() {
		h.logf("%s: waiting for connection\n", path)
		c, err := l.Accept()
		if err != nil {
			cb(nil)
			return
		}

		cb(c)
		h.logf("%s: accepted connection\n", path)
	}()

	return l
}

// Start will
// Once finished with the Hyperstart object, Close must be called.
func (h *Hyperstart) Start() {
	h.log("start")
	h.wgConnected.Add(1)
	h.wgConnected.Add(1)
	h.ctlListener = h.startListening(h.ctlSocketPath, func(s net.Conn) {
		// a client is now connected to the ctl socket
		h.ctl = s

		// Close() was called before we had a chance to accept a
		// connection.
		if s == nil {
			h.wgConnected.Done()
			return
		}

		// start the goroutine that will handle the ctl socket
		h.wg.Add(1)
		go h.handleCtl()

		// we need signal wgConnected late, so wg.Add(1) is done before
		// the wg.Wait() in Close()
		// See https://golang.org/pkg/sync/#WaitGroup.Wait:
		//   "Note that calls with a positive delta that occur when the
		//    counter is zero must happen before a Wait"
		h.wgConnected.Done()
	})

	h.ioListener = h.startListening(h.ioSocketPath, func(s net.Conn) {
		// a client is now connected to the ctl socket
		h.io = s
		h.wgConnected.Done()
	})
}

// Stop closes all internal resources and waits for goroutines started by Start
// to finish. Stop shouldn't be called if Start hasn't been called.
func (h *Hyperstart) Stop() {
	h.wgConnected.Wait()

	h.ctl.Close()
	h.io.Close()

	h.ctlListener.Close()
	h.ioListener.Close()

	h.wg.Wait()

	os.Remove(h.ctlSocketPath)
	os.Remove(h.ioSocketPath)

	h.log("stopped")
}
