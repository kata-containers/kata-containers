package iso9660

import (
	"fmt"
	"io"
	"os"
	"path"
	"path/filepath"
	"regexp"
	"sort"
	"strings"
	"time"

	"github.com/diskfs/go-diskfs/util"
)

const (
	dataStartSector         = 16
	defaultVolumeIdentifier = "ISOIMAGE"
)

// FinalizeOptions options to pass to finalize
type FinalizeOptions struct {
	// RockRidge enable Rock Ridge extensions
	RockRidge bool
	// DeepDirectories allow directories deeper than 8
	DeepDirectories bool
	// ElTorito slice of el torito entry configs
	ElTorito *ElTorito
	// VolumeIdentifier custom volume name, defaults to "ISOIMAGE"
	VolumeIdentifier string
}

// finalizeFileInfo is a file info useful for finalization
// fulfills os.FileInfo
//
//	Name() string       // base name of the file
//	Size() int64        // length in bytes for regular files; system-dependent for others
//	Mode() FileMode     // file mode bits
//	ModTime() time.Time // modification time
//	IsDir() bool        // abbreviation for Mode().IsDir()
//	Sys() interface{}   // underlying data source (can return nil)
//
//nolint:structcheck // keep unused members so that we can know their references
type finalizeFileInfo struct {
	path               string
	target             string
	shortname          string
	extension          string
	location           uint32
	blocks             uint32 // blocks for the directory itself and its entries
	continuationBlocks uint32 // blocks for CE entries
	recordSize         uint8
	depth              int
	name               string
	size               int64
	mode               os.FileMode
	modTime            time.Time
	isDir              bool
	isRoot             bool
	bytes              [][]byte
	parent             *finalizeFileInfo
	children           []*finalizeFileInfo
	trueParent         *finalizeFileInfo
	trueChild          *finalizeFileInfo
	elToritoEntry      *ElToritoEntry
	content            []byte
}

func (fi *finalizeFileInfo) Name() string {
	// we are using plain iso9660 (without extensions), so just shortname possibly with extension
	ret := fi.shortname
	if !fi.isDir {
		ret = fmt.Sprintf("%s.%s;1", fi.shortname, fi.extension)
	}
	// shortname already is ucased
	return ret
}
func (fi *finalizeFileInfo) Size() int64 {
	return fi.size
}
func (fi *finalizeFileInfo) Mode() os.FileMode {
	return fi.mode
}
func (fi *finalizeFileInfo) ModTime() time.Time {
	return fi.modTime
}
func (fi *finalizeFileInfo) IsDir() bool {
	return fi.isDir
}
func (fi *finalizeFileInfo) Sys() interface{} {
	return nil
}

func (fi *finalizeFileInfo) updateDepth(depth int) {
	fi.depth = depth
	if fi.isDir {
		for _, e := range fi.children {
			e.updateDepth(depth + 1)
		}
	}
}

func (fi *finalizeFileInfo) toDirectoryEntry(fs *FileSystem, isSelf, isParent bool) (*directoryEntry, error) {
	de := &directoryEntry{
		extAttrSize:              0,
		location:                 fi.location,
		size:                     uint32(fi.Size()),
		creation:                 fi.ModTime(),
		isHidden:                 false,
		isSubdirectory:           fi.IsDir(),
		isAssociated:             false,
		hasExtendedAttrs:         false,
		hasOwnerGroupPermissions: false,
		hasMoreEntries:           false,
		isSelf:                   isSelf,
		isParent:                 isParent,
		volumeSequence:           1,
		filesystem:               fs,
		// we keep the full filename until after processing
		filename: fi.Name(),
	}
	// if it is root, and we have susp enabled, add the necessary entries
	if fs.suspEnabled {
		if fi.isRoot && isSelf {
			de.extensions = append(de.extensions, directoryEntrySystemUseExtensionSharingProtocolIndicator{skipBytes: 0})
		}
		// add appropriate PX, TF, SL, NM extensions
		for _, e := range fs.suspExtensions {
			ext, err := e.GetFileExtensions(path.Join(fs.workspace, fi.path), isSelf, isParent)
			if err != nil {
				return nil, fmt.Errorf("error getting extensions for %s at path %s: %v", e.ID(), fi.path, err)
			}
			ext2, err := e.GetFinalizeExtensions(fi)
			if err != nil {
				return nil, fmt.Errorf("error getting finalize extensions for %s at path %s: %v", e.ID(), fi.path, err)
			}
			ext = append(ext, ext2...)
			de.extensions = append(de.extensions, ext...)
		}

		if fi.isRoot && isSelf {
			for _, e := range fs.suspExtensions {
				de.extensions = append(de.extensions, directoryEntrySystemUseExtensionReference{id: e.ID(), descriptor: e.Descriptor(), source: e.Source(), extensionVersion: e.Version()})
			}
		}
	}
	return de, nil
}
func (fi *finalizeFileInfo) toDirectory(fs *FileSystem) (*Directory, error) {
	// also need to add self and parent to it
	var (
		self, parent, dirEntry *directoryEntry
		err                    error
	)
	if !fi.IsDir() {
		return nil, fmt.Errorf("cannot convert a file entry to a directtory")
	}
	self, err = fi.toDirectoryEntry(fs, true, false)
	if err != nil {
		return nil, fmt.Errorf("could not convert self entry %s to dirEntry: %v", fi.path, err)
	}

	// if we have no parent, we are the root entry
	// we also need to put in the SUSP if it is enabled
	parentEntry := fi.parent
	if fi.isRoot {
		parentEntry = fi
	}
	parent, err = parentEntry.toDirectoryEntry(fs, false, true)
	if err != nil {
		return nil, fmt.Errorf("could not convert parent entry %s to dirEntry: %v", fi.parent.path, err)
	}

	entries := []*directoryEntry{self, parent}
	for _, child := range fi.children {
		dirEntry, err = child.toDirectoryEntry(fs, false, false)
		if err != nil {
			return nil, fmt.Errorf("could not convert child entry %s to dirEntry: %v", child.path, err)
		}
		entries = append(entries, dirEntry)
	}
	d := &Directory{
		directoryEntry: *self,
		entries:        entries,
	}
	return d, nil
}

// calculate the size of a directory entry single record
func (fi *finalizeFileInfo) calculateRecordSize(fs *FileSystem, isSelf, isParent bool) (dirEntrySize, continuationBlocksSize int, err error) {
	// we do not actually need the the continuation blocks to calculate size, just length, so use an empty slice
	extTmpBlocks := make([]uint32, 100)
	dirEntry, err := fi.toDirectoryEntry(fs, isSelf, isParent)
	if err != nil {
		return 0, 0, fmt.Errorf("could not convert to dirEntry: %v", err)
	}
	dirBytes, err := dirEntry.toBytes(false, extTmpBlocks)
	if err != nil {
		return 0, 0, fmt.Errorf("could not convert dirEntry to bytes: %v", err)
	}
	// first entry is the bytes to store in the directory
	// rest are continuation blocks
	return len(dirBytes[0]), len(dirBytes) - 1, nil
}

// calculate the size of a directory, similar to a file size
func (fi *finalizeFileInfo) calculateDirectorySize(fs *FileSystem) (dirEntrySize, continuationBlocksSize int, err error) {
	var (
		recSize, recCE int
	)
	if !fi.IsDir() {
		return 0, 0, fmt.Errorf("cannot convert a file entry to a directtory")
	}
	recSize, recCE, err = fi.calculateRecordSize(fs, true, false)
	if err != nil {
		return 0, 0, fmt.Errorf("could not calculate self entry size %s: %v", fi.path, err)
	}
	dirEntrySize += recSize
	continuationBlocksSize += recCE

	recSize, recCE, err = fi.calculateRecordSize(fs, false, true)
	if err != nil {
		return 0, 0, fmt.Errorf("could not calculate parent entry size %s: %v", fi.path, err)
	}
	dirEntrySize += recSize
	continuationBlocksSize += recCE

	for _, e := range fi.children {
		// get size of data and CE blocks
		recSize, recCE, err = e.calculateRecordSize(fs, false, false)
		if err != nil {
			return 0, 0, fmt.Errorf("could not calculate child %s entry size %s: %v", e.path, fi.path, err)
		}
		// do not go over a block boundary; pad if necessary
		newSize := dirEntrySize + recSize
		blocksize := int(fs.blocksize)
		left := blocksize - dirEntrySize%blocksize
		if left != 0 && newSize/blocksize > dirEntrySize/blocksize {
			dirEntrySize += left
		}
		continuationBlocksSize += recCE
		dirEntrySize += recSize
	}
	return dirEntrySize, continuationBlocksSize, nil
}

// add depth to all children
func (fi *finalizeFileInfo) addProperties(depth int) {
	fi.depth = depth
	for _, e := range fi.children {
		e.parent = fi
		e.addProperties(depth + 1)
	}
}

// sort all of the directory children recursively - this is for ordering into blocks
func (fi *finalizeFileInfo) collapseAndSortChildren() (dirs, files []*finalizeFileInfo) {
	tmpDirs := make([]*finalizeFileInfo, 0)
	tmpFiles := make([]*finalizeFileInfo, 0)
	// first extract all of the directories
	for _, e := range fi.children {
		if e.IsDir() {
			tmpDirs = append(tmpDirs, e)
		} else {
			tmpFiles = append(tmpFiles, e)
		}
	}

	// next sort them
	sort.Slice(tmpDirs, func(i, j int) bool {
		// just sort by filename; as good as anything else
		return tmpDirs[i].Name() < tmpDirs[j].Name()
	})
	sort.Slice(tmpFiles, func(i, j int) bool {
		// just sort by filename; as good as anything else
		return tmpFiles[i].Name() < tmpFiles[j].Name()
	})
	// finally add in the children going down
	dirs = make([]*finalizeFileInfo, 0)
	files = tmpFiles
	for _, e := range tmpDirs {
		dirs = append(dirs, e)
		// now get any children
		d, f := e.collapseAndSortChildren()
		dirs = append(dirs, d...)
		files = append(files, f...)
	}
	return dirs, files
}

func (fi *finalizeFileInfo) findEntry(p string) (*finalizeFileInfo, error) {
	// break path down into parts and levels
	var (
		target *finalizeFileInfo
		err    error
	)
	parts := splitPath(p)
	if len(parts) == 0 || p == "." {
		target = fi
	} else {
		current := parts[0]
		// read the directory bytes
		for _, e := range fi.children {
			// do we have an alternate name?
			// only care if not self or parent entry
			checkFilename := e.name
			if checkFilename == current {
				if len(parts) > 1 {
					target, err = e.findEntry(path.Join(parts[1:]...))
					if err != nil {
						return nil, fmt.Errorf("could not get entry: %v", err)
					}
				} else {
					// this is the final one, we found it, keep it
					target = e
				}
				break
			}
		}
	}
	return target, nil
}
func (fi *finalizeFileInfo) removeChild(p string) *finalizeFileInfo {
	var removed *finalizeFileInfo
	children := make([]*finalizeFileInfo, 0)
	for _, e := range fi.children {
		if e.name != p {
			children = append(children, e)
		} else {
			removed = e
		}
	}
	fi.children = children
	return removed
}
func (fi *finalizeFileInfo) addChild(entry *finalizeFileInfo) {
	fi.children = append(fi.children, entry)
}

// Finalize finalize a read-only filesystem by writing it out to a read-only format
//
//nolint:gocyclo // this finalize function is complex and needs to be. We might be better off refactoring it to multiple functions, but it does not buy all that much.
func (fs *FileSystem) Finalize(options FinalizeOptions) error {
	if fs.workspace == "" {
		return fmt.Errorf("cannot finalize an already finalized filesystem")
	}

	// did we ask for susp?
	if options.RockRidge {
		fs.suspEnabled = true
		fs.suspExtensions = append(fs.suspExtensions, getRockRidgeExtension(rockRidge112))
	}

	/*
		There is nothing in the iso9660 spec about the order of directories and files,
		other than that they must be accessible in the location specified in directory entry and/or path table
		However, most implementations seem to it as follows:
		- each directory follows its parent
		- data (i.e. file) sectors in each directory are immediately after its directory and immediately before the next sibling directory to its parent

		to keep it simple, we will follow what xorriso/mkisofs on linux does, in the following order:
		- volume descriptor set, beginning at sector 16
		- root directory entry
		- all other directory entries, sorted alphabetically, depth first
		- L path table
		- M path table
		- data sectors for files, sorted alphabetically, matching order of directories

		this is where we build our filesystem
		 1- blank out sectors 0-15 for system use
		 2- skip sectors 16-17 for PVD and terminator (fill later)
		 3- calculate how many sectors required for root directory
		 4- calculate each child directory, working our way down, including number of sectors and location
		 5- write path tables (L & M)
		 6- write files for root directory
		 7- write root directory entry into its sector (18)
		 8- repeat steps 6&7 for all other directories
		 9- write PVD
		 10- write volume descriptor set terminator
	*/

	f := fs.file
	blocksize := int(fs.blocksize)

	// 1- blank out sectors 0-15
	b := make([]byte, dataStartSector*fs.blocksize)
	n, err := f.WriteAt(b, 0)
	if err != nil {
		return fmt.Errorf("could not write blank system area: %v", err)
	}
	if n != len(b) {
		return fmt.Errorf("only wrote %d bytes instead of expected %d to system area", n, len(b))
	}

	// 3- build out file tree
	fileList, dirList, err := walkTree(fs.Workspace())
	if err != nil {
		return fmt.Errorf("error walking tree: %v", err)
	}

	// starting point
	root := dirList["."]
	root.addProperties(1)

	// if we need to relocate directories, must do them here, before finalizing order and sizes
	// do not bother if enabled DeepDirectories, i.e. non-ISO9660 compliant
	if !options.DeepDirectories {
		if fs.suspEnabled {
			var handler suspExtension
			for _, e := range fs.suspExtensions {
				if e.Relocatable() {
					handler = e
					break
				}
			}
			var relocateFiles []*finalizeFileInfo
			relocateFiles, dirList, err = handler.Relocate(dirList)
			if err != nil {
				return fmt.Errorf("unable to use extension %s to relocate directories from depth > 8: %v", handler.ID(), err)
			}
			fileList = append(fileList, relocateFiles...)
		}
		// check if there are any deeper than 9
		for _, e := range dirList {
			if e.depth > 8 {
				return fmt.Errorf("directory %s deeper than 8 deep and DeepDirectories override not enabled", e.path)
			}
		}
	}

	// convert sizes to required blocks for files
	for _, e := range fileList {
		e.blocks = calculateBlocks(e.size, fs.blocksize)
	}

	// we now have list of all of the files and directories and their properties, as well as children of every directory
	// store them in a flat sorted slice, beginning with root so we can write them out in order to blocks after
	dirs := make([]*finalizeFileInfo, 0, 20)
	dirs = append(dirs, root)
	subdirs, files := root.collapseAndSortChildren()
	dirs = append(dirs, subdirs...)

	// calculate the sizes and locations of the directories from the flat list and assign blocks
	rootLocation := uint32(dataStartSector + 2)
	// if el torito was enabled, use one sector for boot volume entry
	if options.ElTorito != nil {
		rootLocation++
	}
	location := rootLocation

	var (
		catEntry *finalizeFileInfo
		bootcat  []byte
	)

	if options.ElTorito != nil {
		bootcat = options.ElTorito.generateCatalog()
		// figure out where to save it on disk
		catname := options.ElTorito.BootCatalog
		switch {
		case catname == "" && options.RockRidge:
			catname = elToritoDefaultCatalogRR
		case catname == "":
			catname = elToritoDefaultCatalog
		}
		shortname, extension := calculateShortnameExtension(path.Base(catname))
		// break down the catalog basename from the parent dir
		catSize := int64(len(bootcat))
		catEntry = &finalizeFileInfo{
			content:   bootcat,
			size:      catSize,
			path:      catname,
			name:      path.Base(catname),
			shortname: shortname,
			extension: extension,
			blocks:    calculateBlocks(catSize, fs.blocksize),
		}
		// make it the first file
		files = append([]*finalizeFileInfo{catEntry}, files...)

		// if we were not told to hide the catalog, add it to its parent
		if !options.ElTorito.HideBootCatalog {
			var parent *finalizeFileInfo
			parent, err = root.findEntry(path.Dir(catname))
			if err != nil {
				return fmt.Errorf("error finding parent for boot catalog %s: %v", catname, err)
			}
			parent.addChild(catEntry)
		}
		for _, e := range options.ElTorito.Entries {
			var parent, child *finalizeFileInfo
			parent, err = root.findEntry(path.Dir(e.BootFile))
			if err != nil {
				return fmt.Errorf("error finding parent for boot image file %s: %v", e.BootFile, err)
			}
			// did we ask to hide any image files?
			if e.HideBootFile {
				child = parent.removeChild(path.Base(e.BootFile))
			} else {
				child, err = parent.findEntry(path.Base(e.BootFile))
				if err != nil {
					return fmt.Errorf("unable to find image child %s: %v", e.BootFile, err)
				}
			}
			// save the child so we can add location late
			e.size = uint16(child.size)
			child.elToritoEntry = e
		}
	}

	var size, ceBlocks int
	for _, dir := range dirs {
		dir.location = location
		size, ceBlocks, err = dir.calculateDirectorySize(fs)
		if err != nil {
			return fmt.Errorf("unable to calculate size of directory for %s: %v", dir.path, err)
		}
		dir.size = int64(size)
		dir.blocks = calculateBlocks(int64(size), int64(blocksize))
		dir.continuationBlocks = uint32(ceBlocks)
		location += dir.blocks + dir.continuationBlocks
	}

	// we now have sorted list of block order, with sizes and number of blocks on each
	// next assign the blocks to each, and then we can enter the data in the directory entries

	// create the pathtables (L & M)
	// with the list of directories, we can make a path table
	pathTable := createPathTable(dirs)
	// how big is the path table? we will take LSB for now, because they are the same size
	pathTableLBytes := pathTable.toLBytes()
	pathTableMBytes := pathTable.toMBytes()
	pathTableSize := len(pathTableLBytes)
	pathTableBlocks := uint32(pathTableSize / blocksize)
	if pathTableSize%blocksize > 0 {
		pathTableBlocks++
	}
	// we do not do optional path tables yet
	pathTableLLocation := location
	location += pathTableBlocks
	pathTableMLocation := location
	location += pathTableBlocks

	// if we asked for ElTorito, need to generate the boot catalog and save it
	volIdentifier := defaultVolumeIdentifier
	if options.VolumeIdentifier != "" {
		volIdentifier = options.VolumeIdentifier
	}

	for _, e := range files {
		e.location = location
		location += e.blocks
		if e.elToritoEntry != nil {
			e.elToritoEntry.location = e.location
		}
	}

	// now that we have all of the files with their locations, we can rebuild the boot catalog using the correct data
	if catEntry != nil {
		bootcat = options.ElTorito.generateCatalog()
		catEntry.content = bootcat
	}

	// now we can write each one out - dirs first then files
	for _, e := range dirs {
		writeAt := int64(e.location) * int64(blocksize)
		var d *Directory
		d, err = e.toDirectory(fs)
		if err != nil {
			return fmt.Errorf("unable to convert entry to directory: %v", err)
		}
		// Directory.toBytes() always returns whole blocks
		// get the continuation entry locations
		ceLocations := make([]uint32, 0)
		ceLocationStart := e.location + e.blocks
		for i := 0; i < int(e.continuationBlocks); i++ {
			ceLocations = append(ceLocations, ceLocationStart+uint32(i))
		}
		var p [][]byte
		p, err = d.entriesToBytes(ceLocations)
		if err != nil {
			return fmt.Errorf("could not convert directory to bytes: %v", err)
		}
		for i, e := range p {
			_, _ = f.WriteAt(e, writeAt+int64(i*blocksize))
		}
	}

	// now write out the path tables, L & M
	writeAt := int64(pathTableLLocation) * int64(blocksize)
	_, _ = f.WriteAt(pathTableLBytes, writeAt)
	writeAt = int64(pathTableMLocation) * int64(blocksize)
	_, _ = f.WriteAt(pathTableMBytes, writeAt)

	var closeFiles []*os.File
	defer func() {
		for _, f := range closeFiles {
			f.Close()
		}
	}()
	for _, e := range files {
		var (
			from   *os.File
			copied int
		)
		writeAt := int64(e.location) * int64(blocksize)
		if e.content == nil {
			// for file, just copy the data across
			from, err = os.Open(path.Join(fs.workspace, e.path))
			if err != nil {
				return fmt.Errorf("failed to open file for reading %s: %v", e.path, err)
			}
			closeFiles = append(closeFiles, from)
			if e.elToritoEntry != nil && e.elToritoEntry.BootTable {
				// copy first 8 bytes, then insert the El Torito Boot Information Table, then the rest
				var count int

				// first 8 bytes
				count, err = copyFileData(from, f, 0, writeAt, 8)
				if err != nil {
					return fmt.Errorf("failed to copy first bytes 0-8 of boot file to disk %s: %v", e.path, err)
				}
				copied += count
				// insert El Torito Boot Information Table
				bootTable, err := e.elToritoEntry.generateBootTable(dataStartSector, path.Join(fs.workspace, e.path))
				if err != nil {
					return fmt.Errorf("failed to generate boot table for %s: %v", e.path, err)
				}
				count, err = f.WriteAt(bootTable, writeAt+8)
				if err != nil {
					return fmt.Errorf("failed to write 56 byte boot table to disk %s: %v", e.path, err)
				}
				copied += count
				// remainder of file
				count, err = copyFileData(from, f, 64, writeAt+64, 0)
				if err != nil {
					return fmt.Errorf("failed to copy bytes 64 to end of boot file to disk %s: %v", e.path, err)
				}
				copied += count
			} else {
				copied, err = copyFileData(from, f, 0, writeAt, 0)
				if err != nil {
					return fmt.Errorf("failed to copy file to disk %s: %v", e.path, err)
				}
			}
			if copied != int(e.Size()) {
				return fmt.Errorf("error copying file %s to disk, copied %d bytes, expected %d", e.path, copied, e.Size())
			}
		} else {
			copied = len(e.content)
			if _, err = f.WriteAt(e.content, writeAt); err != nil {
				return fmt.Errorf("failed to write content of %s to disk: %v", e.path, err)
			}
		}
		// fill in
		left := blocksize - (copied % blocksize)
		if left > 0 {
			b2 := make([]byte, left)
			_, _ = f.WriteAt(b2, writeAt+int64(copied))
		}
	}

	totalSize := location
	location = dataStartSector
	// create and write the primary volume descriptor, supplementary and boot, and volume descriptor set terminator
	now := time.Now()
	rootDE, err := root.toDirectoryEntry(fs, true, false)
	if err != nil {
		return fmt.Errorf("could not convert root entry for primary volume descriptor to dirEntry: %v", err)
	}

	pvd := &primaryVolumeDescriptor{
		systemIdentifier:           "",
		volumeIdentifier:           volIdentifier,
		volumeSize:                 totalSize,
		setSize:                    1,
		sequenceNumber:             1,
		blocksize:                  uint16(fs.blocksize),
		pathTableSize:              uint32(pathTableSize),
		pathTableLLocation:         pathTableLLocation,
		pathTableLOptionalLocation: 0,
		pathTableMLocation:         pathTableMLocation,
		pathTableMOptionalLocation: 0,
		volumeSetIdentifier:        "",
		publisherIdentifier:        "",
		preparerIdentifier:         util.AppNameVersion,
		applicationIdentifier:      "",
		copyrightFile:              "", // 37 bytes
		abstractFile:               "", // 37 bytes
		bibliographicFile:          "", // 37 bytes
		creation:                   now,
		modification:               now,
		expiration:                 now,
		effective:                  now,
		rootDirectoryEntry:         rootDE,
	}
	b = pvd.toBytes()
	_, _ = f.WriteAt(b, int64(location)*int64(blocksize))
	location++

	// do we have a boot sector?
	if options.ElTorito != nil {
		bvd := &bootVolumeDescriptor{location: catEntry.location}
		b = bvd.toBytes()
		_, _ = f.WriteAt(b, int64(location)*int64(blocksize))
		location++
	}
	terminator := &terminatorVolumeDescriptor{}
	b = terminator.toBytes()
	_, _ = f.WriteAt(b, int64(location)*int64(blocksize))

	_ = os.RemoveAll(fs.workspace)

	// finish by setting as finalized
	fs.workspace = ""
	return nil
}

// copyFileData copy data from file `from` at offset `fromOffset` to file `to` at offset `toOffset`.
// Copies `size` bytes. If `size` is 0, copies as many bytes as it can.
func copyFileData(from, to util.File, fromOffset, toOffset int64, size int) (int, error) {
	buf := make([]byte, 2048)
	copied := 0
	for {
		n, err := from.ReadAt(buf, fromOffset+int64(copied))
		if err != nil && err != io.EOF {
			return copied, err
		}

		if size > 0 && n > (size-copied) {
			n = size - copied
		}

		if n == 0 {
			break
		}

		if _, err := to.WriteAt(buf[:n], toOffset+int64(copied)); err != nil {
			return copied, err
		}
		copied += n
	}
	return copied, nil
}

// sort path table entries
func sortFinalizeFileInfoPathTable(left, right *finalizeFileInfo) bool {
	switch {
	case left.parent == right.parent:
		// same parents = same depth, just sort on name
		lname := left.Name()
		rname := right.Name()
		maxLen := maxInt(len(lname), len(rname))
		format := fmt.Sprintf("%%-%ds", maxLen)
		return fmt.Sprintf(format, lname) < fmt.Sprintf(format, rname)
	case left.depth < right.depth:
		// different parents with different depth, lower first
		return true
	case right.depth > left.depth:
		return false
	case left.parent == nil && right.parent != nil:
		return true
	case left.parent != nil && right.parent == nil:
		return false
	default:
		// same depth, different parents, it depends on the sort order of the parents
		return sortFinalizeFileInfoPathTable(left.parent, right.parent)
	}
}

// create a path table from a slice of *finalizeFileInfo that are directories
func createPathTable(fi []*finalizeFileInfo) *pathTable {
	// copy so we do not modify the original
	fs := make([]*finalizeFileInfo, len(fi))
	copy(fs, fi)
	// sort via the rules
	sort.Slice(fs, func(i, j int) bool {
		return sortFinalizeFileInfoPathTable(fs[i], fs[j])
	})
	indexMap := make(map[*finalizeFileInfo]int)
	// now that it is sorted, create the ordered path table entries
	entries := make([]*pathTableEntry, 0)
	for i, e := range fs {
		name := e.Name()
		nameSize := len(name)
		size := 8 + uint16(nameSize)
		if nameSize%2 != 0 {
			size++
		}
		ownIndex := i + 1
		indexMap[e] = ownIndex
		// root just points to itself
		parentIndex := ownIndex
		if ip, ok := indexMap[e.parent]; ok {
			parentIndex = ip
		}
		pte := &pathTableEntry{
			nameSize:      uint8(nameSize),
			size:          size,
			extAttrLength: 0,
			location:      e.location,
			parentIndex:   uint16(parentIndex),
			dirname:       name,
		}
		entries = append(entries, pte)
	}
	return &pathTable{
		records: entries,
	}
}

func walkTree(workspace string) ([]*finalizeFileInfo, map[string]*finalizeFileInfo, error) {
	cwd, err := os.Getwd()
	if err != nil {
		return nil, nil, fmt.Errorf("could not get pwd: %v", err)
	}
	// make everything relative to the workspace
	_ = os.Chdir(workspace)
	dirList := make(map[string]*finalizeFileInfo)
	fileList := make([]*finalizeFileInfo, 0)
	var entry *finalizeFileInfo
	err = filepath.Walk(".", func(fp string, fi os.FileInfo, err error) error {
		if err != nil {
			return fmt.Errorf("error walking path %s: %v", fp, err)
		}
		isRoot := fp == "."
		name := fi.Name()
		shortname, extension := calculateShortnameExtension(name)

		if isRoot {
			name = string([]byte{0x00})
			shortname = name
		}
		entry = &finalizeFileInfo{path: fp, name: name, isDir: fi.IsDir(), isRoot: isRoot, modTime: fi.ModTime(), mode: fi.Mode(), size: fi.Size(), shortname: shortname}

		// we will have to save it as its parent
		parentDir := filepath.Dir(fp)
		parentDirInfo := dirList[parentDir]

		if fi.IsDir() {
			entry.children = make([]*finalizeFileInfo, 0, 20)
			dirList[fp] = entry
			if !isRoot {
				parentDirInfo.children = append(parentDirInfo.children, entry)
				dirList[parentDir] = parentDirInfo
			}
		} else {
			// calculate blocks
			entry.size = fi.Size()
			entry.extension = extension
			parentDirInfo.children = append(parentDirInfo.children, entry)
			dirList[parentDir] = parentDirInfo
			fileList = append(fileList, entry)
		}
		return nil
	})
	if err != nil {
		return nil, nil, err
	}
	// reset the workspace
	_ = os.Chdir(cwd)
	return fileList, dirList, nil
}

func calculateBlocks(size, blocksize int64) uint32 {
	blocks := uint32(size / blocksize)
	// add one for partial
	if size%blocksize > 0 {
		blocks++
	}
	return blocks
}

func calculateShortnameExtension(name string) (shortname, extension string) {
	parts := strings.SplitN(name, ".", 2)
	shortname = parts[0]
	if len(parts) > 1 {
		extension = parts[1]
	}
	// shortname and extension must be upper-case
	shortname = strings.ToUpper(shortname)
	extension = strings.ToUpper(extension)

	// replace illegal characters in shortname and extension with _
	re := regexp.MustCompile("[^A-Z0-9_]")
	shortname = re.ReplaceAllString(shortname, "_")
	extension = re.ReplaceAllString(extension, "_")

	return shortname, extension
}
