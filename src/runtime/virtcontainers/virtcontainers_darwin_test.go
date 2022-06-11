// Copyright (c) 2023 Apple Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import "os"

// cleanUp removes any stale sandbox/container state that can affect
// the next test to run.
func cleanUp() {
	os.RemoveAll(testDir)
	os.MkdirAll(testDir, DirMode)
	setup()
}
