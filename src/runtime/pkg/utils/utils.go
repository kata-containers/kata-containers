// Copyright (c) 2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

package utils

import (
	"net/http"
	"strings"
)

const (
	acceptEncodingHeader = "Accept-Encoding"
)

// GzipAccepted returns whether the client will accept gzip-encoded content.
func GzipAccepted(header http.Header) bool {
	a := header.Get(acceptEncodingHeader)
	parts := strings.Split(a, ",")
	for _, part := range parts {
		part = strings.TrimSpace(part)
		if part == "gzip" || strings.HasPrefix(part, "gzip;") {
			return true
		}
	}
	return false
}

// String2Pointer make a string to a pointer to string
func String2Pointer(s string) *string {
	return &s
}
