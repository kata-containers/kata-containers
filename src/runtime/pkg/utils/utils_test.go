// Copyright (c) 2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

package utils

import (
	"net/http"
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestGzipAccepted(t *testing.T) {
	assert := assert.New(t)
	testCases := []struct {
		header string
		result bool
	}{
		{
			header: "",
			result: false,
		},
		{
			header: "abc",
			result: false,
		},
		{
			header: "gzip",
			result: true,
		},
		{
			header: "deflate, gzip;q=1.0, *;q=0.5",
			result: true,
		},
	}

	h := http.Header{}

	for i := range testCases {
		tc := testCases[i]
		h[acceptEncodingHeader] = []string{tc.header}
		b := GzipAccepted(h)
		assert.Equal(tc.result, b)
	}
}
