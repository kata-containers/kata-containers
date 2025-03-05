// Package gpt provides an interface to GUID Partition Table (GPT) partitioned disks.
//
// You can use this package to manipulate existing GPT disks, read existing disks, or create entirely
// new partition tables on disks or disk files.
//
// gpt.Table implements the Table interface in github.com/diskfs/go-diskfs/partition
//
// Normally, the best way to interact with a disk is to use the github.com/diskfs/go-diskfs package,
// which, when necessary, will call this one. When creating a new disk or manipulating an existing one,
// You will, however, need to interact with an gpt.Table and gpt.Partition structs.
//
// Here is a simple example of a GPT Table with a single 10MB Linux partition:
//
//	table := &gpt.Table{
//	  LogicalSectorSize:  512,
//	  PhysicalSectorSize: 512,
//	  Partitions: []*mbr.Partition{
//	    {
//	      LogicalSectorSize:  512,
//	      PhysicalSectorSize: 512,
//	      ProtectiveMBR:      true,
//	      GUID:               "43E51892-3273-42F7-BCDA-B43B80CDFC48",
//	    },
//	  },
//	}
package gpt
