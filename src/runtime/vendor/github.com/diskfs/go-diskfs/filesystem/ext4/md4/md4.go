package md4

// rotateLeft rotates a 32-bit integer to the left
func rotateLeft(x uint32, s uint) uint32 {
	return (x << s) | (x >> (32 - s))
}

// basic MD4 functions
func f(x, y, z uint32) uint32 {
	return z ^ (x & (y ^ z))
}

func g(x, y, z uint32) uint32 {
	return (x & y) + ((x ^ y) & z)
}

func h(x, y, z uint32) uint32 {
	return x ^ y ^ z
}

// MD4 constants
const (
	k1 uint32 = 0
	k2 uint32 = 0x5A827999
	k3 uint32 = 0x6ED9EBA1
)

// round applies the round function as a macro
func round(f func(uint32, uint32, uint32) uint32, a, b, c, d, x uint32, s uint) uint32 {
	return rotateLeft(a+f(b, c, d)+x, s)
}

// halfMD4Transform basic cut-down MD4 transform.  Returns only 32 bits of result.
func HalfMD4Transform(buf [4]uint32, in []uint32) uint32 {
	var a, b, c, d = buf[0], buf[1], buf[2], buf[3]

	/* Round 1 */
	a = round(f, a, b, c, d, in[0]+k1, 3)
	d = round(f, d, a, b, c, in[1]+k1, 7)
	c = round(f, c, d, a, b, in[2]+k1, 11)
	b = round(f, b, c, d, a, in[3]+k1, 19)
	a = round(f, a, b, c, d, in[4]+k1, 3)
	d = round(f, d, a, b, c, in[5]+k1, 7)
	c = round(f, c, d, a, b, in[6]+k1, 11)
	b = round(f, b, c, d, a, in[7]+k1, 19)

	/* Round 2 */
	a = round(g, a, b, c, d, in[1]+k2, 3)
	d = round(g, d, a, b, c, in[3]+k2, 5)
	c = round(g, c, d, a, b, in[5]+k2, 9)
	b = round(g, b, c, d, a, in[7]+k2, 13)
	a = round(g, a, b, c, d, in[0]+k2, 3)
	d = round(g, d, a, b, c, in[2]+k2, 5)
	c = round(g, c, d, a, b, in[4]+k2, 9)
	b = round(g, b, c, d, a, in[6]+k2, 13)

	/* Round 3 */
	a = round(h, a, b, c, d, in[3]+k3, 3)
	d = round(h, d, a, b, c, in[7]+k3, 9)
	c = round(h, c, d, a, b, in[2]+k3, 11)
	b = round(h, b, c, d, a, in[6]+k3, 15)
	a = round(h, a, b, c, d, in[1]+k3, 3)
	d = round(h, d, a, b, c, in[5]+k3, 9)
	c = round(h, c, d, a, b, in[0]+k3, 11)
	b = round(h, b, c, d, a, in[4]+k3, 15)

	buf[0] += a
	buf[1] += b
	buf[2] += c
	buf[3] += d

	return buf[1]
}
