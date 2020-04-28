// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Package uuid can be used to generate 128 bit UUIDs compatible with
// rfc4122.  Currently, only version 4 UUIDs, UUIDs generated from random
// data, can be created.  The package includes functions for generating
// UUIDs and for converting them to and from strings.
package uuid

import (
	"crypto/rand"
	"encoding/binary"
	"errors"
	"fmt"
	"io"
	"strconv"
	"strings"
)

// UUID represents a single 128 bit UUID as an array of 16 bytes.
type UUID [16]byte

// UUIDRegex defines a pattern for validating UUIDs
const UUIDRegex = "[a-fA-F0-9]{8}-?[a-fA-F0-9]{4}-?4[a-fA-F0-9]{3}-?[8|9|aA|bB][a-fA-F0-9]{3}-?[a-fA-F0-9]{12}"

var (
	// ErrUUIDInvalid indicates that a UIID is invalid.  Currently,
	// returned by uuid.Parse if the string passed to this function
	// does not contain a valid UUID.
	ErrUUIDInvalid = errors.New("invalid uuid")
)

func encode4bytes(n uint64, b []byte) {
	binary.BigEndian.PutUint32(b, uint32(n))
}

func encode2bytes(n uint64, b []byte) {
	binary.BigEndian.PutUint16(b, uint16(n))
}

func encode1byte(n uint64, b []byte) {
	b[0] = uint8(n)
}

func encode6bytes(n uint64, b []byte) {
	d := make([]byte, 8)
	binary.BigEndian.PutUint64(d, n)
	copy(b, d[2:])
}

func stringToBE(s string, b []byte, f func(uint64, []byte)) error {
	num, err := strconv.ParseUint(s, 16, len(s)*4)
	if err != nil {
		return ErrUUIDInvalid
	}
	f(num, b)
	return nil
}

// Parse returns the binary encoding of the UUID passed in the s parameter.
// The error ErrUUIDInvalid will be returned if s does not represent a valid
// UUID.
func Parse(s string) (UUID, error) {
	var uuid UUID
	var segmentSizes = [...]int{8, 4, 4, 4, 12}

	segments := strings.Split(s, "-")
	if len(segments) != len(segmentSizes) {
		return uuid, ErrUUIDInvalid
	}

	for i, l := range segmentSizes {
		if len(segments[i]) != l {
			return uuid, ErrUUIDInvalid
		}
	}

	if err := stringToBE(segments[0], uuid[:4], encode4bytes); err != nil {
		return uuid, err
	}
	if err := stringToBE(segments[1], uuid[4:6], encode2bytes); err != nil {
		return uuid, err
	}
	if err := stringToBE(segments[2], uuid[6:8], encode2bytes); err != nil {
		return uuid, err
	}
	if err := stringToBE(segments[3][:2], uuid[8:9], encode1byte); err != nil {
		return uuid, err
	}
	if err := stringToBE(segments[3][2:], uuid[9:10], encode1byte); err != nil {
		return uuid, err
	}
	if err := stringToBE(segments[4], uuid[10:], encode6bytes); err != nil {
		return uuid, err
	}

	return uuid, nil
}

// Generate generates a new v4 UUID, i.e., a random UUID.
func Generate() UUID {
	var u UUID

	_, err := io.ReadFull(rand.Reader, u[:])
	if err != nil {
		panic(fmt.Errorf("Unable to read random data : %v", err))
	}

	u[6] = (u[6] & 0x0f) | 0x40
	u[8] = (u[8] & 0x3f) | 0x80

	return u
}

func (u UUID) String() string {
	timeLow := binary.BigEndian.Uint32(u[:4])
	timeMid := binary.BigEndian.Uint16(u[4:6])
	timeHi := binary.BigEndian.Uint16(u[6:8])
	clkSeqHi := u[8]
	clkSeqLow := u[9]
	buf := make([]byte, 8)
	copy(buf[2:], u[10:])
	node := binary.BigEndian.Uint64(buf)

	return fmt.Sprintf("%08x-%04x-%04x-%02x%02x-%012x",
		timeLow, timeMid, timeHi, clkSeqHi, clkSeqLow, node)
}
