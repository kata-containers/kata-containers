package filesystem

import (
	"io/fs"
	"os"
	"path"
	"time"
)

type fsCompatible struct {
	fs FileSystem
}

type fsFileWrapper struct {
	File
	stat os.FileInfo
}

type fakeRootDir struct{}

func (d *fakeRootDir) Name() string       { return "/" }
func (d *fakeRootDir) Size() int64        { return 0 }
func (d *fakeRootDir) Mode() fs.FileMode  { return 0 }
func (d *fakeRootDir) ModTime() time.Time { return time.Now() }
func (d *fakeRootDir) IsDir() bool        { return true }
func (d *fakeRootDir) Sys() any           { return nil }

type fsDirWrapper struct {
	name   string
	compat *fsCompatible
	stat   os.FileInfo
}

func (f *fsDirWrapper) Close() error {
	return nil
}

func (f *fsDirWrapper) Read([]byte) (int, error) {
	return 0, fs.ErrInvalid
}

func (f *fsDirWrapper) ReadDir(n int) ([]fs.DirEntry, error) {
	entries, err := f.compat.ReadDir(f.name)
	if err != nil {
		return nil, err
	}
	if n < 0 || n >= len(entries) {
		n = len(entries)
	}
	return entries[:n], nil
}

func (f *fsDirWrapper) Stat() (fs.FileInfo, error) {
	return f.stat, nil
}

func (f *fsFileWrapper) Stat() (fs.FileInfo, error) {
	return f.stat, nil
}

// Converts the relative path name to an absolute one
func absoluteName(name string) string {
	if name == "." {
		name = "/"
	}
	if name[0] != '/' {
		name = "/" + name
	}
	return name
}

func (f *fsCompatible) Open(name string) (fs.File, error) {
	var stat os.FileInfo
	name = absoluteName(name)
	if name == "/" {
		return &fsDirWrapper{name: name, compat: f, stat: &fakeRootDir{}}, nil
	}
	dirname := path.Dir(name)
	if info, err := f.fs.ReadDir(dirname); err == nil {
		for i := range info {
			if info[i].Name() == path.Base(name) {
				stat = info[i]
				break
			}
		}
	}
	if stat == nil {
		return nil, fs.ErrNotExist
	}
	if stat.IsDir() {
		return &fsDirWrapper{name: name, compat: f, stat: stat}, nil
	}
	file, err := f.fs.OpenFile(name, os.O_RDONLY)
	if err != nil {
		return nil, err
	}
	return &fsFileWrapper{File: file, stat: stat}, nil
}

func (f *fsCompatible) ReadDir(name string) ([]fs.DirEntry, error) {
	entries, err := f.fs.ReadDir(name)
	if err != nil {
		return nil, err
	}
	direntries := make([]fs.DirEntry, len(entries))
	for i := range entries {
		direntries[i] = fs.FileInfoToDirEntry(entries[i])
	}
	return direntries, nil
}

// FS converts a diskfs FileSystem to a fs.FS for compatibility with
// other utilities
func FS(f FileSystem) fs.ReadDirFS {
	return &fsCompatible{f}
}
