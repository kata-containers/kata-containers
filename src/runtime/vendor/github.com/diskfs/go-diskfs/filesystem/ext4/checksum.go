package ext4

import (
	"encoding/binary"

	"github.com/diskfs/go-diskfs/filesystem/ext4/crc"
)

// checksumAppender is a function that takes a byte slice and returns a byte slice with a checksum appended
type checksumAppender func([]byte) []byte
type checksummer func([]byte) uint32

// directoryChecksummer returns a function that implements checksumAppender for a directory entries block
// original calculations can be seen for e2fsprogs https://git.kernel.org/pub/scm/fs/ext2/e2fsprogs.git/tree/lib/ext2fs/csum.c#n301
// and in the linux tree https://github.com/torvalds/linux/blob/master/fs/ext4/namei.c#L376-L384
func directoryChecksummer(seed, inodeNumber, inodeGeneration uint32) checksummer {
	numBytes := make([]byte, 4)
	binary.LittleEndian.PutUint32(numBytes, inodeNumber)
	crcResult := crc.CRC32c(seed, numBytes)
	genBytes := make([]byte, 4)
	binary.LittleEndian.PutUint32(genBytes, inodeGeneration)
	crcResult = crc.CRC32c(crcResult, genBytes)
	return func(b []byte) uint32 {
		checksum := crc.CRC32c(crcResult, b)
		return checksum
	}
}

// directoryChecksumAppender returns a function that implements checksumAppender for a directory entries block
// original calculations can be seen for e2fsprogs https://git.kernel.org/pub/scm/fs/ext2/e2fsprogs.git/tree/lib/ext2fs/csum.c#n301
// and in the linux tree https://github.com/torvalds/linux/blob/master/fs/ext4/namei.c#L376-L384
//
//nolint:unparam // inodeGeneration is always 0
func directoryChecksumAppender(seed, inodeNumber, inodeGeneration uint32) checksumAppender {
	fn := directoryChecksummer(seed, inodeNumber, inodeGeneration)
	return func(b []byte) []byte {
		checksum := fn(b)
		checksumBytes := make([]byte, 12)
		checksumBytes[4] = 12
		checksumBytes[7] = 0xde
		binary.LittleEndian.PutUint32(checksumBytes[8:12], checksum)
		b = append(b, checksumBytes...)
		return b
	}
}

// nullDirectoryChecksummer does not change anything
func nullDirectoryChecksummer(b []byte) []byte {
	return b
}
