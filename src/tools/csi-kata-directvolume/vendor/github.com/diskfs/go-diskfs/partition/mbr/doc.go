// Package mbr provides an interface to Master Boot Record (MBR) partitioned disks.
//
// You can use this package to manipulate existing MBR disks, read existing disks, or create entirely
// new partition tables on disks or disk files.
//
// mbr.Table implements the Table interface in github.com/diskfs/go-diskfs/partition
//
// Normally, the best way to interact with a disk is to use the github.com/diskfs/go-diskfs package,
// which, when necessary, will call this one. When creating a new disk or manipulating an existing one,
// You will, however, need to interact with an mbr.Table and mbr.Partition structs.
//
// Here is a simple example of an MBR Table with a single 10MB Linux partition:
//
//	table := &mbr.Table{
//	  LogicalSectorSize:  512,
//	  PhysicalSectorSize: 512,
//	  Partitions: []*mbr.Partition{
//	    {
//	      Bootable:      false,
//	      Type:          Linux,
//	      Start:         2048,
//	      Size:          20480,
//	    },
//	  },
//	}
package mbr
