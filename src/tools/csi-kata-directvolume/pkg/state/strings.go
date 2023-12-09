//
// Copyright 2017 The Kubernetes Authors.
// Copyright (c) 2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

package state

// Strings is an ordered set of strings with helper functions for
// adding, searching and removing entries.
type Strings []string

// Add appends at the end.
func (s *Strings) Add(str string) {
	*s = append(*s, str)
}

// Has checks whether the string is already present.
func (s *Strings) Has(str string) bool {
	for _, str2 := range *s {
		if str == str2 {
			return true
		}
	}
	return false
}

// Empty returns true if the list is empty.
func (s *Strings) Empty() bool {
	return len(*s) == 0
}

// Remove removes the first matched target of the string, if present.
func (s *Strings) Remove(str string) {
	for i, str2 := range *s {
		if str == str2 {
			*s = append((*s)[:i], (*s)[i+1:]...)
			return
		}
	}
}
