//
// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import "os"

func createEmptyFile(path string) (err error) {
	return os.WriteFile(path, []byte(""), testFileMode)
}

func createFile(file, contents string) error {
	return os.WriteFile(file, []byte(contents), testFileMode)
}
