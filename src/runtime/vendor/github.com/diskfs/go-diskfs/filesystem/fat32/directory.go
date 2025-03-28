package fat32

import (
	"fmt"
	"time"
)

// Directory represents a single directory in a FAT32 filesystem
type Directory struct {
	directoryEntry
	entries []*directoryEntry
}

// dirEntriesFromBytes loads the directory entries from the raw bytes
func (d *Directory) entriesFromBytes(b []byte) error {
	entries, err := parseDirEntries(b)
	if err != nil {
		return err
	}
	d.entries = entries
	return nil
}

// entriesToBytes convert our entries to raw bytes
func (d *Directory) entriesToBytes(bytesPerCluster int) ([]byte, error) {
	b := make([]byte, 0)
	for _, de := range d.entries {
		b2, err := de.toBytes()
		if err != nil {
			return nil, err
		}
		b = append(b, b2...)
	}
	remainder := len(b) % bytesPerCluster
	extra := bytesPerCluster - remainder
	zeroes := make([]byte, extra)
	b = append(b, zeroes...)
	return b, nil
}

// createEntry creates an entry in the given directory, and returns the handle to it
func (d *Directory) createEntry(name string, cluster uint32, dir bool) (*directoryEntry, error) {
	// is it a long filename or a short filename?
	var isLFN bool
	// TODO: convertLfnSfn does not calculate if the short name conflicts and thus should increment the last character
	//       that should happen here, once we can look in the directory entry
	shortName, extension, isLFN, _ := convertLfnSfn(name)
	lfn := ""
	if isLFN {
		lfn = name
	}

	// allocate a slot for the new filename in the existing directory
	entry := directoryEntry{
		filenameLong:      lfn,
		longFilenameSlots: -1, // indicate that we do not know how many slots, which will force a recalculation
		filenameShort:     shortName,
		fileExtension:     extension,
		fileSize:          uint32(0),
		clusterLocation:   cluster,
		filesystem:        d.filesystem,
		createTime:        time.Now(),
		modifyTime:        time.Now(),
		accessTime:        time.Now(),
		isSubdirectory:    dir,
		isNew:             true,
	}

	entry.longFilenameSlots = calculateSlots(entry.filenameLong)
	d.entries = append(d.entries, &entry)
	return &entry, nil
}

// removeEntry removes an entry in the given directory
func (d *Directory) removeEntry(name string) error {
	// TODO implement check for long/short filename after increment of sfn is correctly implemented

	removeEntryIndex := -1
	for i, entry := range d.entries {
		if entry.filenameLong == name { // || entry.filenameShort == shortName  do not compare SFN, since it is not incremented correctly
			removeEntryIndex = i
		}
	}

	if removeEntryIndex == -1 {
		return fmt.Errorf("cannot find entry for name %s", name)
	}

	// remove the entry from the list
	d.entries = append(d.entries[:removeEntryIndex], d.entries[removeEntryIndex+1:]...)

	return nil
}

// renameEntry renames an entry in the given directory, and returns the handle to it
func (d *Directory) renameEntry(oldFileName, newFileName string) error {
	// TODO implement check for long/short filename after increment of sfn is correctly implemented

	newEntries := make([]*directoryEntry, 0, len(d.entries))
	var isReplaced = false
	for _, entry := range d.entries {
		if entry.filenameLong == newFileName {
			continue // skip adding already existing file, will be overwritten
		}
		if entry.filenameLong == oldFileName { //  || entry.filenameShort == shortName  do not compare SFN, since it is not incremented correctly
			var lfn string
			shortName, extension, isLFN, _ := convertLfnSfn(newFileName)
			if isLFN {
				lfn = newFileName
			}
			entry.filenameLong = lfn
			entry.filenameShort = shortName
			entry.fileExtension = extension
			entry.modifyTime = time.Now()
			isReplaced = true
		}
		newEntries = append(newEntries, entry)
	}
	if !isReplaced {
		return fmt.Errorf("cannot find file entry for %s", oldFileName)
	}

	d.entries = newEntries

	return nil
}

// createVolumeLabel create a volume label entry in the given directory, and return the handle to it
func (d *Directory) createVolumeLabel(name string) (*directoryEntry, error) {
	// allocate a slot for the new filename in the existing directory
	entry := directoryEntry{
		filenameLong:      "",
		longFilenameSlots: -1, // indicate that we do not know how many slots, which will force a recalculation
		filenameShort:     name[:8],
		fileExtension:     name[8:11],
		fileSize:          uint32(0),
		clusterLocation:   0,
		filesystem:        d.filesystem,
		createTime:        time.Now(),
		modifyTime:        time.Now(),
		accessTime:        time.Now(),
		isSubdirectory:    false,
		isNew:             true,
		isVolumeLabel:     true,
	}

	d.entries = append(d.entries, &entry)
	return &entry, nil
}
