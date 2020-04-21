// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package tests

import (
	"math/rand"
	"time"
)

const letters = "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ"

const lettersMask = 63

// RandID returns a random string
func RandID(n int) string {
	randSrc := rand.NewSource(time.Now().UnixNano())
	b := make([]byte, n)
	for i := 0; i < n; {
		if j := int(randSrc.Int63() & lettersMask); j < len(letters) {
			b[i] = letters[j]
			i++
		}
	}

	return string(b)
}
