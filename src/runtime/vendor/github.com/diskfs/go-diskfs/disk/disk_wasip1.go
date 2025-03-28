//go:build wasip1
// +build wasip1

package disk

import "errors"

func (d *Disk) ReReadPartitionTable() error {
	return errors.New("not implemented")
}
