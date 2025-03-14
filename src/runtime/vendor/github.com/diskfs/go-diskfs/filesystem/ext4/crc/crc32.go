package crc

import (
	"encoding/binary"
	"hash/crc32"
)

// Define the CRC32C table using the Castagnoli polynomial
var (
	crc32cTable  = crc32.MakeTable(crc32.Castagnoli)
	crc32cTables = generateTables(crc32cTable)
)

func generateTables(poly *crc32.Table) [8][256]uint32 {
	var tab [8][256]uint32
	tab[0] = *poly

	for i := 0; i < 256; i++ {
		crc := tab[0][i]
		for j := 1; j < 8; j++ {
			crc = (crc >> 8) ^ tab[0][crc&0xff]
			tab[j][i] = crc
		}
	}

	return tab
}

func CRC32c(base uint32, b []byte) uint32 {
	// Compute the CRC32C checksum
	// for reasons unknown, the checksum from go package hash/crc32, using crc32.Update(), is different from the one calculated by the kernel
	// so we use this
	return crc32Body(base, b, &crc32cTables)
}

// doCRC processes a single byte
func doCRC(crc uint32, x byte, tab *[256]uint32) uint32 {
	return tab[(crc^uint32(x))&0xff] ^ (crc >> 8)
}

// doCRC4 processes 4 bytes
func doCRC4(q uint32, tab *[8][256]uint32) uint32 {
	return tab[3][q&0xff] ^ tab[2][(q>>8)&0xff] ^ tab[1][(q>>16)&0xff] ^ tab[0][(q>>24)&0xff]
}

// doCRC8 processes 8 bytes
func doCRC8(q uint32, tab *[8][256]uint32) uint32 {
	return tab[7][q&0xff] ^ tab[6][(q>>8)&0xff] ^ tab[5][(q>>16)&0xff] ^ tab[4][(q>>24)&0xff]
}

func crc32Body(crc uint32, buf []byte, tab *[8][256]uint32) uint32 {
	// Align it
	for len(buf) > 0 && (uintptr(len(buf))&3) != 0 {
		crc = doCRC(crc, buf[0], &tab[0])
		buf = buf[1:]
	}

	// Process in chunks of 8 bytes
	remLen := len(buf) % 8
	for len(buf) >= 8 {
		q := crc ^ binary.LittleEndian.Uint32(buf[:4])
		crc = doCRC8(q, tab)
		q = binary.LittleEndian.Uint32(buf[4:8])
		crc ^= doCRC4(q, tab)
		buf = buf[8:]
	}

	// Process remaining bytes
	for _, b := range buf[:remLen] {
		crc = doCRC(crc, b, &tab[0])
	}

	return crc
}
