//
// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"flag"
	"os"
	"path/filepath"
	"testing"

	"github.com/stretchr/testify/require"
	"github.com/urfave/cli"
)

func createEmptyFile(path string) (err error) {
	return os.WriteFile(path, []byte(""), testFileMode)
}

func createFile(file, contents string) error {
	return os.WriteFile(file, []byte(contents), testFileMode)
}

func TestHandleLogFilesTruncatesOutput(t *testing.T) {
	originalOutputFile := outputFile
	t.Cleanup(func() { outputFile = originalOutputFile })

	for _, format := range NewDisplayHandlers().Get() {
		t.Run(format, func(t *testing.T) {
			dir := t.TempDir()
			input := filepath.Join(dir, "input.log")
			require.NoError(t, createFile(input, "level=info pid=1234 source=runtime name=kata msg=hello time=2018-02-24T12:36:36.115882442Z\n"))

			convert := func(output string) {
				flags := flag.NewFlagSet("test", flag.ContinueOnError)
				flags.String("output-file", output, "")
				flags.String("output-format", format, "")
				require.NoError(t, flags.Parse([]string{input}))
				require.NoError(t, handleLogFiles(cli.NewContext(cli.NewApp(), flags, nil)))
			}

			fresh := filepath.Join(dir, "fresh")
			convert(fresh)
			expected, err := os.ReadFile(fresh)
			require.NoError(t, err)
			require.NotEmpty(t, expected)

			existing := filepath.Join(dir, "existing")
			require.NoError(t, createFile(existing, string(expected)+"stale output\n"))
			convert(existing)
			actual, err := os.ReadFile(existing)
			require.NoError(t, err)
			require.Equal(t, string(expected), string(actual))
		})
	}
}
