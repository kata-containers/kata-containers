package iso9660

// Directory represents a single directory in a FAT32 filesystem
type Directory struct {
	directoryEntry
	entries []*directoryEntry
}

// dirEntriesFromBytes loads the directory entries from the raw bytes
func (d *Directory) entriesFromBytes(b []byte, f *FileSystem) error {
	entries, err := parseDirEntries(b, f)
	if err != nil {
		return err
	}
	d.entries = entries
	return nil
}

// entriesToBytes convert our entries to raw bytes
func (d *Directory) entriesToBytes(ceBlockLocations []uint32) ([][]byte, error) {
	b := make([]byte, 0)
	ceBlocks := make([][]byte, 0)
	blocksize := int(d.filesystem.blocksize)
	for _, de := range d.entries {
		b2, err := de.toBytes(false, ceBlockLocations)
		if err != nil {
			return nil, err
		}
		recBytes := b2[0]
		// a directory entry cannot cross a block boundary
		// so if adding this puts us past it, then pad it
		// but only if we are not already exactly at the boundary
		newlength := len(b) + len(recBytes)
		left := blocksize - len(b)%blocksize
		if left != 0 && newlength/blocksize > len(b)/blocksize {
			b = append(b, make([]byte, left)...)
		}
		b = append(b, recBytes...)
		if len(b2) > 1 {
			ceBlocks = append(ceBlocks, b2[1:]...)
		}
	}
	// in the end, must pad to exact blocks
	left := blocksize - len(b)%blocksize
	if left > 0 {
		b = append(b, make([]byte, left)...)
	}
	ret := [][]byte{b}
	if len(ceBlocks) > 0 {
		ret = append(ret, ceBlocks...)
	}
	return ret, nil
}
