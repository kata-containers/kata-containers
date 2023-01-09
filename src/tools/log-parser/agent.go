//
// Copyright (c) 2017-2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"
	"strconv"
	"strings"

	"encoding/json"
)

const (
	// "source=agent" logs are actually encoded within proxy logs so need
	// to be unpacked.
	agentSourceField = "agent"
)

// agentLogEntry returns true if the specified log entry actually contains
// an encoded agent log entry.
func agentLogEntry(le LogEntry) bool {
	if le.Source != agentSourceField && le.Source != "virtcontainers" {
		return false
	}

	// agent v1 format
	msg := le.Msg
	if msg == "" {
		return false
	}

	if msg == "reading guest console" {
		// v2 format - check if there is actually something on the console
		if le.Data["vmconsole"] != "" {
			return true
		}
	} else if strings.HasPrefix(msg, "time=") {
		return true
	}

	return false
}

// unpackAgentLogEntry unpacks the proxy log entry that encodes an agent
// message and returns the agent log entry, discarding the proxy log entry
// that held it.
func unpackAgentLogEntry(le LogEntry) (agent LogEntry, err error) {
	if le.Source == agentSourceField {
		return unpackAgentLogEntry_v1(le)
	}
	if le.Msg == "reading guest console" {
		return unpackAgentLogEntry_v2(le)
	}

	return LogEntry{}, fmt.Errorf("agent log entry not found (source: %v - msg: %v): %+v",
		le.Source, le.Msg, le)
}

func unpackAgentLogEntry_v1(le LogEntry) (agent LogEntry, err error) {
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

	entries, err := parseLogFmtData(reader, file, false)
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

func unpackAgentLogEntry_v2(le LogEntry) (agent LogEntry, err error) {

	agent = le

	// we expect the agent's message to be in JSON, under le.Data["vmconsole"]
	var result map[string]string
	err = json.Unmarshal([]byte(le.Data["vmconsole"]), &result)
	if err != nil {
		// entry is not in JSON format. Use the vmconsole field as the msg, and keep the rest of the log entry unmodified
		agent.Msg = le.Data["vmconsole"]
		agent.Source = "vmconsole"
		return agent, nil
	}
	// we were able to unpack agent msg, let's drop previous Data (runtime entries)
	agent.Data = make(map[string]string)

	for k, v := range result {
		switch k {
		case "container-id", "cid": // better to ensure consistency in the agent
			agent.Container = v
		case "level":
			switch v {
			// match agent's slog short version log leveling to the common log levels
			case "CRIT":
				agent.Level = "critical"
			case "DEBG":
				agent.Level = "debug"
			case "ERRO":
				agent.Level = "error"
			case "INFO":
				agent.Level = "info"
			case "TRCE":
				agent.Level = "trace"
			case "WARN":
				agent.Level = "warning"
			default:
				agent.Level = v
			}
			agent.Data[k] = v // in any case let's leave original level under Data
		case "msg":
			agent.Msg = v
		case "name":
			agent.Name = v
		case "pid":
			pid, err := strconv.Atoi(v)
			if err != nil {
				return LogEntry{}, fmt.Errorf("failed to convert pid")
			}
			agent.Pid = pid
		case "source":
			agent.Source = v
		default:
			// NOTE: we do not take the agent's timestamp into account, because there is a ~1sec delay
			// for the agent's log to get through. Using the agent's timestamp would then make its logs
			// appear out of order compared to other logs from the guest.
			// The agent's logs timestamp is still visible for reference in the Data section of the logs.
			agent.Data[k] = v
		}
	}

	return agent, nil
}
