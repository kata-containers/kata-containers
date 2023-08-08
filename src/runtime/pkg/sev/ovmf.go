// Copyright contributors to AMD SEV/-ES in Go
//
// SPDX-License-Identifier: Apache-2.0

package sev

import (
	"bytes"
	"encoding/binary"
	"errors"
	"os"
)

// GUID 96b582de-1fb2-45f7-baea-a366c55a082d
var ovmfTableFooterGuid = guidLE{0xde, 0x82, 0xb5, 0x96, 0xb2, 0x1f, 0xf7, 0x45, 0xba, 0xea, 0xa3, 0x66, 0xc5, 0x5a, 0x08, 0x2d}

// GUID 00f771de-1a7e-4fcb-890e-68c77e2fb44e
var sevEsResetBlockGuid = guidLE{0xde, 0x71, 0xf7, 0x00, 0x7e, 0x1a, 0xcb, 0x4f, 0x89, 0x0e, 0x68, 0xc7, 0x7e, 0x2f, 0xb4, 0x4e}

type ovmfFooterTableEntry struct {
	Size uint16
	Guid guidLE
}

type ovmf struct {
	table map[guidLE][]byte
}

func NewOvmf(filename string) (ovmf, error) {
	buf, err := os.ReadFile(filename)
	if err != nil {
		return ovmf{}, err
	}
	table, err := parseFooterTable(buf)
	if err != nil {
		return ovmf{}, err
	}
	return ovmf{table}, nil
}

// Parse the OVMF footer table and return a map from GUID to entry value
func parseFooterTable(data []byte) (map[guidLE][]byte, error) {
	table := make(map[guidLE][]byte)

	buf := new(bytes.Buffer)
	err := binary.Write(buf, binary.LittleEndian, ovmfFooterTableEntry{})
	if err != nil {
		return table, err
	}
	entryHeaderSize := buf.Len()

	// The OVMF table ends 32 bytes before the end of the firmware binary
	startOfFooterTable := len(data) - 32 - entryHeaderSize
	footerBytes := bytes.NewReader(data[startOfFooterTable:])
	var footer ovmfFooterTableEntry
	err = binary.Read(footerBytes, binary.LittleEndian, &footer)
	if err != nil {
		return table, err
	}
	if footer.Guid != ovmfTableFooterGuid {
		// No OVMF footer table
		return table, nil
	}
	tableSize := int(footer.Size) - entryHeaderSize
	if tableSize < 0 {
		return table, nil
	}
	tableBytes := data[(startOfFooterTable - tableSize):startOfFooterTable]
	for len(tableBytes) >= entryHeaderSize {
		tsize := len(tableBytes)
		entryBytes := bytes.NewReader(tableBytes[tsize-entryHeaderSize:])
		var entry ovmfFooterTableEntry
		err := binary.Read(entryBytes, binary.LittleEndian, &entry)
		if err != nil {
			return table, err
		}
		if int(entry.Size) < entryHeaderSize {
			return table, errors.New("Invalid entry size")
		}
		entryData := tableBytes[tsize-int(entry.Size) : tsize-entryHeaderSize]
		table[entry.Guid] = entryData
		tableBytes = tableBytes[:tsize-int(entry.Size)]
	}
	return table, nil
}

func (o *ovmf) tableItem(guid guidLE) ([]byte, error) {
	value, ok := o.table[guid]
	if !ok {
		return []byte{}, errors.New("OVMF footer table entry not found")
	}
	return value, nil
}

func (o *ovmf) sevEsResetEip() (uint32, error) {
	value, err := o.tableItem(sevEsResetBlockGuid)
	if err != nil {
		return 0, err
	}
	return binary.LittleEndian.Uint32(value), nil
}
