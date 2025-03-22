package fat32

import (
	"errors"
	"fmt"
	"os"
	"path"
	"strings"
	"time"

	"github.com/diskfs/go-diskfs/backend"
	"github.com/diskfs/go-diskfs/filesystem"
)

// MsdosMediaType is the (mostly unused) media type. However, we provide and export the known constants for it.
type MsdosMediaType uint8

const (
	// Media8InchDrDos for single-sided 250KB DR-DOS disks
	Media8InchDrDos MsdosMediaType = 0xe5
	// Media525InchTandy for 5.25 inch floppy disks for Tandy
	Media525InchTandy MsdosMediaType = 0xed
	// MediaCustomPartitionsDrDos for non-standard custom DR-DOS partitions utilizing non-standard BPB formats
	MediaCustomPartitionsDrDos MsdosMediaType = 0xee
	// MediaCustomSuperFloppyDrDos for non-standard custom superfloppy disks for DR-DOS
	MediaCustomSuperFloppyDrDos MsdosMediaType = 0xef
	// Media35Inch for standard 1.44MB and 2.88MB 3.5 inch floppy disks
	Media35Inch MsdosMediaType = 0xf0
	// MediaDoubleDensityAltos for double-density floppy disks for Altos only
	MediaDoubleDensityAltos MsdosMediaType = 0xf4
	// MediaFixedDiskAltos for fixed disk 1.95MB for Altos only
	MediaFixedDiskAltos MsdosMediaType = 0xf5
	// MediaFixedDisk for standard fixed disks - can be used for any partitioned fixed or removable media where the geometry is defined in the BPB
	MediaFixedDisk MsdosMediaType = 0xf8
)

// SectorSize indicates what the sector size in bytes is
type SectorSize uint16

const (
	// SectorSize512 is a sector size of 512 bytes, used as the logical size for all FAT filesystems
	SectorSize512        SectorSize = 512
	bytesPerSlot         int        = 32
	maxCharsLongFilename int        = 13
)

//nolint:deadcode,varcheck,unused // we need these references in the future
const (
	minClusterSize int = 128
	maxClusterSize int = 65529
)

// FileSystem implememnts the FileSystem interface
type FileSystem struct {
	bootSector      msDosBootSector
	fsis            FSInformationSector
	table           table
	dataStart       uint32
	bytesPerCluster int
	size            int64
	start           int64
	backend         backend.Storage
}

// Equal compare if two filesystems are equal
func (fs *FileSystem) Equal(a *FileSystem) bool {
	if fs == nil && a == nil {
		return true
	}
	if fs == nil || a == nil {
		return false
	}
	localMatch := fs.backend == a.backend && fs.dataStart == a.dataStart && fs.bytesPerCluster == a.bytesPerCluster
	tableMatch := fs.table.equal(&a.table)
	bsMatch := fs.bootSector.equal(&a.bootSector)
	fsisMatch := fs.fsis == a.fsis
	return localMatch && tableMatch && bsMatch && fsisMatch
}

// Create creates a FAT32 filesystem in a given file or device
//
// requires the backend.Storage where to create the filesystem, size is the size of the filesystem in bytes,
// start is how far in bytes from the beginning of the backend.Storage to create the filesystem,
// and blocksize is is the logical blocksize to use for creating the filesystem
//
// note that you are *not* required to create the filesystem on the entire disk. You could have a disk of size
// 20GB, and create a small filesystem of size 50MB that begins 2GB into the disk.
// This is extremely useful for creating filesystems on disk partitions.
//
// Note, however, that it is much easier to do this using the higher-level APIs at github.com/diskfs/go-diskfs
// which allow you to work directly with partitions, rather than having to calculate (and hopefully not make any errors)
// where a partition starts and ends.
//
// If the provided blocksize is 0, it will use the default of 512 bytes. If it is any number other than 0
// or 512, it will return an error.
func Create(b backend.Storage, size, start, blocksize int64, volumeLabel string) (*FileSystem, error) {
	// blocksize must be <=0 or exactly SectorSize512 or error
	if blocksize != int64(SectorSize512) && blocksize > 0 {
		return nil, fmt.Errorf("blocksize for FAT32 must be either 512 bytes or 0, not %d", blocksize)
	}
	if size > Fat32MaxSize {
		return nil, fmt.Errorf("requested size is larger than maximum allowed FAT32, requested %d, maximum %d", size, Fat32MaxSize)
	}
	if size < blocksize*4 {
		return nil, fmt.Errorf("requested size is smaller than minimum allowed FAT32, requested %d minimum %d", size, blocksize*4)
	}
	// FAT filesystems use time-of-day of creation as a volume ID
	now := time.Now()
	// because we like the fudges other people did for uniqueness
	volid := uint32(now.Unix()<<20 | (now.UnixNano() / 1000000))

	fsisPrimarySector := uint16(1)
	backupBootSector := uint16(6)

	writableFile, err := b.Writable()
	if err != nil {
		return nil, err
	}

	/*
		size calculations
		we have the total size of the disk from `size uint64`
		we have the blocksize fixed at SectorSize512
		    so we can calculate diskSectors = size/512
		we know the number of reserved sectors is 32
		so the number of non-reserved sectors: data + FAT = diskSectos - 32
		now we need to figure out cluster size. The allowed number of:
		    sectors per cluster: 1, 2, 4, 8, 16, 32, 64, 128
		    bytes per cluster: 512, 1024, 2048, 4096, 8192, 16384, 32768, 65536
		    since FAT32 uses the least significant 28 bits of a 4-byte entry (uint32) as pointers to a cluster,
		       the maximum cluster pointer address of a FAT32 entry is 268,435,456. However, several
		       entries are reserved, notably 0x0FFFFFF7-0x0FFFFFFF flag bad cluster to end of file,
		       0x0000000 flags an empty cluster, and 0x0000001 is not used, so we only have
		       a potential 268,435,444 pointer entries
		    the maximum size of a disk for FAT32 is 16 sectors per cluster = 8KB/cluster * 268435444 = ~2TB

		Follow Microsoft's `format` commad as per http://www.win.tue.nl/~aeb/linux/fs/fat/fatgen103.pdf p. 20.
		Thanks to github.com/dosfstools/dosfstools for the link
		Filesystem size / cluster size
		   <= 260M      /   1 sector =   512 bytes
			 <=   8G      /   8 sector =  4096 bytes
			 <=  16G      /  32 sector = 16384 bytes
			 <=  32G      /  64 sector = 32768 bytes
			  >  32G      / 128 sector = 65536 bytes
	*/

	var sectorsPerCluster uint8
	switch {
	case size <= 260*MB:
		sectorsPerCluster = 1
	case size <= 8*GB:
		sectorsPerCluster = 8
	case size <= 16*GB:
		sectorsPerCluster = 32
	case size <= 32*GB:
		sectorsPerCluster = 64
	case size <= Fat32MaxSize:
		sectorsPerCluster = 128
	}

	// stick with uint32 and round down
	totalSectors := uint32(size / int64(SectorSize512))
	reservedSectors := uint16(32)
	dataSectors := totalSectors - uint32(reservedSectors)
	totalClusters := dataSectors / uint32(sectorsPerCluster)
	// FAT uses 4 bytes per cluster pointer
	//   so a 512 byte sector can store 512/4 = 128 pointer entries
	//   therefore sectors per FAT = totalClusters / 128
	sectorsPerFat := uint16(totalClusters / 128)

	// what is our FAT ID / Media Type?
	mediaType := uint8(MediaFixedDisk)

	fatIDbase := uint32(0x0f << 24)
	fatID := fatIDbase + 0xffff00 + uint32(mediaType)

	// we need an Extended BIOS Parameter Block
	dos20bpb := dos20BPB{
		sectorsPerCluster:    sectorsPerCluster,
		reservedSectors:      reservedSectors,
		fatCount:             2,
		totalSectors:         0,
		mediaType:            mediaType,
		bytesPerSector:       SectorSize512,
		rootDirectoryEntries: 0,
		sectorsPerFat:        0,
	}

	// some fake logic for heads, since everything is LBA access anyways
	dos331bpb := dos331BPB{
		dos20BPB:        &dos20bpb,
		totalSectors:    totalSectors,
		heads:           1,
		sectorsPerTrack: 1,
		hiddenSectors:   0,
	}

	ebpb := dos71EBPB{
		dos331BPB:             &dos331bpb,
		version:               fatVersion0,
		rootDirectoryCluster:  2,
		fsInformationSector:   fsisPrimarySector,
		backupBootSector:      backupBootSector,
		bootFileName:          [12]byte{0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0},
		extendedBootSignature: longDos71EBPB,
		volumeSerialNumber:    volid,
		volumeLabel:           "NO NAME    ",
		fileSystemType:        fileSystemTypeFAT32,
		mirrorFlags:           0,
		reservedFlags:         0,
		driveNumber:           128,
		sectorsPerFat:         uint32(sectorsPerFat),
	}
	// we need a new boot sector
	bs := msDosBootSector{
		oemName:            "godiskfs",
		jumpInstruction:    [3]byte{0xeb, 0x58, 0x90},
		bootCode:           []byte{},
		biosParameterBlock: &ebpb,
	}

	// create and allocate FAT32 FSInformationSector
	fsis := FSInformationSector{
		lastAllocatedCluster:  0xffffffff,
		freeDataClustersCount: 0xffffffff,
	}

	// create and allocate the FAT tables
	eocMarker := uint32(0x0fffffff)
	unusedMarker := uint32(0x00000000)
	fatPrimaryStart := reservedSectors * uint16(SectorSize512)
	fatSize := uint32(sectorsPerFat) * uint32(SectorSize512)
	fatSecondaryStart := uint64(fatPrimaryStart) + uint64(fatSize)
	maxCluster := fatSize / 4
	rootDirCluster := uint32(2)
	clusters := make([]uint32, maxCluster+1)
	clusters[rootDirCluster] = eocMarker
	fat := table{
		fatID:          fatID,
		eocMarker:      eocMarker,
		unusedMarker:   unusedMarker,
		size:           fatSize,
		rootDirCluster: rootDirCluster,
		clusters:       clusters,
		maxCluster:     maxCluster,
	}

	// where does our data start?
	dataStart := uint32(fatSecondaryStart) + fatSize

	// create the filesystem
	fs := &FileSystem{
		bootSector:      bs,
		fsis:            fsis,
		table:           fat,
		dataStart:       dataStart,
		bytesPerCluster: int(sectorsPerCluster) * int(SectorSize512),
		start:           start,
		size:            size,
		backend:         b,
	}

	// write the boot sector
	if err := fs.writeBootSector(); err != nil {
		return nil, fmt.Errorf("failed to write the boot sector: %w", err)
	}

	// write the fsis
	if err := fs.writeFsis(); err != nil {
		return nil, fmt.Errorf("failed to write the file system information sector: %w", err)
	}

	// write the FAT tables
	if err := fs.writeFat(); err != nil {
		return nil, fmt.Errorf("failed to write the file allocation table: %w", err)
	}

	// create root directory
	// be sure to zero out the root cluster, so we do not pick up phantom
	// entries.
	clusterStart := fs.start + int64(fs.dataStart)
	// length of cluster in bytes
	tmpb := make([]byte, fs.bytesPerCluster)
	// zero out the root directory cluster
	written, err := writableFile.WriteAt(tmpb, clusterStart)
	if err != nil {
		return nil, fmt.Errorf("failed to zero out root directory: %w", err)
	}
	if written != len(tmpb) || written != fs.bytesPerCluster {
		return nil, fmt.Errorf("incomplete zero out of root directory, wrote %d bytes instead of expected %d for cluster size %d", written, len(tmpb), fs.bytesPerCluster)
	}

	// create a volumelabel entry in the root directory
	rootDir := &Directory{
		directoryEntry: directoryEntry{
			clusterLocation: fs.table.rootDirCluster,
			isSubdirectory:  true,
			filesystem:      fs,
		},
	}
	// write the root directory entries to disk
	err = fs.writeDirectoryEntries(rootDir)
	if err != nil {
		return nil, fmt.Errorf("error writing root directory to disk: %w", err)
	}

	// set the volume label
	err = fs.SetLabel(volumeLabel)
	if err != nil {
		return nil, fmt.Errorf("failed to set volume label to '%s': %w", volumeLabel, err)
	}

	return fs, nil
}

// Read reads a filesystem from a given disk.
//
// requires the backend.Storage where to read the filesystem, size is the size of the filesystem in bytes,
// start is how far in bytes from the beginning of the backend.Storage the filesystem is expected to begin,
// and blocksize is is the logical blocksize to use for creating the filesystem
//
// note that you are *not* required to read a filesystem on the entire disk. You could have a disk of size
// 20GB, and a small filesystem of size 50MB that begins 2GB into the disk.
// This is extremely useful for working with filesystems on disk partitions.
//
// Note, however, that it is much easier to do this using the higher-level APIs at github.com/diskfs/go-diskfs
// which allow you to work directly with partitions, rather than having to calculate (and hopefully not make any errors)
// where a partition starts and ends.
//
// If the provided blocksize is 0, it will use the default of 512 bytes. If it is any number other than 0
// or 512, it will return an error.
func Read(b backend.Storage, size, start, blocksize int64) (*FileSystem, error) {
	// blocksize must be <=0 or exactly SectorSize512 or error
	if blocksize != int64(SectorSize512) && blocksize > 0 {
		return nil, fmt.Errorf("blocksize for FAT32 must be either 512 bytes or 0, not %d", blocksize)
	}
	if size > Fat32MaxSize {
		return nil, fmt.Errorf("requested size is larger than maximum allowed FAT32 size %d", Fat32MaxSize)
	}
	if size < blocksize*4 {
		return nil, fmt.Errorf("requested size is smaller than minimum allowed FAT32 size %d", blocksize*4)
	}
	// load the information from the disk
	// read first 512 bytes from the file
	bsb := make([]byte, SectorSize512)
	n, err := b.ReadAt(bsb, start)
	if err != nil {
		return nil, fmt.Errorf("could not read bytes from file: %w", err)
	}
	if uint16(n) < uint16(SectorSize512) {
		return nil, fmt.Errorf("only could read %d bytes from file", n)
	}
	bs, err := msDosBootSectorFromBytes(bsb)

	if err != nil {
		return nil, fmt.Errorf("error reading MS-DOS Boot Sector: %w", err)
	}

	sectorsPerFat := bs.biosParameterBlock.sectorsPerFat
	fatSize := sectorsPerFat * uint32(SectorSize512)
	reservedSectors := bs.biosParameterBlock.dos331BPB.dos20BPB.reservedSectors
	sectorsPerCluster := bs.biosParameterBlock.dos331BPB.dos20BPB.sectorsPerCluster
	fatPrimaryStart := uint64(reservedSectors) * uint64(SectorSize512)
	fatSecondaryStart := fatPrimaryStart + uint64(fatSize)

	fsisBytes := make([]byte, 512)
	read, err := b.ReadAt(fsisBytes, int64(bs.biosParameterBlock.fsInformationSector)*blocksize+start)
	if err != nil {
		return nil, fmt.Errorf("unable to read bytes for FSInformationSector: %w", err)
	}
	if read != 512 {
		return nil, fmt.Errorf("read %d bytes instead of expected %d for FS Information Sector", read, 512)
	}
	fsis, err := fsInformationSectorFromBytes(fsisBytes)
	if err != nil {
		return nil, fmt.Errorf("error reading FileSystem Information Sector: %w", err)
	}

	partitionTableBytes := make([]byte, fatSize)
	_, _ = b.ReadAt(partitionTableBytes, int64(fatPrimaryStart)+start)
	fat := tableFromBytes(partitionTableBytes)

	_, _ = b.ReadAt(partitionTableBytes, int64(fatSecondaryStart)+start)
	fat2 := tableFromBytes(partitionTableBytes)
	if !fat.equal(fat2) {
		return nil, errors.New("fat tables did not match")
	}
	dataStart := uint32(fatSecondaryStart) + fat.size

	return &FileSystem{
		bootSector:      *bs,
		fsis:            *fsis,
		table:           *fat,
		dataStart:       dataStart,
		bytesPerCluster: int(sectorsPerCluster) * int(SectorSize512),
		start:           start,
		size:            size,
		backend:         b,
	}, nil
}

func (fs *FileSystem) writeBootSector() error {
	//nolint:gocritic  // we do not want to remove this commented code, as it is useful for reference and debugging
	/*
		err := bs.write(f)
		if err != nil {
			return nil, fmt.Errorf("error writing MS-DOS Boot Sector: %v", err)
		}
	*/
	writableFile, err := fs.backend.Writable()
	if err != nil {
		return err
	}

	b, err := fs.bootSector.toBytes()
	if err != nil {
		return fmt.Errorf("error converting MS-DOS Boot Sector to bytes: %w", err)
	}

	// write main boot sector
	count, err := writableFile.WriteAt(b, 0+fs.start)
	if err != nil {
		return fmt.Errorf("error writing MS-DOS Boot Sector to disk: %w", err)
	}
	if count != int(SectorSize512) {
		return fmt.Errorf("wrote %d bytes of MS-DOS Boot Sector to disk instead of expected %d", count, SectorSize512)
	}

	// write backup boot sector to the file
	if fs.bootSector.biosParameterBlock.backupBootSector > 0 {
		count, err = writableFile.WriteAt(b, int64(fs.bootSector.biosParameterBlock.backupBootSector)*int64(SectorSize512)+fs.start)
		if err != nil {
			return fmt.Errorf("error writing MS-DOS Boot Sector to disk: %w", err)
		}
		if count != int(SectorSize512) {
			return fmt.Errorf("wrote %d bytes of MS-DOS Boot Sector to disk instead of expected %d", count, SectorSize512)
		}
	}

	return nil
}

func (fs *FileSystem) writeFsis() error {
	fsInformationSector := fs.bootSector.biosParameterBlock.fsInformationSector
	backupBootSector := fs.bootSector.biosParameterBlock.backupBootSector
	fsisPrimary := int64(fsInformationSector * uint16(SectorSize512))

	fsisBytes := fs.fsis.toBytes()
	writableFile, err := fs.backend.Writable()
	if err != nil {
		return err
	}

	if _, err := writableFile.WriteAt(fsisBytes, fsisPrimary+fs.start); err != nil {
		return fmt.Errorf("unable to write primary Fsis: %w", err)
	}

	if backupBootSector > 0 {
		if _, err := writableFile.WriteAt(fsisBytes, int64(backupBootSector+1)*int64(SectorSize512)+fs.start); err != nil {
			return fmt.Errorf("unable to write backup Fsis: %w", err)
		}
	}

	return nil
}

func (fs *FileSystem) writeFat() error {
	reservedSectors := fs.bootSector.biosParameterBlock.dos331BPB.dos20BPB.reservedSectors
	fatPrimaryStart := uint64(reservedSectors) * uint64(SectorSize512)
	fatSecondaryStart := fatPrimaryStart + uint64(fs.table.size)

	fatBytes := fs.table.bytes()
	writableFile, err := fs.backend.Writable()
	if err != nil {
		return err
	}

	if _, err := writableFile.WriteAt(fatBytes, int64(fatPrimaryStart)+fs.start); err != nil {
		return fmt.Errorf("unable to write primary FAT table: %w", err)
	}

	if _, err := writableFile.WriteAt(fatBytes, int64(fatSecondaryStart)+fs.start); err != nil {
		return fmt.Errorf("unable to write backup FAT table: %w", err)
	}

	return nil
}

// interface guard
var _ filesystem.FileSystem = (*FileSystem)(nil)

// Do cleaning job for fat32. Note that fat32 does not have side-effects so we do not do anything.
func (fs *FileSystem) Close() error {
	return nil
}

// Type returns the type code for the filesystem. Always returns filesystem.TypeFat32
func (fs *FileSystem) Type() filesystem.Type {
	return filesystem.TypeFat32
}

// Mkdir make a directory at the given path. It is equivalent to `mkdir -p`, i.e. idempotent, in that:
//
// * It will make the entire tree path if it does not exist
// * It will not return an error if the path already exists
func (fs *FileSystem) Mkdir(p string) error {
	_, _, err := fs.readDirWithMkdir(p, true)
	// we are not interesting in returning the entries
	return err
}

// creates a filesystem node (file, device special file, or named pipe) named pathname,
// with attributes specified by mode and dev
func (fs *FileSystem) Mknod(_ string, _ uint32, _ int) error {
	return filesystem.ErrNotSupported
}

// creates a new link (also known as a hard link) to an existing file.
func (fs *FileSystem) Link(_, _ string) error {
	return filesystem.ErrNotSupported
}

// creates a symbolic link named linkpath which contains the string target.
func (fs *FileSystem) Symlink(_, _ string) error {
	return filesystem.ErrNotSupported
}

// Chmod changes the mode of the named file to mode. If the file is a symbolic link,
// it changes the mode of the link's target.
func (fs *FileSystem) Chmod(_ string, _ os.FileMode) error {
	return filesystem.ErrNotSupported
}

// Chown changes the numeric uid and gid of the named file. If the file is a symbolic link,
// it changes the uid and gid of the link's target. A uid or gid of -1 means to not change that value
func (fs *FileSystem) Chown(_ string, _, _ int) error {
	return filesystem.ErrNotSupported
}

// ReadDir return the contents of a given directory in a given filesystem.
//
// Returns a slice of os.FileInfo with all of the entries in the directory.
//
// Will return an error if the directory does not exist or is a regular file and not a directory
func (fs *FileSystem) ReadDir(p string) ([]os.FileInfo, error) {
	_, entries, err := fs.readDirWithMkdir(p, false)
	if err != nil {
		return nil, fmt.Errorf("error reading directory %s: %w", p, err)
	}
	// once we have made it here, looping is done. We have found the final entry
	// we need to return all of the file info
	//nolint:prealloc // because the following loop may omit some entry
	var ret []os.FileInfo
	for _, e := range entries {
		if e.isVolumeLabel {
			continue
		}
		shortName := e.filenameShort
		if e.lowercaseShortname {
			shortName = strings.ToLower(shortName)
		}
		fileExtension := e.fileExtension
		if e.lowercaseExtension {
			fileExtension = strings.ToLower(fileExtension)
		}
		if fileExtension != "" {
			shortName = fmt.Sprintf("%s.%s", shortName, fileExtension)
		}
		ret = append(ret, FileInfo{
			modTime:   e.modifyTime,
			name:      e.filenameLong,
			shortName: shortName,
			size:      int64(e.fileSize),
			isDir:     e.isSubdirectory,
		})
	}
	return ret, nil
}

// OpenFile returns an io.ReadWriter from which you can read the contents of a file
// or write contents to the file
//
// accepts normal os.OpenFile flags
//
// returns an error if the file does not exist
func (fs *FileSystem) OpenFile(p string, flag int) (filesystem.File, error) {
	// get the path
	dir := path.Dir(p)
	filename := path.Base(p)
	// if the dir == filename, then it is just /
	if dir == filename {
		return nil, fmt.Errorf("cannot open directory %s as file", p)
	}
	// get the directory entries
	parentDir, entries, err := fs.readDirWithMkdir(dir, false)
	if err != nil {
		return nil, fmt.Errorf("could not read directory entries for %s: %w", dir, err)
	}
	// we now know that the directory exists, see if the file exists
	var targetEntry *directoryEntry
	for _, e := range entries {
		shortName := e.filenameShort
		if e.fileExtension != "" {
			shortName += "." + e.fileExtension
		}
		if !strings.EqualFold(e.filenameLong, filename) && !strings.EqualFold(shortName, filename) {
			continue
		}
		// cannot do anything with directories
		if e.isSubdirectory {
			return nil, fmt.Errorf("cannot open directory %s as file", p)
		}
		// if we got this far, we have found the file
		targetEntry = e
	}

	// see if the file exists
	// if the file does not exist, and is not opened for os.O_CREATE, return an error
	if targetEntry == nil {
		if flag&os.O_CREATE == 0 {
			return nil, fmt.Errorf("target file %s does not exist and was not asked to create", p)
		}
		// else create it
		targetEntry, err = fs.mkFile(parentDir, filename)
		if err != nil {
			return nil, fmt.Errorf("failed to create file %s: %w", p, err)
		}
		// write the directory entries to disk
		err = fs.writeDirectoryEntries(parentDir)
		if err != nil {
			return nil, fmt.Errorf("error writing directory file %s to disk: %w", p, err)
		}
	}
	offset := int64(0)

	// what if we were asked to truncate the file?
	if flag&os.O_TRUNC == os.O_TRUNC && targetEntry.fileSize != 0 {
		// pretty simple: change the filesize, and then remove all except the first cluster
		targetEntry.fileSize = 0
		// we should not need to change the parent, because it is all pointers
		if err := fs.writeDirectoryEntries(parentDir); err != nil {
			return nil, fmt.Errorf("error writing directory file %s to disk: %w", p, err)
		}
		if _, err := fs.allocateSpace(1, targetEntry.clusterLocation); err != nil {
			return nil, fmt.Errorf("unable to resize cluster list: %w", err)
		}
	}
	if flag&os.O_APPEND == os.O_APPEND {
		offset = int64(targetEntry.fileSize)
	}
	return &File{
		directoryEntry: targetEntry,
		isReadWrite:    flag&os.O_RDWR != 0,
		isAppend:       flag&os.O_APPEND != 0,
		offset:         offset,
		filesystem:     fs,
		parent:         parentDir,
	}, nil
}

// removes the named file or (empty) directory.
func (fs *FileSystem) Remove(pathname string) error {
	// get the path
	dir := path.Dir(pathname)
	filename := path.Base(pathname)
	// if the dir == filename, then it is just /
	if dir == filename {
		return fmt.Errorf("cannot remove directory %s as file", pathname)
	}
	// get the directory entries
	parentDir, entries, err := fs.readDirWithMkdir(dir, false)
	if err != nil {
		return fmt.Errorf("could not read directory entries for %s", dir)
	}
	// we now know that the directory exists, see if the file exists
	var targetEntry *directoryEntry
	for _, e := range entries {
		shortName := e.filenameShort
		if e.fileExtension != "" {
			shortName += "." + e.fileExtension
		}
		if e.filenameLong != filename && shortName != filename {
			continue
		}
		// cannot do anything with directories
		if e.isSubdirectory {
			content, err := fs.ReadDir(pathname)
			if err != nil {
				return fmt.Errorf("error while checking if file to delete is empty: %+v", err)
			}
			// '.' & '..' are always present in directory
			if len(content) > 2 {
				return fmt.Errorf("cannot remove non-empty directory %s", pathname)
			}
		}
		// if we got this far, we have found the file
		targetEntry = e
	}

	// see if the file exists
	// if the file does not exist, and is not opened for os.O_CREATE, return an error
	if targetEntry == nil {
		return fmt.Errorf("target file %s does not exist", pathname)
	}
	err = parentDir.removeEntry(filename)
	if err != nil {
		return fmt.Errorf("failed to remove file %s: %v", pathname, err)
	}

	// we need to make sure that clusters are removed which may not be used anymore
	_, err = fs.allocateSpace(uint64(parentDir.fileSize), parentDir.clusterLocation)
	if err != nil {
		return fmt.Errorf("failed to allocate clusters: %v", err)
	}

	// write the directory entries to disk
	err = fs.writeDirectoryEntries(parentDir)
	if err != nil {
		return fmt.Errorf("error writing directory file %s to disk: %v", pathname, err)
	}

	return nil
}

// Rename renames (moves) oldpath to newpath. If newpath already exists and is not a directory, Rename replaces it.
func (fs *FileSystem) Rename(oldpath, newpath string) error {
	// get the path
	dir := path.Dir(oldpath)
	filename := path.Base(oldpath)

	newDir := path.Dir(newpath)
	newname := path.Base(newpath)
	if dir != newDir {
		return errors.New("can only rename files within the same directory")
	}

	// if the dir == filename, then it is just /
	if dir == filename {
		return fmt.Errorf("cannot rename directory %s as file", oldpath)
	}
	// get the directory entries
	parentDir, entries, err := fs.readDirWithMkdir(dir, false)
	if err != nil {
		return fmt.Errorf("could not read directory entries for %s", dir)
	}
	// we now know that the directory exists, see if the file exists
	var targetEntry *directoryEntry
	for _, e := range entries {
		shortName := e.filenameShort
		if e.fileExtension != "" {
			shortName += "." + e.fileExtension
		}
		if e.filenameLong != filename && shortName != filename {
			continue
		}
		// if we got this far, we have found the file
		targetEntry = e
	}

	// see if the file exists
	// if the file does not exist, and is not opened for os.O_CREATE, return an error
	if targetEntry == nil {
		return fmt.Errorf("target file %s does not exist", oldpath)
	}
	err = parentDir.renameEntry(filename, newname)
	if err != nil {
		return fmt.Errorf("failed to rename file %s: %v", oldpath, err)
	}

	// we need to make sure that clusters are removed which may not be used anymore
	_, err = fs.allocateSpace(uint64(parentDir.fileSize), parentDir.clusterLocation)
	if err != nil {
		return fmt.Errorf("failed to allocate clusters: %v", err)
	}

	// write the directory entries to disk
	err = fs.writeDirectoryEntries(parentDir)
	if err != nil {
		return fmt.Errorf("error writing directory file %s to disk: %v", oldpath, err)
	}

	return nil
}

// Label get the label of the filesystem from the secial file in the root directory.
// The label stored in the boot sector is ignored to mimic Windows behavior which
// only stores and reads the label from the special file in the root directory.
func (fs *FileSystem) Label() string {
	// locate the filesystem root directory
	_, dirEntries, err := fs.readDirWithMkdir("/", false)
	if err != nil {
		return ""
	}

	// locate the label entry, it may not exist
	var labelEntry *directoryEntry
	for _, entry := range dirEntries {
		if entry.isVolumeLabel {
			labelEntry = entry
		}
	}

	// if we have no label entry, return
	if labelEntry == nil {
		return ""
	}

	// reconstruct the label, does not attempt to sanitize anything
	return labelEntry.filenameShort + labelEntry.fileExtension
}

// SetLabel changes the filesystem label
func (fs *FileSystem) SetLabel(volumeLabel string) error {
	if volumeLabel == "" {
		volumeLabel = "NO NAME"
	}

	// ensure the volumeLabel is proper sized
	volumeLabel = fmt.Sprintf("%-11.11s", volumeLabel)

	// set the label in the superblock
	bpb := fs.bootSector.biosParameterBlock
	if bpb == nil {
		return fmt.Errorf("failed to load the boot sector")
	}
	bpb.volumeLabel = volumeLabel

	// write the boot sector
	if err := fs.writeBootSector(); err != nil {
		return fmt.Errorf("failed to write the boot sector: %w", err)
	}

	// locate the filesystem root directory or create it
	rootDir, dirEntries, err := fs.readDirWithMkdir("/", false)
	if err != nil {
		return fmt.Errorf("failed to locate root directory: %w", err)
	}

	// locate the label entry, it may not exist
	var labelEntry *directoryEntry
	for _, entry := range dirEntries {
		if entry.isVolumeLabel {
			labelEntry = entry
		}
	}

	// if have an entry, change the label. Otherwise, create it
	if labelEntry != nil {
		labelEntry.filenameShort = volumeLabel[:8]
		labelEntry.fileExtension = volumeLabel[8:11]
	} else {
		_, err = fs.mkLabel(rootDir, volumeLabel)
		if err != nil {
			return fmt.Errorf("failed to create volume label root directory entry '%s': %w", volumeLabel, err)
		}
	}

	// write the root directory entries to disk
	err = fs.writeDirectoryEntries(rootDir)
	if err != nil {
		return fmt.Errorf("failed to save the root directory to disk: %w", err)
	}

	return nil
}

// read directory entries for a given cluster
func (fs *FileSystem) getClusterList(firstCluster uint32) ([]uint32, error) {
	// first, get the chain of clusters
	complete := false
	cluster := firstCluster

	// do we even have a valid cluster?
	if cluster > fs.table.maxCluster || fs.table.clusters[cluster] == 0 {
		return nil, fmt.Errorf("invalid start cluster: %d", cluster)
	}

	clusterList := make([]uint32, 0, 5)
	for !complete {
		// save the current cluster
		clusterList = append(clusterList, cluster)
		// get the next cluster
		newCluster := fs.table.clusters[cluster]
		// if it is EOC, we are done
		switch {
		case fs.table.isEoc(newCluster):
			complete = true
		case newCluster > fs.table.maxCluster:
			return nil, fmt.Errorf("invalid cluster chain at %d", newCluster)
		case cluster < 2:
			return nil, fmt.Errorf("invalid cluster chain at %d", cluster)
		}
		cluster = newCluster
	}
	return clusterList, nil
}

// read directory entries for a given cluster
func (fs *FileSystem) readDirectory(dir *Directory) ([]*directoryEntry, error) {
	clusterList, err := fs.getClusterList(dir.clusterLocation)
	if err != nil {
		return nil, fmt.Errorf("could not read cluster list: %w", err)
	}
	// read the data from all of the cluster entries in the list
	byteCount := len(clusterList) * fs.bytesPerCluster
	b := make([]byte, 0, byteCount)
	for _, cluster := range clusterList {
		// bytes where the cluster starts
		clusterStart := fs.start + int64(fs.dataStart) + int64(cluster-2)*int64(fs.bytesPerCluster)
		// length of cluster in bytes
		tmpb := make([]byte, fs.bytesPerCluster)
		// read the entire cluster
		_, _ = fs.backend.ReadAt(tmpb, clusterStart)
		b = append(b, tmpb...)
	}
	// get the directory
	if err := dir.entriesFromBytes(b); err != nil {
		return nil, err
	}
	return dir.entries, nil
}

// make a subdirectory
func (fs *FileSystem) mkSubdir(parent *Directory, name string) (*directoryEntry, error) {
	// get a cluster chain for the file
	clusters, err := fs.allocateSpace(1, 0)
	if err != nil {
		return nil, fmt.Errorf("could not allocate disk space for file %s: %w", name, err)
	}
	// create a directory entry for the file
	return parent.createEntry(name, clusters[0], true)
}

func (fs *FileSystem) writeDirectoryEntries(dir *Directory) error {
	// we need to save the entries of the parent
	b, err := dir.entriesToBytes(fs.bytesPerCluster)
	if err != nil {
		return fmt.Errorf("could not create a valid byte stream for a FAT32 Entries: %w", err)
	}

	writableFile, err := fs.backend.Writable()
	if err != nil {
		return err
	}
	// now have to expand with zeros to the a multiple of cluster lengths
	// how many clusters do we need, how many do we have?
	clusterList, err := fs.getClusterList(dir.clusterLocation)
	if err != nil {
		return fmt.Errorf("unable to get clusters for directory: %w", err)
	}

	if len(b) > len(clusterList)*fs.bytesPerCluster {
		clusters, err := fs.allocateSpace(uint64(len(b)), clusterList[0])
		if err != nil {
			return fmt.Errorf("unable to allocate space for directory entries: %w", err)
		}
		clusterList = clusters
	}
	// now write everything out to the cluster list
	// read the data from all of the cluster entries in the list
	for i, cluster := range clusterList {
		// bytes where the cluster starts
		clusterStart := fs.start + int64(fs.dataStart) + int64(cluster-2)*int64(fs.bytesPerCluster)
		bStart := i * fs.bytesPerCluster
		written, err := writableFile.WriteAt(b[bStart:bStart+fs.bytesPerCluster], clusterStart)
		if err != nil {
			return fmt.Errorf("error writing directory entries: %w", err)
		}
		if written != fs.bytesPerCluster {
			return fmt.Errorf("wrote %d bytes to cluster %d instead of expected %d", written, cluster, fs.bytesPerCluster)
		}
	}
	return nil
}

// mkFile make a file in a directory
func (fs *FileSystem) mkFile(parent *Directory, name string) (*directoryEntry, error) {
	// get a cluster chain for the file
	clusters, err := fs.allocateSpace(1, 0)
	if err != nil {
		return nil, fmt.Errorf("could not allocate disk space for directory %s: %w", name, err)
	}
	// create a directory entry for the file
	return parent.createEntry(name, clusters[0], false)
}

// mkLabel make a volume label in a directory
func (fs *FileSystem) mkLabel(parent *Directory, name string) (*directoryEntry, error) {
	// create a directory entry for the file
	return parent.createVolumeLabel(name)
}

// readDirWithMkdir - walks down a directory tree to the last entry
// if it does not exist, it may or may not make it
func (fs *FileSystem) readDirWithMkdir(p string, doMake bool) (*Directory, []*directoryEntry, error) {
	paths, err := splitPath(p)

	if err != nil {
		return nil, nil, err
	}
	// walk down the directory tree until all paths have been walked or we cannot find something
	// start with the root directory
	var entries []*directoryEntry
	currentDir := &Directory{
		directoryEntry: directoryEntry{
			clusterLocation: fs.table.rootDirCluster,
			isSubdirectory:  true,
			filesystem:      fs,
		},
	}
	entries, err = fs.readDirectory(currentDir)
	if err != nil {
		return nil, nil, fmt.Errorf("failed to read directory %s: %w", "/", err)
	}
	for i, subp := range paths {
		// do we have an entry whose name is the same as this name?
		found := false
		for _, e := range entries {
			// don't match volume label
			if e.isVolumeLabel {
				continue
			}
			// if the filename does not match, continue
			// match is determined by any one of:
			// - long filename == provided name
			// - uppercase(short filename) == uppercase(provided name)
			if !strings.EqualFold(e.filenameLong, subp) && !strings.EqualFold(e.filenameShort, subp) {
				continue
			}
			if !e.isSubdirectory {
				return nil, nil, fmt.Errorf("cannot create directory at %s since it is a file", "/"+strings.Join(paths[0:i+1], "/"))
			}
			// the filename matches, and it is a subdirectory, so we can break after saving the cluster
			found = true
			currentDir = &Directory{
				directoryEntry: *e,
			}
			break
		}

		// if not, either make it, retrieve its cluster and entries, and loop;
		//  or error out
		if !found {
			if doMake {
				var subdirEntry *directoryEntry
				subdirEntry, err = fs.mkSubdir(currentDir, subp)
				if err != nil {
					return nil, nil, fmt.Errorf("failed to create subdirectory %s: %w", "/"+strings.Join(paths[0:i+1], "/"), err)
				}
				currentDir.modifyTime = subdirEntry.createTime
				// make a basic entry for the new subdir
				parentDirectoryCluster := currentDir.clusterLocation
				if parentDirectoryCluster == 2 {
					// references to the root directory (cluster 2) must be stored as 0
					parentDirectoryCluster = 0
				}
				dir := &Directory{
					directoryEntry: directoryEntry{clusterLocation: subdirEntry.clusterLocation},
					entries: []*directoryEntry{
						{
							filenameShort:   ".",
							isSubdirectory:  true,
							clusterLocation: subdirEntry.clusterLocation,
							createTime:      subdirEntry.createTime,
							modifyTime:      subdirEntry.modifyTime,
							accessTime:      subdirEntry.accessTime,
						},
						{
							filenameShort:   "..",
							isSubdirectory:  true,
							clusterLocation: parentDirectoryCluster,
							createTime:      currentDir.createTime,
							modifyTime:      currentDir.modifyTime,
							accessTime:      currentDir.accessTime,
						},
					},
				}
				// write the new directory entries to disk
				err = fs.writeDirectoryEntries(dir)
				if err != nil {
					return nil, nil, fmt.Errorf("error writing new directory entries to disk: %w", err)
				}
				// write the parent directory entries to disk
				err = fs.writeDirectoryEntries(currentDir)
				if err != nil {
					return nil, nil, fmt.Errorf("error writing directory entries to disk: %w", err)
				}
				// save where we are to search next
				currentDir = &Directory{
					directoryEntry: *subdirEntry,
				}
			} else {
				return nil, nil, fmt.Errorf("path %s not found", "/"+strings.Join(paths[0:i+1], "/"))
			}
		}
		// get all of the entries in this directory
		entries, err = fs.readDirectory(currentDir)
		if err != nil {
			return nil, nil, fmt.Errorf("failed to read directory %s: %w", "/"+strings.Join(paths[0:i+1], "/"), err)
		}
	}
	// once we have made it here, looping is done; we have found the final entry
	return currentDir, entries, nil
}

// allocateSpace ensure that a cluster chain exists to handle a file of a given size.
// arguments are file size in bytes and starting cluster of the chain
// if starting is 0, then we are not (re)sizing an existing chain but creating a new one
// returns the indexes of clusters to be used in order. If the new size is smaller than
// the original size, will shrink the chain.
func (fs *FileSystem) allocateSpace(size uint64, previous uint32) ([]uint32, error) {
	if previous > fs.table.maxCluster {
		return nil, fmt.Errorf("invalid cluster chain at %d", previous)
	}

	var (
		clusters             []uint32
		err                  error
		lastAllocatedCluster uint32
	)
	// 1- calculate how many clusters needed
	// 2- see how many clusters already are allocated
	// 3- if needed, allocate new clusters and extend the chain in the FAT table
	allocated := make([]uint32, 0, 20)

	// what is the total count of clusters needed?
	count := int(size / uint64(fs.bytesPerCluster))
	if size%uint64(fs.bytesPerCluster) > 0 {
		count++
	}
	extraClusterCount := count

	clusters = make([]uint32, 0, 20)

	// are we extending an existing chain, or creating a new one?
	if previous >= 2 {
		clusters, err = fs.getClusterList(previous)
		if err != nil {
			return nil, fmt.Errorf("unable to get cluster list: %w", err)
		}
		originalClusterCount := len(clusters)
		extraClusterCount = count - originalClusterCount
		// make sure that previous is the last cluster of the previous chain
		previous = clusters[len(clusters)-1]
	}

	// what if we do not need to change anything?
	if extraClusterCount == 0 {
		return clusters, nil
	}

	// get a list of allocated clusters, so we can know which ones are unallocated and therefore allocatable
	maxCluster := fs.table.maxCluster

	if extraClusterCount > 0 {
		for i := uint32(2); i < maxCluster && len(allocated) < extraClusterCount; i++ {
			if fs.table.clusters[i] == 0 {
				// these become the same at this point
				allocated = append(allocated, i)
			}
		}

		// did we allocate them all?
		if len(allocated) < extraClusterCount {
			return nil, errors.New("no space left on device")
		}

		// mark last allocated one as EOC
		lastAlloc := len(allocated) - 1

		// extend the chain and fill them in
		if previous > 0 {
			fs.table.clusters[previous] = allocated[0]
		}
		for i := 0; i < lastAlloc; i++ {
			fs.table.clusters[allocated[i]] = allocated[i+1]
		}
		fs.table.clusters[allocated[lastAlloc]] = fs.table.eocMarker

		// update the FSIS
		lastAllocatedCluster = allocated[len(allocated)-1]
	} else {
		var (
			lastAlloc   int
			deallocated []uint32
		)
		toRemove := abs(extraClusterCount)
		lastAlloc = len(clusters) - toRemove - 1
		if lastAlloc < 0 {
			lastAlloc = 0
		}
		deallocated = clusters[lastAlloc+1:]

		if uint32(lastAlloc) > fs.table.maxCluster || clusters[lastAlloc] > fs.table.maxCluster {
			return nil, fmt.Errorf("invalid cluster chain at %d", lastAlloc)
		}

		// mark last allocated one as EOC
		fs.table.clusters[clusters[lastAlloc]] = fs.table.eocMarker

		// unmark all of the unused ones
		lastAllocatedCluster = fs.fsis.lastAllocatedCluster
		for _, cl := range deallocated {
			if cl > fs.table.maxCluster {
				return nil, fmt.Errorf("invalid cluster chain at %d", cl)
			}

			fs.table.clusters[cl] = fs.table.unusedMarker
			if cl == lastAllocatedCluster {
				lastAllocatedCluster--
			}
		}
	}

	// update the FSIS
	fs.fsis.lastAllocatedCluster = lastAllocatedCluster
	if err := fs.writeFsis(); err != nil {
		return nil, fmt.Errorf("failed to write the file system information sector: %w", err)
	}

	// write the FAT tables
	if err := fs.writeFat(); err != nil {
		return nil, fmt.Errorf("failed to write the file allocation table: %w", err)
	}

	// return all of the clusters
	return append(clusters, allocated...), nil
}

func abs(x int) int {
	if x < 0 {
		return -x
	}
	return x
}
