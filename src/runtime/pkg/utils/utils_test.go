package utils

import (
	"net/http"
	"os"
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

func TestEnsureFileDir(t *testing.T) {
	assert := assert.New(t)
	testCases := []struct {
		file string
		path string
		err  bool
	}{
		{
			file: "abc.txt",
			path: "",
			err:  true,
		},
		{
			file: "/tmp/kata-test/abc/def/igh.txt",
			path: "/tmp/kata-test/abc/def",
			err:  false,
		},
		{
			file: "/tmp/kata-test/abc/../def/igh.txt",
			path: "/tmp/kata-test/def",
			err:  false,
		},
	}

	for i := range testCases {
		tc := testCases[i]
		err := EnsureFileDir(tc.file)
		// assert error
		assert.Equal(tc.err, err != nil)

		if !tc.err {
			// assert directory created
			fileInfo, err := os.Stat(tc.path)
			assert.Equal(nil, err)
			assert.Equal(true, fileInfo.IsDir())
		}
	}

	// clear test directory
	os.RemoveAll("/tmp/kata-test")
}
