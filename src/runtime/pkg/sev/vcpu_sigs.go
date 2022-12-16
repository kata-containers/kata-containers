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
