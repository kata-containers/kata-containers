// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package mock

import (
	"encoding/json"
	"errors"
	"fmt"
	"net"
	"os"
	"sync"
	"testing"

	"github.com/clearcontainers/proxy/api"
	"github.com/stretchr/testify/assert"
)

const testToken = "pF56IaDpuax6hihJ5PneB8JypqmOvjkqY-wKGVYqgIM="

// CCProxyMock is an object mocking clearcontainers Proxy
type CCProxyMock struct {
	sync.Mutex

	t              *testing.T
	wg             sync.WaitGroup
	connectionPath string

	// proxy socket
	listener net.Listener

	// single client to serve
	cl net.Conn

	//token to be used for the connection
	token string

	lastStdinStream []byte

	ShimConnected    chan bool
	Signal           chan ShimSignal
	ShimDisconnected chan bool
	StdinReceived    chan bool

	stopped bool
}

// NewCCProxyMock creates a hyperstart instance
func NewCCProxyMock(t *testing.T, path string) *CCProxyMock {
	return &CCProxyMock{
		t:                t,
		connectionPath:   path,
		lastStdinStream:  nil,
		Signal:           make(chan ShimSignal, 5),
		ShimConnected:    make(chan bool),
		ShimDisconnected: make(chan bool),
		StdinReceived:    make(chan bool),
		token:            testToken,
	}
}

// GetProxyToken returns the token that mock proxy uses
// to verify its client connection
func (proxy *CCProxyMock) GetProxyToken() string {
	return proxy.token
}

// GetLastStdinStream returns the last received stdin stream
func (proxy *CCProxyMock) GetLastStdinStream() []byte {
	return proxy.lastStdinStream
}

func (proxy *CCProxyMock) log(s string) {
	proxy.logF("%s\n", s)
}

func (proxy *CCProxyMock) logF(format string, args ...interface{}) {
	proxy.t.Logf("[CCProxyMock] "+format, args...)
}

type client struct {
	proxy *CCProxyMock
	conn  net.Conn
}

// ConnectShim payload defined here, as it has not been defined
// in proxy api package yet
type ConnectShim struct {
	Token string `json:"token"`
}

// ShimSignal is the struct used to represent the signal received from the shim
type ShimSignal struct {
	Signal int `json:"signalNumber"`
	Row    int `json:"rows,omitempty"`
	Column int `json:"columns,omitempty"`
}

func connectShimHandler(data []byte, userData interface{}, response *handlerResponse) {
	client := userData.(*client)
	proxy := client.proxy

	payload := ConnectShim{}
	err := json.Unmarshal(data, &payload)
	assert.Nil(proxy.t, err)

	if payload.Token != proxy.token {
		response.SetErrorMsg("Invalid Token")
	}

	proxy.logF("ConnectShim(token=%s)", payload.Token)

	response.AddResult("version", api.Version)
	proxy.ShimConnected <- true
}

func signalShimHandler(data []byte, userData interface{}, response *handlerResponse) {
	client := userData.(*client)
	proxy := client.proxy

	signalPayload := ShimSignal{}
	err := json.Unmarshal(data, &signalPayload)
	assert.Nil(proxy.t, err)

	proxy.logF("CCProxyMock received signal: %v", signalPayload)

	proxy.Signal <- signalPayload
}

func disconnectShimHandler(data []byte, userData interface{}, response *handlerResponse) {
	client := userData.(*client)
	proxy := client.proxy

	proxy.log("Client sent DisconnectShim Command")
	proxy.ShimDisconnected <- true
}

func stdinShimHandler(data []byte, userData interface{}, response *handlerResponse) {
	client := userData.(*client)
	proxy := client.proxy

	proxy.lastStdinStream = data
	proxy.StdinReceived <- true
}

func registerVMHandler(data []byte, userData interface{}, response *handlerResponse) {
	client := userData.(*client)
	proxy := client.proxy

	proxy.log("Register VM")

	payload := api.RegisterVM{}
	if err := json.Unmarshal(data, &payload); err != nil {
		response.SetError(err)
		return
	}

	// Generate fake tokens
	var tokens []string
	for i := 0; i < payload.NumIOStreams; i++ {
		tokens = append(tokens, fmt.Sprintf("%d", i))
	}

	io := &api.IOResponse{
		Tokens: tokens,
	}

	response.AddResult("io", io)
}

func unregisterVMHandler(data []byte, userData interface{}, response *handlerResponse) {
	client := userData.(*client)
	proxy := client.proxy

	proxy.log("Unregister VM")
}

func attachVMHandler(data []byte, userData interface{}, response *handlerResponse) {
	client := userData.(*client)
	proxy := client.proxy

	proxy.log("Attach VM")

	payload := api.AttachVM{}
	if err := json.Unmarshal(data, &payload); err != nil {
		response.SetError(err)
		return
	}

	// Generate fake tokens
	var tokens []string
	for i := 0; i < payload.NumIOStreams; i++ {
		tokens = append(tokens, fmt.Sprintf("%d", i))
	}

	io := &api.IOResponse{
		Tokens: tokens,
	}

	response.AddResult("io", io)
}

func hyperCmdHandler(data []byte, userData interface{}, response *handlerResponse) {
	client := userData.(*client)
	proxy := client.proxy

	proxy.log("Hyper command")

	response.SetData([]byte{})
}

// SendStdoutStream sends a Stdout Stream Frame to connected client
func (proxy *CCProxyMock) SendStdoutStream(payload []byte) {
	err := api.WriteStream(proxy.cl, api.StreamStdout, payload)
	assert.Nil(proxy.t, err)
}

// SendStderrStream sends a Stderr Stream Frame to connected client
func (proxy *CCProxyMock) SendStderrStream(payload []byte) {
	err := api.WriteStream(proxy.cl, api.StreamStderr, payload)
	assert.Nil(proxy.t, err)
}

// SendExitNotification sends an Exit Notification Frame to connected client
func (proxy *CCProxyMock) SendExitNotification(payload []byte) {
	err := api.WriteNotification(proxy.cl, api.NotificationProcessExited, payload)
	assert.Nil(proxy.t, err)
}

func (proxy *CCProxyMock) startListening() {

	l, err := net.ListenUnix("unix", &net.UnixAddr{Name: proxy.connectionPath, Net: "unix"})
	assert.Nil(proxy.t, err)

	proxy.logF("listening on %s", proxy.connectionPath)

	proxy.listener = l
}

func (proxy *CCProxyMock) serveClient(proto *ccProxyProtocol, newConn net.Conn) {
	newClient := &client{
		proxy: proxy,
		conn:  newConn,
	}
	err := proto.Serve(newConn, newClient)
	proxy.logF("Error serving client : %v\n", err)

	newConn.Close()
	proxy.log("Client closed connection")

	proxy.wg.Done()
}

func (proxy *CCProxyMock) serve() {
	proto := newCCProxyProtocol()

	// shim handlers
	proto.Handle(FrameKey{api.TypeCommand, int(api.CmdConnectShim)}, connectShimHandler)
	proto.Handle(FrameKey{api.TypeCommand, int(api.CmdDisconnectShim)}, disconnectShimHandler)
	proto.Handle(FrameKey{api.TypeStream, int(api.StreamStdin)}, stdinShimHandler)

	// runtime handlers
	proto.Handle(FrameKey{api.TypeCommand, int(api.CmdRegisterVM)}, registerVMHandler)
	proto.Handle(FrameKey{api.TypeCommand, int(api.CmdUnregisterVM)}, unregisterVMHandler)
	proto.Handle(FrameKey{api.TypeCommand, int(api.CmdAttachVM)}, attachVMHandler)
	proto.Handle(FrameKey{api.TypeCommand, int(api.CmdHyper)}, hyperCmdHandler)

	// Shared handler between shim and runtime
	proto.Handle(FrameKey{api.TypeCommand, int(api.CmdSignal)}, signalShimHandler)

	//Wait for a single client connection
	conn, err := proxy.listener.Accept()
	if err != nil {
		// Ending up into this case when the listener is closed, which
		// is still a valid case. We don't want to throw an error in
		// this case.
		return
	}

	assert.NotNil(proxy.t, conn)
	proxy.log("Client connected")

	proxy.wg.Add(1)

	proxy.cl = conn

	proxy.serveClient(proto, conn)
}

// Start invokes mock proxy instance to start listening.
func (proxy *CCProxyMock) Start() {
	proxy.stopped = false
	proxy.startListening()
	go func() {
		for {
			proxy.serve()

			proxy.Lock()
			stopped := proxy.stopped
			proxy.Unlock()

			if stopped {
				break
			}
		}
	}()
}

// Stop causes  mock proxy instance to stop listening,
// close connection to client and close all channels
func (proxy *CCProxyMock) Stop() {
	proxy.Lock()
	proxy.stopped = true
	proxy.Unlock()

	proxy.listener.Close()

	if proxy.cl != nil {
		proxy.log("Closing client connection")
		proxy.cl.Close()
		proxy.cl = nil
	} else {
		proxy.log("Client connection already closed")
	}

	proxy.wg.Wait()
	close(proxy.ShimConnected)
	close(proxy.Signal)
	close(proxy.ShimDisconnected)
	close(proxy.StdinReceived)
	os.Remove(proxy.connectionPath)
	proxy.log("Stopped")
}

// XXX: could do with its own package to remove that ugly namespacing
type ccProxyProtocolHandler func([]byte, interface{}, *handlerResponse)

// Encapsulates the different parts of what a handler can return.
type handlerResponse struct {
	err     error
	results map[string]interface{}
	data    []byte
}

// SetError indicates sets error for the response.
func (r *handlerResponse) SetError(err error) {
	r.err = err
}

// SetErrorMsg sets an error with the passed string for the response.
func (r *handlerResponse) SetErrorMsg(msg string) {
	r.err = errors.New(msg)
}

// SetErrorf sets an error with the formatted string for the response.
func (r *handlerResponse) SetErrorf(format string, a ...interface{}) {
	r.SetError(fmt.Errorf(format, a...))
}

// AddResult adds the given key/val to the response.
func (r *handlerResponse) AddResult(key string, value interface{}) {
	if r.results == nil {
		r.results = make(map[string]interface{})
	}
	r.results[key] = value
}

func (r *handlerResponse) SetData(data []byte) {
	r.data = data
}

// FrameKey is a struct composed of the the frame type and opcode,
// used as a key for retrieving the handler for handling the frame.
type FrameKey struct {
	ftype  api.FrameType
	opcode int
}

type ccProxyProtocol struct {
	cmdHandlers map[FrameKey]ccProxyProtocolHandler
}

func newCCProxyProtocol() *ccProxyProtocol {
	return &ccProxyProtocol{
		cmdHandlers: make(map[FrameKey]ccProxyProtocolHandler),
	}
}

// Handle retreives the handler for handling the frame
func (proto *ccProxyProtocol) Handle(key FrameKey, handler ccProxyProtocolHandler) bool {
	if _, ok := proto.cmdHandlers[key]; ok {
		return false
	}
	proto.cmdHandlers[key] = handler
	return true
}

type clientCtx struct {
	conn     net.Conn
	userData interface{}
}

func newErrorResponse(opcode int, errMsg string) *api.Frame {
	frame, err := api.NewFrameJSON(api.TypeResponse, opcode, &api.ErrorResponse{
		Message: errMsg,
	})

	if err != nil {
		frame, err = api.NewFrameJSON(api.TypeResponse, opcode, &api.ErrorResponse{
			Message: fmt.Sprintf("couldn't marshal response: %v", err),
		})
		if err != nil {
			frame = api.NewFrame(api.TypeResponse, opcode, nil)
		}
	}

	frame.Header.InError = true
	return frame
}

func (proto *ccProxyProtocol) handleCommand(ctx *clientCtx, cmd *api.Frame) *api.Frame {
	hr := handlerResponse{}

	// cmd.Header.Opcode is guaranteed to be within the right bounds by
	// ReadFrame().
	handler := proto.cmdHandlers[FrameKey{cmd.Header.Type, cmd.Header.Opcode}]

	handler(cmd.Payload, ctx.userData, &hr)
	if hr.err != nil {
		return newErrorResponse(cmd.Header.Opcode, hr.err.Error())
	}

	var payload interface{}
	if len(hr.results) > 0 {
		payload = hr.results
	}

	frame, err := api.NewFrameJSON(api.TypeResponse, cmd.Header.Opcode, payload)
	if err != nil {
		return newErrorResponse(cmd.Header.Opcode, err.Error())
	}
	return frame
}

// Serve serves the client connection in a continuous loop.
func (proto *ccProxyProtocol) Serve(conn net.Conn, userData interface{}) error {
	ctx := &clientCtx{
		conn:     conn,
		userData: userData,
	}

	for {
		frame, err := api.ReadFrame(conn)
		if err != nil {
			// EOF or the client isn't even sending proper JSON,
			// just kill the connection
			return err
		}

		if frame.Header.Type != api.TypeCommand && frame.Header.Type != api.TypeStream {
			// EOF or the client isn't even sending proper JSON,
			// just kill the connection
			return fmt.Errorf("serve: expected a command got a %v", frame.Header.Type)
		}

		// Execute the corresponding handler
		resp := proto.handleCommand(ctx, frame)

		// Send the response back to the client.
		if err = api.WriteFrame(conn, resp); err != nil {
			// Something made us unable to write the response back
			// to the client (could be a disconnection, ...).
			return err
		}
	}
}
