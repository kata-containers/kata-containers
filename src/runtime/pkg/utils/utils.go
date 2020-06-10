package utils

import (
	"fmt"
	"net/http"
	"os"
	"path/filepath"
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

// EnsureFileDir will check if file a in an absolute format and ensure the directory is exits
// if not, make the dir like `mkdir -p`
func EnsureFileDir(file string) error {
	if !filepath.IsAbs(file) {
		return fmt.Errorf("file must be an absolute path")
	}

	path := filepath.Dir(file)
	return os.MkdirAll(path, 0755)
}
