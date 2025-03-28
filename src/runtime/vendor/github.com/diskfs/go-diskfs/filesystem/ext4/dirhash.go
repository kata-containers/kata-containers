package ext4

import (
	"github.com/diskfs/go-diskfs/filesystem/ext4/md4"
)

const (
	teaDelta       uint32 = 0x9E3779B9
	k1             uint32 = 0
	k2             uint32 = 0o13240474631
	k3             uint32 = 0o15666365641
	ext4HtreeEOF32 uint32 = ((1 << (32 - 1)) - 1)
	ext4HtreeEOF64 uint64 = ((1 << (64 - 1)) - 1)
)

type hashVersion uint8

const (
	HashVersionLegacy          = 0
	HashVersionHalfMD4         = 1
	HashVersionTEA             = 2
	HashVersionLegacyUnsigned  = 3
	HashVersionHalfMD4Unsigned = 4
	HashVersionTEAUnsigned     = 5
	HashVersionSIP             = 6
)

func TEATransform(buf [4]uint32, in []uint32) [4]uint32 {
	var sum uint32
	var b0, b1 = buf[0], buf[1]
	var a, b, c, d = in[0], in[1], in[2], in[3]
	var n = 16

	for ; n > 0; n-- {
		sum += teaDelta
		b0 += ((b1 << 4) + a) ^ (b1 + sum) ^ ((b1 >> 5) + b)
		b1 += ((b0 << 4) + c) ^ (b0 + sum) ^ ((b0 >> 5) + d)
	}

	buf[0] += b0
	buf[1] += b1
	return buf
}

// the old legacy hash
//
//nolint:unparam,revive // we do not used signed, but we probably should, so leaving until we are sure
func dxHackHash(name string, signed bool) uint32 {
	var hash uint32
	var hash0, hash1 uint32 = 0x12a3fe2d, 0x37abe8f9
	b := []byte(name)

	for i := len(b); i > 0; i-- {
		// get the specific character
		c := int(b[i-1])
		// the value of the individual character depends on if it is signed or not
		hash = hash1 + (hash0 ^ uint32(c*7152373))

		if hash&0x80000000 != 0 {
			hash -= 0x7fffffff
		}
		hash1 = hash0
		hash0 = hash
	}
	return hash0 << 1
}

//nolint:unparam,revive // we do not used signed, but we probably should, so leaving until we are sure
func str2hashbuf(msg string, num int, signed bool) []uint32 {
	var buf [8]uint32
	var pad, val uint32
	b := []byte(msg)
	size := len(b)

	pad = uint32(size) | (uint32(size) << 8)
	pad |= pad << 16

	val = pad
	if size > num*4 {
		size = num * 4
	}
	var j int
	for i := 0; i < size; i++ {
		c := int(b[i])
		val = uint32(c) + (val << 8)
		if (i % 4) == 3 {
			buf[j] = val
			val = pad
			num--
			j++
		}
	}
	num--
	if num >= 0 {
		buf[j] = val
		j++
	}
	for num--; num >= 0; num-- {
		buf[j] = pad
		j++
	}
	return buf[:]
}

func ext4fsDirhash(name string, version hashVersion, seed []uint32) (hash, minorHash uint32) {
	/* Initialize the default seed for the hash checksum functions */
	var buf = [4]uint32{0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476}

	// Check to see if the seed is all zero, and if so, use the default
	for i, val := range seed {
		if val != 0 {
			buf[i] = val
		}
	}

	switch version {
	case HashVersionLegacyUnsigned:
		hash = dxHackHash(name, false)
	case HashVersionLegacy:
		hash = dxHackHash(name, true)
	case HashVersionHalfMD4Unsigned:
		for i := 0; i < len(name); i += 32 {
			in := str2hashbuf(name[i:], 8, false)
			buf[1] = md4.HalfMD4Transform(buf, in)
		}
		minorHash = buf[2]
		hash = buf[1]
	case HashVersionHalfMD4:
		for i := 0; i < len(name); i += 32 {
			in := str2hashbuf(name[i:], 8, true)
			buf[1] = md4.HalfMD4Transform(buf, in)
		}
		minorHash = buf[2]
		hash = buf[1]
	case HashVersionTEAUnsigned:
		for i := 0; i < len(name); i += 16 {
			in := str2hashbuf(name[i:], 4, false)
			buf = TEATransform(buf, in)
		}
		hash = buf[0]
		minorHash = buf[1]
	case HashVersionTEA:
		for i := 0; i < len(name); i += 16 {
			in := str2hashbuf(name[i:], 4, true)
			buf = TEATransform(buf, in)
		}
		hash = buf[0]
		minorHash = buf[1]
	default:
		return 0, 0
	}
	hash &= ^uint32(1)
	if hash == (ext4HtreeEOF32 << 1) {
		hash = (ext4HtreeEOF32 - 1) << 1
	}
	return hash, minorHash
}
