package disk

// ReReadPartitionTable is used to re-read the partition table
// on the disk.
//
// In windows machine, force re-read is not done. The method returns nil when
// invoked
func (d *Disk) ReReadPartitionTable() error {
	return nil
}
