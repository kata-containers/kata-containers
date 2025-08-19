package iso9660

import (
	"strings"
)

const (
	// KB represents one KB
	KB int64 = 1024
	// MB represents one MB
	MB int64 = 1024 * KB
	// GB represents one GB
	GB int64 = 1024 * MB
	// TB represents one TB
	TB int64 = 1024 * GB
)

func universalizePath(p string) string {
	// globalize the separator
	return strings.ReplaceAll(p, `\`, "/")
}
func splitPath(p string) []string {
	ps := universalizePath(p)
	// we need to split such that each one ends in "/", except possibly the last one
	parts := strings.Split(ps, "/")
	// eliminate empty parts
	ret := make([]string, 0)
	for _, sub := range parts {
		if sub != "" {
			ret = append(ret, sub)
		}
	}
	return ret
}

func ucs2StringToBytes(s string) []byte {
	rs := []rune(s)
	l := len(rs)
	b := make([]byte, 0, 2*l)
	// big endian
	for _, r := range rs {
		tmpb := []byte{byte(r >> 8), byte(r & 0x00ff)}
		b = append(b, tmpb...)
	}
	return b
}

// bytesToUCS2String convert bytes to UCS-2. We aren't 100% sure that this is right,
// as it is possible to pass it an odd number of characters. But good enough for now.
func bytesToUCS2String(b []byte) string {
	r := make([]rune, 0, 30)
	// now we can iterate - be careful in case we were given an odd number of bytes
	for i := 0; i < len(b); {
		// little endian
		var val uint16
		if i >= len(b)-1 {
			val = uint16(b[i])
		} else {
			val = uint16(b[i])<<8 + uint16(b[i+1])
		}
		r = append(r, rune(val))
		i += 2
	}
	return string(r)
}

// maxInt returns the larger of x or y.
func maxInt(x, y int) int {
	if x < y {
		return y
	}
	return x
}
