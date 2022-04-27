//
// Copyright (c) 2017-2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"errors"
	"fmt"
	"io"
	"regexp"
	"sort"
	"strconv"
	"strings"
	"time"

	"github.com/go-logfmt/logfmt"
)

const (
	// Tell time.Parse() how to handle the various logfile timestamp
	// formats by providing a number of formats for the "magic" data the
	// golang time package mandates:
	//
	//     "Mon Jan 2 15:04:05 -0700 MST 2006"
	//
	dateFormat = "2006-01-02T15:04:05.999999999Z07:00"

	// The timezone of an RFC3339 timestamp can either be "Z" to denote
	// UTC, or "+/-HH:MM" to denote an actual offset.
	timezonePattern = `(` +
		`Z` +
		`|` +
		`[\+|\-]\d{2}:\d{2}` +
		`)`

	dateFormatPattern =
	// YYYY-MM-DD
	`\d{4}-\d{2}-\d{2}` +
		// time separator
		`T` +
		// HH:MM:SS
		`\d{2}:\d{2}:\d{2}` +
		// high-precision separator
		`.` +
		// nano-seconds. Note that the quantifier range is
		// required because the time.RFC3339Nano format
		// trunctates trailing zeros.
		`\d{1,9}` +
		// timezone
		timezonePattern

	agentContainerIDPattern = `container_id:"([^"]*)"`
)

type kvPair struct {
	key   string
	value string
}

type kvPairs []kvPair

var (
	dateFormatRE       *regexp.Regexp
	agentContainerIDRE *regexp.Regexp
)

func init() {
	dateFormatRE = regexp.MustCompile(dateFormatPattern)
	agentContainerIDRE = regexp.MustCompile(agentContainerIDPattern)
}

// parseLogFmtData reads logfmt records using the provided reader and returns
// log entries.
//
// Note that the filename specified is not validated - it is added to the
// returned log entries and also used for returned errors.
func parseLogFmtData(reader io.Reader, file string, ignoreMissingFields bool) (LogEntries, error) {
	entries := LogEntries{}

	d := logfmt.NewDecoder(reader)

	line := uint64(0)

	// A record is a single line
	for d.ScanRecord() {
		line++
		var keyvals kvPairs

		// split the line into key/value pairs
		for d.ScanKeyval() {
			key := string(d.Key())
			value := string(d.Value())

			// If agent debug is enabled, every gRPC request ("req")
			// is logged. Since most such requests contain the
			// container ID as a `container_id` field, extract and
			// save it when present.
			//
			// See: https://github.com/kata-containers/agent/blob/master/protocols/grpc/agent.proto
			//
			// Note that we save the container ID in addition to
			// the original value.
			if key == "req" {
				matches := agentContainerIDRE.FindSubmatch([]byte(value))
				if matches != nil {
					containerID := string(matches[1])

					pair := kvPair{
						key:   "container",
						value: containerID,
					}

					// save key/value pair
					keyvals = append(keyvals, pair)
				}
			}

			pair := kvPair{
				key:   key,
				value: value,
			}

			// save key/value pair
			keyvals = append(keyvals, pair)
		}

		if err := d.Err(); err != nil {
			return LogEntries{},
				fmt.Errorf("failed to parse file %q, line %d: %v (keyvals: %+v)",
					file, line, err, keyvals)

		}

		entry, err := createLogEntry(file, line, keyvals)
		if err != nil {
			return LogEntries{}, err
		}

		err = entry.Check(ignoreMissingFields)
		if err != nil {
			return LogEntries{}, err
		}

		entries.Entries = append(entries.Entries, entry)
	}

	if d.Err() != nil {
		return LogEntries{},
			fmt.Errorf("failed to parse file %q line %d: %v", file, line, d.Err())
	}

	return entries, nil
}

// parseLogFile reads a logfmt format logfile and converts it into log
// entries.
func parseLogFile(file string, ignoreMissingFields bool) (LogEntries, error) {
	// logfmt is unhappy attempting to read hex-encoded bytes in strings,
	// so hide those from it by escaping them.
	reader := NewHexByteReader(file)

	return parseLogFmtData(reader, file, ignoreMissingFields)
}

// parseLogFiles parses all log files, sorts the results by timestamp and
// returns the collated results
func parseLogFiles(files []string, ignoreMissingFields bool) (LogEntries, error) {
	entries := LogEntries{
		FormatVersion: logEntryFormatVersion,
	}

	for _, file := range files {
		e, err := parseLogFile(file, ignoreMissingFields)
		if err != nil {
			return LogEntries{}, err
		}

		entries.Entries = append(entries.Entries, e.Entries...)
	}

	sort.Sort(entries)

	return entries, nil
}

// parseTime attempts to convert the specified timestamp string into a Time
// object by checking it against various known timestamp formats.
func parseTime(timeString string) (time.Time, error) {
	if timeString == "" {
		return time.Time{}, errors.New("need time string")
	}

	t, err := time.Parse(dateFormat, timeString)
	if err != nil {
		return time.Time{}, err
	}

	// time.Parse() is "clever" but also doesn't check anything more
	// granular than a second, so let's be completely paranoid and check
	// via regular expression too.
	matched := dateFormatRE.FindAllStringSubmatch(timeString, -1)
	if matched == nil {
		return time.Time{},
			fmt.Errorf("expected time in format %q, got %q", dateFormatPattern, timeString)
	}

	return t, nil
}

func checkKeyValueValid(key, value string) error {
	if key == "" {
		return fmt.Errorf("key cannot be blank (value: %q)", value)
	}

	if strings.TrimSpace(key) == "" {
		return fmt.Errorf("key cannot be pure whitespace (value: %q)", value)
	}

	err := checkValid(key)
	if err != nil {
		return fmt.Errorf("key %q is invalid (value: %q): %v", key, value, err)
	}

	err = checkValid(value)
	if err != nil {
		return fmt.Errorf("value %q is invalid (key: %v): %v", value, key, err)
	}

	return nil
}

// handleLogEntry takes a partial LogEntry and adds values to it based on the
// key and value specified.
func handleLogEntry(l *LogEntry, key, value string) (err error) {
	if l == nil {
		return errors.New("invalid LogEntry")
	}

	if err = checkKeyValueValid(key, value); err != nil {
		return fmt.Errorf("%v (entry: %+v)", err, l)
	}

	switch key {
	case "container":
		l.Container = value

	case "level":
		l.Level = value

	case "msg":
		l.Msg = value

	case "name":
		l.Name = value

	case "pid":
		pid := 0
		if value != "" {
			pid, err = strconv.Atoi(value)
			if err != nil {
				return fmt.Errorf("failed to parse pid from value %v (entry: %+v, key: %v): %v", value, l, key, err)
			}
		}

		l.Pid = pid

	case "sandbox":
		l.Sandbox = value

	case "source":
		l.Source = value

	case "time":
		t, err := parseTime(value)
		if err != nil {
			return fmt.Errorf("failed to parse time for value %v (entry: %+v, key: %v): %v", value, l, key, err)
		}

		l.Time = t

	default:
		if v, exists := l.Data[key]; exists {
			return fmt.Errorf("key %q already exists in map with value %q (entry: %+v)", key, v, l)
		}

		// non-standard fields are stored here
		l.Data[key] = value
	}

	return nil
}

// createLogEntry converts a logfmt record into a LogEntry.
func createLogEntry(filename string, line uint64, pairs kvPairs) (LogEntry, error) {
	if filename == "" {
		return LogEntry{}, fmt.Errorf("need filename")
	}

	if line == 0 {
		return LogEntry{}, fmt.Errorf("need line number for file %v", filename)
	}

	if pairs == nil || len(pairs) == 0 {
		return LogEntry{}, fmt.Errorf("need key/value pairs for line %v:%d", filename, line)
	}

	l := LogEntry{}

	l.Filename = filename
	l.Line = line
	l.Data = make(map[string]string)

	for _, v := range pairs {
		if err := handleLogEntry(&l, v.key, v.value); err != nil {
			return LogEntry{}, fmt.Errorf("%v (entry: %+v)", err, l)
		}
	}

	if !disableAgentUnpack && agentLogEntry(l) {
		agent, err := unpackAgentLogEntry(l)
		if err != nil {
			// allow the user to see that the unpack failed
			l.Data[agentUnpackFailTag] = "true"

			if strict {
				return LogEntry{}, err
			}

			logger.Warnf("failed to unpack agent log entry %v: %v", l, err)
		} else {
			// the agent log entry totally replaces the proxy log entry
			// that encapsulated it.
			l = agent
		}
	}

	return l, nil
}
