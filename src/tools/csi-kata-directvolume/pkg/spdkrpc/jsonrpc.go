//
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
)

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

var requestID = 1

func CallSPDK(method string, params map[string]interface{}) (string, error) {
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

	conn.SetReadDeadline(time.Now().Add(3 * time.Second))
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
		return "", fmt.Errorf("SPDK returned error: %v", resp.Error)
	}

	return fmt.Sprintf("%v", resp.Result), nil
}
