// Copyright (c) 2025 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

package spdkrpc

import (
	"encoding/json"
	"fmt"
	"net"
	"time"

	"golang.org/x/sys/unix"
)

const SpdkErrNoDevice = -int(unix.ENODEV)

var Call = CallSPDK

type JsonRPCRequest struct {
	Method  string                 `json:"method"`
	Params  map[string]interface{} `json:"params"`
	ID      int                    `json:"id"`
	Jsonrpc string                 `json:"jsonrpc"`
}

type JsonRPCResponse struct {
	Result interface{} `json:"result"`
	Error  interface{} `json:"error"`
	ID     int         `json:"id"`
}

type SpdkError struct {
	Code    int    `json:"code"`
	Message string `json:"message"`
}

var (
	requestID  = 1
	RPCTimeout = 10 * time.Second
)

func Init(timeout time.Duration) {
	if timeout > 0 {
		RPCTimeout = timeout
	}
}

func (e *SpdkError) Error() string {
	return fmt.Sprintf("SPDK error %d: %s", e.Code, e.Message)
}

func CallSPDK(method string, params map[string]interface{}) (interface{}, error) {
	req := JsonRPCRequest{
		Method:  method,
		Params:  params,
		ID:      requestID,
		Jsonrpc: "2.0",
	}
	requestID++

	data, err := json.Marshal(req)
	if err != nil {
		return "", fmt.Errorf("failed to marshal request: %w", err)
	}

	conn, err := net.Dial("unix", "/var/tmp/spdk.sock")
	if err != nil {
		return "", fmt.Errorf("failed to connect to spdk.sock: %w", err)
	}
	defer conn.Close()

	_, err = conn.Write(data)
	if err != nil {
		return "", fmt.Errorf("failed to write to spdk.sock: %w", err)
	}

	conn.SetReadDeadline(time.Now().Add(RPCTimeout))

	buf := make([]byte, 8192)
	n, err := conn.Read(buf)
	if err != nil {
		return "", fmt.Errorf("failed to read from spdk.sock: %w", err)
	}

	var resp JsonRPCResponse
	if err := json.Unmarshal(buf[:n], &resp); err != nil {
		return "", fmt.Errorf("failed to unmarshal response: %w", err)
	}

	if resp.Error != nil {
		errBytes, _ := json.Marshal(resp.Error)
		var spdkErr SpdkError
		if json.Unmarshal(errBytes, &spdkErr) == nil && spdkErr.Code != 0 {
			return "", &spdkErr
		}
		return "", fmt.Errorf("SPDK returned error: %v", resp.Error)
	}

	// return fmt.Sprintf("%v", resp.Result), nil
	return resp.Result, nil
}
