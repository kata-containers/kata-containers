package mbr

// Type constants for the GUID for type of partition, see https://en.wikipedia.org/wiki/GUID_Partition_Table#Partition_entries
type Type byte

// List of GUID partition types
const (
	Empty         Type = 0x00
	Fat12         Type = 0x01
	XenixRoot     Type = 0x02
	XenixUsr      Type = 0x03
	Fat16         Type = 0x04
	ExtendedCHS   Type = 0x05
	Fat16b        Type = 0x06
	NTFS          Type = 0x07
	CommodoreFAT  Type = 0x08
	Fat32CHS      Type = 0x0b
	Fat32LBA      Type = 0x0c
	Fat16bLBA     Type = 0x0e
	ExtendedLBA   Type = 0x0f
	LinuxSwap     Type = 0x82
	Linux         Type = 0x83
	LinuxExtended Type = 0x85
	LinuxLVM      Type = 0x8e
	Iso9660       Type = 0x96
	MacOSXUFS     Type = 0xa8
	MacOSXBoot    Type = 0xab
	HFS           Type = 0xaf
	Solaris8Boot  Type = 0xbe
	GPTProtective Type = 0xee
	EFISystem     Type = 0xef
	VMWareFS      Type = 0xfb
	VMWareSwap    Type = 0xfc
)
