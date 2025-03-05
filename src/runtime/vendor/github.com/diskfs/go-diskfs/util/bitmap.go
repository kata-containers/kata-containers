package util

import "fmt"

// Bitmap is a structure holding a bitmap
type Bitmap struct {
	bits []byte
}

// Contiguous a position and count of contiguous bits, either free or set
type Contiguous struct {
	Position int
	Count    int
}

// BitmapFromBytes create a bitmap struct from bytes
func BitmapFromBytes(b []byte) *Bitmap {
	// just copy them over
	bits := make([]byte, len(b))
	copy(bits, b)
	bm := Bitmap{
		bits: bits,
	}

	return &bm
}

// NewBitmap creates a new bitmap of size bytes; it is not in bits to force the caller to have
// a complete set
func NewBitmap(bytes int) *Bitmap {
	bm := Bitmap{
		bits: make([]byte, bytes),
	}
	return &bm
}

// ToBytes returns raw bytes underlying the bitmap
func (bm *Bitmap) ToBytes() []byte {
	b := make([]byte, len(bm.bits))
	copy(b, bm.bits)

	return b
}

// FromBytes overwrite the existing map with the contents of the bytes.
// It is the equivalent of BitmapFromBytes, but uses an existing Bitmap.
func (bm *Bitmap) FromBytes(b []byte) {
	bm.bits = make([]byte, len(b))
	copy(bm.bits, b)
}

// IsSet check if a specific bit location is set
func (bm *Bitmap) IsSet(location int) (bool, error) {
	byteNumber, bitNumber := findBitForIndex(location)
	if byteNumber > len(bm.bits) {
		return false, fmt.Errorf("location %d is not in %d size bitmap", location, len(bm.bits)*8)
	}
	mask := byte(0x1) << bitNumber
	return bm.bits[byteNumber]&mask == mask, nil
}

// Clear a specific bit location
func (bm *Bitmap) Clear(location int) error {
	byteNumber, bitNumber := findBitForIndex(location)
	if byteNumber > len(bm.bits) {
		return fmt.Errorf("location %d is not in %d size bitmap", location, len(bm.bits)*8)
	}
	mask := byte(0x1) << bitNumber
	mask = ^mask
	bm.bits[byteNumber] &= mask
	return nil
}

// Set a specific bit location
func (bm *Bitmap) Set(location int) error {
	byteNumber, bitNumber := findBitForIndex(location)
	if byteNumber > len(bm.bits) {
		return fmt.Errorf("location %d is not in %d size bitmap", location, len(bm.bits)*8)
	}
	mask := byte(0x1) << bitNumber
	bm.bits[byteNumber] |= mask
	return nil
}

// FirstFree returns the first free bit in the bitmap
// Begins at start, so if you want to find the first free bit, pass start=1.
// Returns -1 if none found.
func (bm *Bitmap) FirstFree(start int) int {
	var location = -1
	candidates := bm.bits[start/8:]
	for i, b := range candidates {
		// if all used, continue to next byte
		if b&0xff == 0xff {
			continue
		}
		// not all used, so find first bit set to 0
		for j := uint8(0); j < 8; j++ {
			mask := byte(0x1) << j
			if b&mask != mask {
				location = 8*i + int(j)
				break
			}
		}
		break
	}
	return location
}

// FirstSet returns location of first set bit in the bitmap
func (bm *Bitmap) FirstSet() int {
	var location = -1
	for i, b := range bm.bits {
		// if all free, continue to next
		if b == 0x00 {
			continue
		}
		// not all free, so find first bit set to 1
		for j := uint8(0); j < 8; j++ {
			mask := byte(0x1) << j
			mask = ^mask
			if b|mask != mask {
				location = 8*i + (8 - int(j))
				break
			}
		}
		break
	}
	return location
}

// FreeList returns a slicelist of contiguous free locations by location.
// It is sorted by location. If you want to sort it by size, uses sort.Slice
// for example, if the bitmap is 10010010 00100000 10000010, it will return
//
//		 1: 2, // 2 free bits at position 1
//		 4: 2, // 2 free bits at position 4
//		 8: 3, // 3 free bits at position 8
//		11: 5  // 5 free bits at position 11
//	    17: 5  // 5 free bits at position 17
//		23: 1, // 1 free bit at position 23
//
// if you want it in reverse order, just reverse the slice.
func (bm *Bitmap) FreeList() []Contiguous {
	var list []Contiguous
	var location = -1
	var count = 0
	for i, b := range bm.bits {
		for j := uint8(0); j < 8; j++ {
			mask := byte(0x1) << j
			switch {
			case b&mask != mask:
				if location == -1 {
					location = 8*i + int(j)
				}
				count++
			case location != -1:
				list = append(list, Contiguous{location, count})
				location = -1
				count = 0
			}
		}
	}
	if location != -1 {
		list = append(list, Contiguous{location, count})
	}
	return list
}

func findBitForIndex(index int) (byteNumber int, bitNumber uint8) {
	return index / 8, uint8(index % 8)
}
