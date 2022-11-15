// Copyright contributors to AMD SEV/-ES in Go
//
// SPDX-License-Identifier: Apache-2.0

package sev

type VCPUSig uint64

const (
	// 'EPYC': family=23, model=1, stepping=2
	SigEpyc VCPUSig = 0x800f12

	// 'EPYC-v1': family=23, model=1, stepping=2
	SigEpycV1 VCPUSig = 0x800f12

	// 'EPYC-v2': family=23, model=1, stepping=2
	SigEpycV2 VCPUSig = 0x800f12

	// 'EPYC-IBPB': family=23, model=1, stepping=2
	SigEpycIBPB VCPUSig = 0x800f12

	// 'EPYC-v3': family=23, model=1, stepping=2
	SigEpycV3 VCPUSig = 0x800f12

	// 'EPYC-v4': family=23, model=1, stepping=2
	SigEpycV4 VCPUSig = 0x800f12

	// 'EPYC-Rome': family=23, model=49, stepping=0
	SigEpycRome VCPUSig = 0x830f10

	// 'EPYC-Rome-v1': family=23, model=49, stepping=0
	SigEpycRomeV1 VCPUSig = 0x830f10

	// 'EPYC-Rome-v2': family=23, model=49, stepping=0
	SigEpycRomeV2 VCPUSig = 0x830f10

	// 'EPYC-Rome-v3': family=23, model=49, stepping=0
	SigEpycRomeV3 VCPUSig = 0x830f10

	// 'EPYC-Milan': family=25, model=1, stepping=1
	SigEpycMilan VCPUSig = 0xa00f11

	// 'EPYC-Milan-v1': family=25, model=1, stepping=1
	SigEpycMilanV1 VCPUSig = 0xa00f11

	// 'EPYC-Milan-v2': family=25, model=1, stepping=1
	SigEpycMilanV2 VCPUSig = 0xa00f11
)

// NewVCPUSig computes the CPU signature (32-bit value) from the given family,
// model, and stepping.
//
// This computation is described in AMD's CPUID Specification, publication #25481
// https://www.amd.com/system/files/TechDocs/25481.pdf
// See section: CPUID Fn0000_0001_EAX Family, Model, Stepping Identifiers
func NewVCPUSig(family, model, stepping uint32) VCPUSig {
	var family_low, family_high uint32
	if family > 0xf {
		family_low = 0xf
		family_high = (family - 0x0f) & 0xff
	} else {
		family_low = family
		family_high = 0
	}

	model_low := model & 0xf
	model_high := (model >> 4) & 0xf

	stepping_low := stepping & 0xf

	return VCPUSig((family_high << 20) |
		(model_high << 16) |
		(family_low << 8) |
		(model_low << 4) |
		stepping_low)
}
