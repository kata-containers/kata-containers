//
// Copyright (c) 2017-2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"
	"strings"
)

const (
	// "source=agent" logs are actually encoded within proxy logs so need
	// to be unpacked.
	agentSourceField = "agent"
)

// agentLogEntry returns true if the specified log entry actually contains
// an encoded agent log entry.
func agentLogEntry(le LogEntry) bool {
	if le.Source != agentSourceField {
		return false
	}

	msg := le.Msg
	if msg == "" {
		return false
	}

	if strings.HasPrefix(msg, "time=") {
		return true
	}

	return false
}

// unpackAgentLogEntry unpacks the proxy log entry that encodes an agent
// message and returns the agent log entry, discarding the proxy log entry
// that held it.
func unpackAgentLogEntry(le LogEntry) (agent LogEntry, err error) {
	if le.Source != agentSourceField {
		return LogEntry{}, fmt.Errorf("agent log entry has wrong source (expected %v, got %v): %+v",
			agentSourceField, le.Source, le)
	}

	msg := le.Msg
	if msg == "" {
		return LogEntry{}, fmt.Errorf("no agent message data (entry %+v", le)
	}

	file := le.Filename
	if file == "" {
		return LogEntry{}, fmt.Errorf("filename blank (entry %+v)", le)
	}

	line := le.Line
	if line == 0 {
		return LogEntry{}, fmt.Errorf("invalid line number (entry %+v)", le)
	}

	reader := strings.NewReader(le.Msg)

	entries, err := parseLogFmtData(reader, file)
	if err != nil {
		return LogEntry{}, fmt.Errorf("failed to parse agent log entry %+v: %v", le, err)
	}

	expectedRecords := 1

	count := entries.Len()
	if count != expectedRecords {
		return LogEntry{}, fmt.Errorf("expected %d record, got %d", expectedRecords, count)
	}

	agent = entries.Entries[0]

	// Supplement the agent entry with a few extra details
	agent.Count = le.Count
	agent.Source = agentSourceField
	agent.Filename = file
	agent.Line = line

	return agent, nil
}
