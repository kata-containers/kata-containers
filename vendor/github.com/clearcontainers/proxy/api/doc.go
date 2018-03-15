// Copyright (c) 2017 Intel Corporation
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

// Package api defines the API cc-proxy exposes to clients (processes
// connecting to the proxy AF_UNIX socket).
//
// This package contains the low level definitions of the protocol, frame
// structure and the various payloads that can be sent and received.
//
// The proxy protocol is composed of commands, responses and notifications.
// They all share the same frame structure: a header followed by an optional
// payload.
//
// • Commands are always initiated by a client, never by the proxy itself.
//
// • Responses are sent by the proxy to acknowledge commands.
//
// • Notifications are sent by either the proxy or clients and do not generate
// responses.
//
// Frame Structure
//
// The frame format is illustrated below:
//
//                      1 1 1 1 1 1 1 1 1 2 2 2 2 2 2 2 2 2 2 3 3
//  0 1 2 3 4 5 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
//  ┌───────────────────────────┬───────────────┬───────────────┐
//  │          Version          │ Header Length │   Reserved    │
//  ├───────────────────────────┼─────┬─┬───────┼───────────────┤
//  │          Reserved         │ Res.│E│ Type  │    Opcode     │
//  ├───────────────────────────┴─────┴─┴───────┴───────────────┤
//  │                      Payload Length                       │
//  ├───────────────────────────────────────────────────────────┤
//  │                                                           │
//  │                         Payload                           │
//  │                                                           │
//  │      (variable length, optional and opcode-specific)      │
//  │                                                           │
//  └───────────────────────────────────────────────────────────┘
//
// All header fields are encoded in network order (big endian).
//
// • Version (16 bits) is the proxy protocol version. See api.Version for
// details about what information it encodes.
//
// • Header Length (8 bits) is the length of the header in number of 32-bit
// words.  Header Length is greater or equal to 3 (12 bytes).
//
// • Type (4 bits) is the frame type: command (0x0), response (0x1),
// stream (0x2) or notification (0x3).
//
// • Opcode (8 bits) specifies the kind of command, response, stream or
// notification this frame represents. In conjunction with Type, this field
// will dictate the payload content.
//
// • E, Error. This flag is set when a response returns an error. Currently
// Error can ony be set in response frames.
//
// • Payload Length (32 bits) is in bytes.
//
// • Payload is optional data that can be sent with the various frames.
// Commands, responses and notifications usually encode their payloads in JSON
// while stream frames have raw data payloads.
//
// • Reserved fields are reserved for future use and must be zeroed.
//
// Frame Size and Header Length
//
// The full size of a frame is (Header Length + Payload Length). The Payload
// starts at offset Header Length from the start of the frame.
//
// It is guaranteed that future header sizes will be at least 12 bytes.
package api
