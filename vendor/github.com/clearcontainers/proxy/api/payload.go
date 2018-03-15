// Copyright (c) 2016,2017 Intel Corporation
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

package api

import (
	"encoding/json"
)

// The RegisterVM payload is issued first after connecting to the proxy socket.
// It is used to let the proxy know about a new container on the system along
// with the paths go hyperstart's command and I/O channels (AF_UNIX sockets).
//
// Console can be used to indicate the path of a socket linked to the VM
// console. The proxy can output this data when asked for verbose output.
//
//  {
//    "containerId": "756535dc6e9ab9b560f84c8...",
//    "ctlSerial": "/tmp/sh.hyper.channel.0.sock",
//    "ioSerial": "/tmp/sh.hyper.channel.1.sock",
//    "numIOStreams: 1
//  }
type RegisterVM struct {
	ContainerID string `json:"containerId"`
	CtlSerial   string `json:"ctlSerial"`
	IoSerial    string `json:"ioSerial"`
	Console     string `json:"console,omitempty"`
	// NumIOStreams asks for a number of I/O tokens. An I/O token
	// represents the communication between a container process inside
	// the VM and a shim process outside the VM. This communication
	// includes I/O streams (stdin, out, err) but also signals, exit
	// status, ...
	// The response frame will contain NumIOStreams I/O tokens.
	NumIOStreams int `json:"numIOStreams,omitempty"`
}

// IOResponse is the response data in RegisterVMResponse and AttachVMResponse
// when the client is asking for I/O tokens from the proxy (NumIOStreams > 0).
type IOResponse struct {
	// URL is the URL a shim process should connect to in order to initiate
	// the I/O communication with the process inside the VM
	URL string
	// IOTokens is a array of I/O tokens of length NumIOStreams. See
	// RegisterVM for some details on I/O tokens.
	Tokens []string `json:"tokens"`
}

// RegisterVMResponse is the result from a successful RegisterVM.
//
//  {
//    "io": {
//      "url": "unix:///run/clearcontainers/proxy.sock",
//      "tokens": [
//        "bwgxfmQj9uG3YCsFHrvontwDw41CJJ76Y7qVt4Bi9wc="
//      ]
//    }
//  }
type RegisterVMResponse struct {
	// IO contains the proxy answer when asking for I/O tokens.
	IO IOResponse `json:"io,omitempty"`
}

// The AttachVM payload can be used to associate clients to an already known
// VM. AttachVM cannot be issued if a RegisterVM for this container hasn't been
// issued beforehand.
//
//  {
//    "containerId": "756535dc6e9ab9b560f84c8...".
//    "numIOStreams: 1
//  }
type AttachVM struct {
	ContainerID string `json:"containerId"`
	// NumIOStreams asks for a number of I/O tokens. See RegisterVM for
	// some details on I/O tokens.
	NumIOStreams int `json:"numIOStreams,omitempty"`
}

// AttachVMResponse is the result from a successful AttachVM.
//
//  {
//    "io": {
//      "url": "unix:///run/clearcontainers/proxy.sock",
//      "tokens": [
//        "bwgxfmQj9uG3YCsFHrvontwDw41CJJ76Y7qVt4Bi9wc="
//      ]
//    }
//  }
type AttachVMResponse struct {
	// IO contains the proxy answer when asking for I/O tokens.
	IO IOResponse `json:"io,omitempty"`
}

// The UnregisterVM payload does the opposite of what RegisterVM does,
// indicating to the proxy it should release resources created by RegisterVM
// for the container identified by containerId.
//
//  {
//    "containerId": "756535dc6e9ab9b560f84c8..."
//  }
type UnregisterVM struct {
	ContainerID string `json:"containerId"`
}

// The Hyper payload will forward an hyperstart command to hyperstart.
//
// Note: the newcontainer and execmd hyperstart commands start one or more
// processes. When sending those commands, tokens acquired through either
// RegisterVM or AttachVM need to be sent along in the tokens array. The number
// of tokens sent has to match the number of processes to be started.
//
//  {
//    "hyperName": "newcontainer",
//    "tokens": [
//      "bwgxfmQj9uG3YCsFHrvontwDw41CJJ76Y7qVt4Bi9wc="
//    ],
//    "data": {
//      "id": "756535dc6e9ab9b560f84c8...",
//      "rootfs": "/foo/bar",
//      ...
//      }
//    }
//  }
type Hyper struct {
	HyperName string          `json:"hyperName"`
	Tokens    []string        `json:"tokens"`
	Data      json.RawMessage `json:"data,omitempty"`
}

// ConnectShim identifies a shim against the proxy. A shim process is a process
// running on host shadowing a container process running inside the VM. A shim
// will forward stdin and signals to the process inside the VM and will receive
// stdout, stderr and the exit status.
type ConnectShim struct {
	// Token is id corresponding to the process the shim wants to handle
	// the I/O streams, signals, exit status for. Tokens are allocated with
	// a call to RegisterVM or AttachVM.
	Token string `json:"token"`
}

// DisconnectShim unregister a shim from the proxy.
type DisconnectShim struct {
}

// Signal is used to send signals to the container process inside the VM. This
// payload is only valid after a successful ConnectShim.
type Signal struct {
	SignalNumber int `json:"signalNumber"`
	// Columns is only valid for SIGWINCH and is the new number of columns of
	// the terminal.
	Columns int `json:"columns,omitempty"`
	// Rows is only valid for SIGWINCH and is the new number of rows of the
	// terminal.
	Rows int `json:"rows,omitempty"`
}

// ErrorResponse is the payload send in Responses where the Error flag is set.
type ErrorResponse struct {
	Message string `json:"msg"`
}

// LogEntry is the payload for the StreamLog data.
type LogEntry struct {
	// Source is the source of the log entry. One of "shim" or "runtime".
	Source string `json:"source"`
	// ContainerID is the ID of the container the log entry is for (optional).
	ContainerID string `json:"containerId,omitempty"`
	// Level is the verbosity level of the log entry. One of "debug", "info", "warn"
	// or "error".
	Level string `json:"level"`
	// Message is the log message
	Message string `json:"msg"`
}
