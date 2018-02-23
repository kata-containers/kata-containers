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

package hyperstart_test

import (
	"math"
	"net"
	"reflect"
	"testing"
	"time"

	. "github.com/containers/virtcontainers/pkg/hyperstart"
	"github.com/containers/virtcontainers/pkg/hyperstart/mock"
)

const (
	testSockType = "unix"
	testSequence = uint64(100)
	testMessage  = "test_message"
)

func connectHyperstartNoMulticast(h *Hyperstart) error {
	return h.OpenSocketsNoMulticast()
}

func connectHyperstart(h *Hyperstart) error {
	return h.OpenSockets()
}

func disconnectHyperstart(h *Hyperstart) {
	h.CloseSockets()
}

func connectMockHyperstart(t *testing.T, multiCast bool) (*mock.Hyperstart, *Hyperstart, error) {
	mockHyper := mock.NewHyperstart(t)

	mockHyper.Start()

	ctlSock, ioSock := mockHyper.GetSocketPaths()

	h := NewHyperstart(ctlSock, ioSock, testSockType)

	var err error
	if multiCast {
		err = connectHyperstart(h)
	} else {
		err = connectHyperstartNoMulticast(h)
	}
	if err != nil {
		mockHyper.Stop()
		return nil, nil, err
	}

	return mockHyper, h, nil
}

func TestNewHyperstart(t *testing.T) {
	ctlSock := "/tmp/test_hyper.sock"
	ioSock := "/tmp/test_tty.sock"
	sockType := "test_unix"

	h := NewHyperstart(ctlSock, ioSock, sockType)

	resultCtlSockPath := h.GetCtlSockPath()
	resultIoSockPath := h.GetIoSockPath()
	resultSockType := h.GetSockType()

	if resultCtlSockPath != ctlSock {
		t.Fatalf("CTL sock result %s should be the same than %s", resultCtlSockPath, ctlSock)
	}

	if resultIoSockPath != ioSock {
		t.Fatalf("IO sock result %s should be the same than %s", resultIoSockPath, ioSock)
	}

	if resultSockType != sockType {
		t.Fatalf("Sock type result %s should be the same than %s", resultSockType, sockType)
	}
}

func TestOpenSockets(t *testing.T) {
	mockHyper := mock.NewHyperstart(t)

	mockHyper.Start()

	ctlSock, ioSock := mockHyper.GetSocketPaths()

	h := NewHyperstart(ctlSock, ioSock, testSockType)

	err := h.OpenSockets()
	if err != nil {
		mockHyper.Stop()
		t.Fatal()
	}

	mockHyper.Stop()

	disconnectHyperstart(h)
}

func TestCloseSockets(t *testing.T) {
	mockHyper, h, err := connectMockHyperstart(t, true)
	if err != nil {
		t.Fatal()
	}

	mockHyper.Stop()

	err = h.CloseSockets()
	if err != nil {
		t.Fatal()
	}
}

func TestSetDeadline(t *testing.T) {
	mockHyper, h, err := connectMockHyperstart(t, false)
	if err != nil {
		t.Fatal()
	}
	defer disconnectHyperstart(h)
	defer mockHyper.Stop()

	timeoutDuration := 1 * time.Second

	err = h.SetDeadline(time.Now().Add(timeoutDuration))
	if err != nil {
		t.Fatal()
	}

	mockHyper.SendMessage(ReadyCode, []byte{})

	buf := make([]byte, 512)
	_, err = h.GetCtlSock().Read(buf)
	if err != nil {
		t.Fatal()
	}

	err = h.SetDeadline(time.Now().Add(timeoutDuration))
	if err != nil {
		t.Fatal()
	}

	time.Sleep(timeoutDuration)

	_, err = h.GetCtlSock().Read(buf)
	netErr, ok := err.(net.Error)
	if ok && netErr.Timeout() == false {
		t.Fatal()
	}
}

func TestIsStartedFalse(t *testing.T) {
	h := &Hyperstart{}

	if h.IsStarted() == true {
		t.Fatal()
	}
}

func TestIsStartedTrue(t *testing.T) {
	mockHyper, h, err := connectMockHyperstart(t, true)
	if err != nil {
		t.Fatal()
	}
	defer disconnectHyperstart(h)
	defer mockHyper.Stop()

	if h.IsStarted() == false {
		t.Fatal()
	}
}

func testFormatMessage(t *testing.T, payload interface{}, expected []byte) {
	res, err := FormatMessage(payload)
	if err != nil {
		t.Fatal()
	}

	if reflect.DeepEqual(res, expected) == false {
		t.Fatal()
	}
}

func TestFormatMessageFromString(t *testing.T) {
	payload := testMessage
	expectedOut := []byte(payload)

	testFormatMessage(t, payload, expectedOut)
}

type TestStruct struct {
	FieldString string `json:"fieldString"`
	FieldInt    int    `json:"fieldInt"`
}

func TestFormatMessageFromStruct(t *testing.T) {
	payload := TestStruct{
		FieldString: "test_string",
		FieldInt:    100,
	}

	expectedOut := []byte("{\"fieldString\":\"test_string\",\"fieldInt\":100}")

	testFormatMessage(t, payload, expectedOut)
}

func TestReadCtlMessage(t *testing.T) {
	mockHyper, h, err := connectMockHyperstart(t, false)
	if err != nil {
		t.Fatal()
	}
	defer disconnectHyperstart(h)
	defer mockHyper.Stop()

	expected := &DecodedMessage{
		Code:    ReadyCode,
		Message: []byte{},
	}

	mockHyper.SendMessage(int(expected.Code), expected.Message)

	reply, err := ReadCtlMessage(h.GetCtlSock())
	if err != nil {
		t.Fatal()
	}

	if reflect.DeepEqual(reply, expected) == false {
		t.Fatal()
	}
}

func TestWriteCtlMessage(t *testing.T) {
	mockHyper, h, err := connectMockHyperstart(t, false)
	if err != nil {
		t.Fatal()
	}
	defer disconnectHyperstart(h)
	defer mockHyper.Stop()

	msg := DecodedMessage{
		Code:    PingCode,
		Message: []byte{},
	}

	err = h.WriteCtlMessage(h.GetCtlSock(), &msg)
	if err != nil {
		t.Fatal()
	}

	for {
		reply, err := ReadCtlMessage(h.GetCtlSock())
		if err != nil {
			t.Fatal()
		}

		if reply.Code == NextCode {
			continue
		}

		err = h.CheckReturnedCode(reply, AckCode)
		if err != nil {
			t.Fatal()
		}

		break
	}

	msgs := mockHyper.GetLastMessages()
	if msgs == nil {
		t.Fatal()
	}

	if msgs[0].Code != msg.Code || string(msgs[0].Message) != string(msg.Message) {
		t.Fatal()
	}
}

func TestReadIoMessage(t *testing.T) {
	mockHyper, h, err := connectMockHyperstart(t, true)
	if err != nil {
		t.Fatal()
	}
	defer disconnectHyperstart(h)
	defer mockHyper.Stop()

	mockHyper.SendIo(testSequence, []byte(testMessage))

	msg, err := h.ReadIoMessage()
	if err != nil {
		t.Fatal()
	}

	if msg.Session != testSequence || string(msg.Message) != testMessage {
		t.Fatal()
	}
}

func TestReadIoMessageWithConn(t *testing.T) {
	mockHyper, h, err := connectMockHyperstart(t, true)
	if err != nil {
		t.Fatal()
	}
	defer disconnectHyperstart(h)
	defer mockHyper.Stop()

	mockHyper.SendIo(testSequence, []byte(testMessage))

	msg, err := ReadIoMessageWithConn(h.GetIoSock())
	if err != nil {
		t.Fatal()
	}

	if msg.Session != testSequence || string(msg.Message) != testMessage {
		t.Fatal()
	}
}

func TestSendIoMessage(t *testing.T) {
	mockHyper, h, err := connectMockHyperstart(t, true)
	if err != nil {
		t.Fatal()
	}
	defer disconnectHyperstart(h)
	defer mockHyper.Stop()

	msg := &TtyMessage{
		Session: testSequence,
		Message: []byte(testMessage),
	}

	err = h.SendIoMessage(msg)
	if err != nil {
		t.Fatal()
	}

	buf := make([]byte, 512)
	n, seqRecv := mockHyper.ReadIo(buf)

	if seqRecv != testSequence || string(buf[TtyHdrSize:n]) != testMessage {
		t.Fatal()
	}
}

func TestSendIoMessageWithConn(t *testing.T) {
	mockHyper, h, err := connectMockHyperstart(t, true)
	if err != nil {
		t.Fatal()
	}
	defer disconnectHyperstart(h)
	defer mockHyper.Stop()

	msg := &TtyMessage{
		Session: testSequence,
		Message: []byte(testMessage),
	}

	err = SendIoMessageWithConn(h.GetIoSock(), msg)
	if err != nil {
		t.Fatal()
	}

	buf := make([]byte, 512)
	n, seqRecv := mockHyper.ReadIo(buf)

	if seqRecv != testSequence || string(buf[TtyHdrSize:n]) != testMessage {
		t.Fatal()
	}
}

func testCodeFromCmd(t *testing.T, cmd string, expected uint32) {
	h := &Hyperstart{}

	code, err := h.CodeFromCmd(cmd)
	if err != nil || code != expected {
		t.Fatal()
	}
}

func TestCodeFromCmdVersion(t *testing.T) {
	testCodeFromCmd(t, Version, VersionCode)
}

func TestCodeFromCmdStartPod(t *testing.T) {
	testCodeFromCmd(t, StartPod, StartPodCode)
}

func TestCodeFromCmdDestroyPod(t *testing.T) {
	testCodeFromCmd(t, DestroyPod, DestroyPodCode)
}

func TestCodeFromCmdExecCmd(t *testing.T) {
	testCodeFromCmd(t, ExecCmd, ExecCmdCode)
}

func TestCodeFromCmdReady(t *testing.T) {
	testCodeFromCmd(t, Ready, ReadyCode)
}

func TestCodeFromCmdAck(t *testing.T) {
	testCodeFromCmd(t, Ack, AckCode)
}

func TestCodeFromCmdError(t *testing.T) {
	testCodeFromCmd(t, Error, ErrorCode)
}

func TestCodeFromCmdWinSize(t *testing.T) {
	testCodeFromCmd(t, WinSize, WinsizeCode)
}

func TestCodeFromCmdPing(t *testing.T) {
	testCodeFromCmd(t, Ping, PingCode)
}

func TestCodeFromCmdNext(t *testing.T) {
	testCodeFromCmd(t, Next, NextCode)
}

func TestCodeFromCmdWriteFile(t *testing.T) {
	testCodeFromCmd(t, WriteFile, WriteFileCode)
}

func TestCodeFromCmdReadFile(t *testing.T) {
	testCodeFromCmd(t, ReadFile, ReadFileCode)
}

func TestCodeFromCmdNewContainer(t *testing.T) {
	testCodeFromCmd(t, NewContainer, NewContainerCode)
}

func TestCodeFromCmdKillContainer(t *testing.T) {
	testCodeFromCmd(t, KillContainer, KillContainerCode)
}

func TestCodeFromCmdOnlineCPUMem(t *testing.T) {
	testCodeFromCmd(t, OnlineCPUMem, OnlineCPUMemCode)
}

func TestCodeFromCmdSetupInterface(t *testing.T) {
	testCodeFromCmd(t, SetupInterface, SetupInterfaceCode)
}

func TestCodeFromCmdSetupRoute(t *testing.T) {
	testCodeFromCmd(t, SetupRoute, SetupRouteCode)
}

func TestCodeFromCmdRemoveContainer(t *testing.T) {
	testCodeFromCmd(t, RemoveContainer, RemoveContainerCode)
}

func TestCodeFromCmdUnknown(t *testing.T) {
	h := &Hyperstart{}

	code, err := h.CodeFromCmd("unknown")
	if err == nil || code != math.MaxUint32 {
		t.Fatal()
	}
}

func testCheckReturnedCode(t *testing.T, recvMsg *DecodedMessage, refCode uint32) {
	h := &Hyperstart{}

	err := h.CheckReturnedCode(recvMsg, refCode)
	if err != nil {
		t.Fatal()
	}
}

func TestCheckReturnedCodeList(t *testing.T) {
	for _, code := range CodeList {
		recvMsg := DecodedMessage{Code: code}
		testCheckReturnedCode(t, &recvMsg, code)
	}
}

func testCheckReturnedCodeFailure(t *testing.T, recvMsg *DecodedMessage, refCode uint32) {
	h := &Hyperstart{}

	err := h.CheckReturnedCode(recvMsg, refCode)
	if err == nil {
		t.Fatal()
	}
}

func TestCheckReturnedCodeListWrong(t *testing.T) {
	for _, code := range CodeList {
		msg := DecodedMessage{Code: code}
		if code != ReadyCode {
			testCheckReturnedCodeFailure(t, &msg, ReadyCode)
		} else {
			testCheckReturnedCodeFailure(t, &msg, PingCode)
		}
	}
}

func TestWaitForReady(t *testing.T) {
	mockHyper, h, err := connectMockHyperstart(t, true)
	if err != nil {
		t.Fatal()
	}
	defer disconnectHyperstart(h)
	defer mockHyper.Stop()

	mockHyper.SendMessage(int(ReadyCode), []byte{})

	err = h.WaitForReady()
	if err != nil {
		t.Fatal()
	}
}

func TestWaitForReadyError(t *testing.T) {
	mockHyper, h, err := connectMockHyperstart(t, true)
	if err != nil {
		t.Fatal()
	}
	defer disconnectHyperstart(h)
	defer mockHyper.Stop()

	mockHyper.SendMessage(int(ErrorCode), []byte{})

	err = h.WaitForReady()
	if err == nil {
		t.Fatal()
	}
}

var cmdList = []string{
	Version,
	StartPod,
	DestroyPod,
	ExecCmd,
	Ready,
	Ack,
	Error,
	WinSize,
	Ping,
	Next,
	NewContainer,
	KillContainer,
	OnlineCPUMem,
	SetupInterface,
	SetupRoute,
	RemoveContainer,
}

func testSendCtlMessage(t *testing.T, cmd string) {
	mockHyper, h, err := connectMockHyperstart(t, true)
	if err != nil {
		t.Fatal()
	}
	defer disconnectHyperstart(h)
	defer mockHyper.Stop()

	msg, err := h.SendCtlMessage(cmd, []byte{})
	if err != nil {
		t.Fatal()
	}

	if msg.Code != AckCode {
		t.Fatal()
	}
}

func TestSendCtlMessage(t *testing.T) {
	for _, cmd := range cmdList {
		testSendCtlMessage(t, cmd)
	}
}
