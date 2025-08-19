// Package util common utilities or other elements shared across github.com/diskfs/go-diskfs packages
package util

import "io"

// File interface that can be read from and written to.
// Normally implemented as actual os.File, but useful as a separate interface so can easily
// use alternate implementations.
type File interface {
	io.ReaderAt
	io.WriterAt
	io.Seeker
}
