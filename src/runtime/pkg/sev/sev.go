// Copyright contributors to AMD SEV/-ES in Go
//
// SPDX-License-Identifier: Apache-2.0
//

// Package sev can be used to compute the expected hash values for
// SEV/-ES pre-launch attestation
package sev

import (
	"bytes"
	"crypto/sha256"
	"encoding/binary"
	"io"
	"os"
)

type guidLE [16]byte

// The following definitions must be identical to those in QEMU target/i386/sev.c

// GUID: 9438d606-4f22-4cc9-b479-a793d411fd21
var sevHashTableHeaderGuid = guidLE{0x06, 0xd6, 0x38, 0x94, 0x22, 0x4f, 0xc9, 0x4c, 0xb4, 0x79, 0xa7, 0x93, 0xd4, 0x11, 0xfd, 0x21}

// GUID: 4de79437-abd2-427f-b835-d5b172d2045b
var sevKernelEntryGuid = guidLE{0x37, 0x94, 0xe7, 0x4d, 0xd2, 0xab, 0x7f, 0x42, 0xb8, 0x35, 0xd5, 0xb1, 0x72, 0xd2, 0x04, 0x5b}

// GUID: 44baf731-3a2f-4bd7-9af1-41e29169781d
var sevInitrdEntryGuid = guidLE{0x31, 0xf7, 0xba, 0x44, 0x2f, 0x3a, 0xd7, 0x4b, 0x9a, 0xf1, 0x41, 0xe2, 0x91, 0x69, 0x78, 0x1d}

// GUID: 97d02dd8-bd20-4c94-aa78-e7714d36ab2a
var sevCmdlineEntryGuid = guidLE{0xd8, 0x2d, 0xd0, 0x97, 0x20, 0xbd, 0x94, 0x4c, 0xaa, 0x78, 0xe7, 0x71, 0x4d, 0x36, 0xab, 0x2a}

type sevHashTableEntry struct {
	entryGuid guidLE
	length    uint16
	hash      [sha256.Size]byte
}

type sevHashTable struct {
	tableGuid guidLE
	length    uint16
	cmdline   sevHashTableEntry
	initrd    sevHashTableEntry
	kernel    sevHashTableEntry
}

type paddedSevHashTable struct {
	table   sevHashTable
	padding [8]byte
}

func fileSha256(filename string) (res [sha256.Size]byte, err error) {
	f, err := os.Open(filename)
	if err != nil {
		return res, err
	}
	defer f.Close()

	digest := sha256.New()
	if _, err := io.Copy(digest, f); err != nil {
		return res, err
	}

	copy(res[:], digest.Sum(nil))
	return res, nil
}

func constructSevHashesTable(kernelPath, initrdPath, cmdline string) ([]byte, error) {
	kernelHash, err := fileSha256(kernelPath)
	if err != nil {
		return []byte{}, err
	}

	initrdHash, err := fileSha256(initrdPath)
	if err != nil {
		return []byte{}, err
	}

	cmdlineHash := sha256.Sum256(append([]byte(cmdline), 0))

	buf := new(bytes.Buffer)
	err = binary.Write(buf, binary.LittleEndian, sevHashTableEntry{})
	if err != nil {
		return []byte{}, err
	}
	entrySize := uint16(buf.Len())

	buf = new(bytes.Buffer)
	err = binary.Write(buf, binary.LittleEndian, sevHashTable{})
	if err != nil {
		return []byte{}, err
	}
	tableSize := uint16(buf.Len())

	ht := paddedSevHashTable{
		table: sevHashTable{
			tableGuid: sevHashTableHeaderGuid,
			length:    tableSize,
			cmdline: sevHashTableEntry{
				entryGuid: sevCmdlineEntryGuid,
				length:    entrySize,
				hash:      cmdlineHash,
			},
			initrd: sevHashTableEntry{
				entryGuid: sevInitrdEntryGuid,
				length:    entrySize,
				hash:      initrdHash,
			},
			kernel: sevHashTableEntry{
				entryGuid: sevKernelEntryGuid,
				length:    entrySize,
				hash:      kernelHash,
			},
		},
		padding: [8]byte{0, 0, 0, 0, 0, 0, 0, 0},
	}

	htBuf := new(bytes.Buffer)
	err = binary.Write(htBuf, binary.LittleEndian, ht)
	if err != nil {
		return []byte{}, err
	}
	return htBuf.Bytes(), nil
}

// CalculateLaunchDigest returns the sha256 encoded SEV launch digest based off
// the current firmware, kernel, initrd, and the kernel cmdline
func CalculateLaunchDigest(firmwarePath, kernelPath, initrdPath, cmdline string) (res [sha256.Size]byte, err error) {
	f, err := os.Open(firmwarePath)
	if err != nil {
		return res, err
	}
	defer f.Close()

	digest := sha256.New()
	if _, err := io.Copy(digest, f); err != nil {
		return res, err
	}

	// When used for confidential containers in kata-containers, kernelPath
	// is always set (direct boot).  However, this current package can also
	// be used by other programs which may calculate launch digests of
	// arbitrary SEV guests without SEV kernel hashes table.
	if kernelPath != "" {
		ht, err := constructSevHashesTable(kernelPath, initrdPath, cmdline)
		if err != nil {
			return res, err
		}
		digest.Write(ht)
	}

	copy(res[:], digest.Sum(nil))
	return res, nil
}

// CalculateSEVESLaunchDigest returns the sha256 encoded SEV-ES launch digest
// based off the current firmware, kernel, initrd, and the kernel cmdline, and
// the number of vcpus and their type
func CalculateSEVESLaunchDigest(vcpus int, vcpuSig VCPUSig, firmwarePath, kernelPath, initrdPath, cmdline string) (res [sha256.Size]byte, err error) {
	f, err := os.Open(firmwarePath)
	if err != nil {
		return res, err
	}
	defer f.Close()

	digest := sha256.New()
	if _, err := io.Copy(digest, f); err != nil {
		return res, err
	}

	// When used for confidential containers in kata-containers, kernelPath
	// is always set (direct boot).  However, this current package can also
	// be used by other programs which may calculate launch digests of
	// arbitrary SEV guests without SEV kernel hashes table.
	if kernelPath != "" {
		ht, err := constructSevHashesTable(kernelPath, initrdPath, cmdline)
		if err != nil {
			return res, err
		}
		digest.Write(ht)
	}

	o, err := NewOvmf(firmwarePath)
	if err != nil {
		return res, err
	}
	resetEip, err := o.sevEsResetEip()
	if err != nil {
		return res, err
	}
	v := vmsaBuilder{uint64(resetEip), vcpuSig}
	for i := 0; i < vcpus; i++ {
		vmsaPage, err := v.buildPage(i)
		if err != nil {
			return res, err
		}
		digest.Write(vmsaPage)
	}

	copy(res[:], digest.Sum(nil))
	return res, nil
}
