// +build windows

package iso9660

func statt(sys interface{}) (uint32, uint32, uint32) {
	return uint32(0), uint32(0), uint32(0)
}
