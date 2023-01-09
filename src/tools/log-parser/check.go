//
// Copyright (c) 2017-2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"
	"regexp"
	"unicode"
)

// checkValid determines if the specified string is valid or not.
// It looks for:
//
//   - Invalid (unprintable) characters.
//   - Standard golang error strings added by the formatting functions into the
//     resulting strings when issues are detected.
func checkValid(value string) error {
	if value == "" {
		return nil
	}

	for _, ch := range value {
		if !(unicode.IsPrint(ch) || unicode.IsSpace(ch)) {
			return fmt.Errorf("character %v (%x) in value %v not printable", ch, ch, value)
		}
	}

	// See: https://golang.org/pkg/fmt/
	invalidPatterns := []string{
		`%!\(BADINDEX\)`,
		`%!\(BADPREC\)`,
		`%!\(BADWIDTH\)`,
		`%!\(EXTRA\b`,
		`%!\w\(MISSING\)`,
	}

	for _, pattern := range invalidPatterns {
		re := regexp.MustCompile(pattern)
		foundMissing := re.FindStringSubmatch(value)

		if foundMissing != nil {
			return fmt.Errorf("invalid pattern %q in value %v "+
				"suggests log creator programming error",
				pattern, value)
		}
	}

	return nil
}
