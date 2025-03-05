package fat32

import (
	"encoding/binary"
	"slices"
)

// table a FAT32 table
type table struct {
	fatID          uint32
	eocMarker      uint32
	unusedMarker   uint32
	clusters       []uint32
	rootDirCluster uint32
	size           uint32
	maxCluster     uint32
}

func (t *table) equal(a *table) bool {
	if (t == nil && a != nil) || (t != nil && a == nil) {
		return false
	}
	if t == nil && a == nil {
		return true
	}
	return t.fatID == a.fatID &&
		t.eocMarker == a.eocMarker &&
		t.rootDirCluster == a.rootDirCluster &&
		t.size == a.size &&
		t.maxCluster == a.maxCluster &&
		slices.Equal(a.clusters, t.clusters)
}

/*
  when reading from disk, remember that *any* of the following is a valid eocMarker:
  0x?ffffff8 - 0x?fffffff
*/

func tableFromBytes(b []byte) *table {
	maxCluster := uint32(len(b) / 4)

	t := table{
		fatID:          binary.LittleEndian.Uint32(b[0:4]),
		eocMarker:      binary.LittleEndian.Uint32(b[4:8]),
		size:           uint32(len(b)),
		clusters:       make([]uint32, maxCluster+1),
		maxCluster:     maxCluster,
		rootDirCluster: 2, // always 2 for FAT32
	}
	// just need to map the clusters in
	for i := uint32(2); i < t.maxCluster; i++ {
		bStart := i * 4
		bEnd := bStart + 4
		val := binary.LittleEndian.Uint32(b[bStart:bEnd])
		// 0 indicates an empty cluster, so we can ignore
		if val != 0 {
			t.clusters[i] = val
		}
	}
	return &t
}

// bytes returns a FAT32 table as bytes ready to be written to disk
func (t *table) bytes() []byte {
	b := make([]byte, t.size)

	// FAT ID and fixed values
	binary.LittleEndian.PutUint32(b[0:4], t.fatID)
	// End-of-Cluster marker
	binary.LittleEndian.PutUint32(b[4:8], t.eocMarker)
	// now just clusters
	numClusters := t.maxCluster
	for i := uint32(2); i < numClusters; i++ {
		bStart := i * 4
		bEnd := bStart + 4
		val := t.clusters[i]
		binary.LittleEndian.PutUint32(b[bStart:bEnd], val)
	}

	return b
}

func (t *table) isEoc(cluster uint32) bool {
	return cluster&0xFFFFFF8 == 0xFFFFFF8
}
