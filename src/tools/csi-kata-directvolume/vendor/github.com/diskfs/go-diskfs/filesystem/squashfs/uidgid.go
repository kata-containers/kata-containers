package squashfs

import "encoding/binary"

const (
	idEntrySize = 4
)

func parseIDTable(b []byte) []uint32 {
	uidgids := make([]uint32, 0)
	for i := 0; i+4-1 < len(b); i += 4 {
		uidgids = append(uidgids, binary.LittleEndian.Uint32(b[i:i+4]))
	}
	return uidgids
}
