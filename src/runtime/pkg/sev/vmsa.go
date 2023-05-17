// Copyright contributors to AMD SEV/-ES in Go
//
// SPDX-License-Identifier: Apache-2.0

package sev

import (
	"bytes"
	"encoding/binary"
)

// VMCB Segment (struct vmcb_seg in the linux kernel)
type vmcbSeg struct {
	selector uint16
	attrib   uint16
	limit    uint32
	base     uint64
}

// VMSA page
//
// The names of the fields are taken from struct sev_es_work_area in the linux kernel:
// https://github.com/AMDESE/linux/blob/sev-snp-v12/arch/x86/include/asm/svm.h#L318
// (following the definitions in AMD APM Vol 2 Table B-4)
type sevEsSaveArea struct {
	es                  vmcbSeg
	cs                  vmcbSeg
	ss                  vmcbSeg
	ds                  vmcbSeg
	fs                  vmcbSeg
	gs                  vmcbSeg
	gdtr                vmcbSeg
	ldtr                vmcbSeg
	idtr                vmcbSeg
	tr                  vmcbSeg
	vmpl0_ssp           uint64   // nolint: unused
	vmpl1_ssp           uint64   // nolint: unused
	vmpl2_ssp           uint64   // nolint: unused
	vmpl3_ssp           uint64   // nolint: unused
	u_cet               uint64   // nolint: unused
	reserved_1          [2]uint8 // nolint: unused
	vmpl                uint8    // nolint: unused
	cpl                 uint8    // nolint: unused
	reserved_2          [4]uint8 // nolint: unused
	efer                uint64
	reserved_3          [104]uint8 // nolint: unused
	xss                 uint64     // nolint: unused
	cr4                 uint64
	cr3                 uint64 // nolint: unused
	cr0                 uint64
	dr7                 uint64
	dr6                 uint64
	rflags              uint64
	rip                 uint64
	dr0                 uint64    // nolint: unused
	dr1                 uint64    // nolint: unused
	dr2                 uint64    // nolint: unused
	dr3                 uint64    // nolint: unused
	dr0_addr_mask       uint64    // nolint: unused
	dr1_addr_mask       uint64    // nolint: unused
	dr2_addr_mask       uint64    // nolint: unused
	dr3_addr_mask       uint64    // nolint: unused
	reserved_4          [24]uint8 // nolint: unused
	rsp                 uint64    // nolint: unused
	s_cet               uint64    // nolint: unused
	ssp                 uint64    // nolint: unused
	isst_addr           uint64    // nolint: unused
	rax                 uint64    // nolint: unused
	star                uint64    // nolint: unused
	lstar               uint64    // nolint: unused
	cstar               uint64    // nolint: unused
	sfmask              uint64    // nolint: unused
	kernel_gs_base      uint64    // nolint: unused
	sysenter_cs         uint64    // nolint: unused
	sysenter_esp        uint64    // nolint: unused
	sysenter_eip        uint64    // nolint: unused
	cr2                 uint64    // nolint: unused
	reserved_5          [32]uint8 // nolint: unused
	g_pat               uint64
	dbgctrl             uint64    // nolint: unused
	br_from             uint64    // nolint: unused
	br_to               uint64    // nolint: unused
	last_excp_from      uint64    // nolint: unused
	last_excp_to        uint64    // nolint: unused
	reserved_7          [80]uint8 // nolint: unused
	pkru                uint32    // nolint: unused
	reserved_8          [20]uint8 // nolint: unused
	reserved_9          uint64    // nolint: unused
	rcx                 uint64    // nolint: unused
	rdx                 uint64
	rbx                 uint64    // nolint: unused
	reserved_10         uint64    // nolint: unused
	rbp                 uint64    // nolint: unused
	rsi                 uint64    // nolint: unused
	rdi                 uint64    // nolint: unused
	r8                  uint64    // nolint: unused
	r9                  uint64    // nolint: unused
	r10                 uint64    // nolint: unused
	r11                 uint64    // nolint: unused
	r12                 uint64    // nolint: unused
	r13                 uint64    // nolint: unused
	r14                 uint64    // nolint: unused
	r15                 uint64    // nolint: unused
	reserved_11         [16]uint8 // nolint: unused
	guest_exit_info_1   uint64    // nolint: unused
	guest_exit_info_2   uint64    // nolint: unused
	guest_exit_int_info uint64    // nolint: unused
	guest_nrip          uint64    // nolint: unused
	sev_features        uint64
	vintr_ctrl          uint64 // nolint: unused
	guest_exit_code     uint64 // nolint: unused
	virtual_tom         uint64 // nolint: unused
	tlb_id              uint64 // nolint: unused
	pcpu_id             uint64 // nolint: unused
	event_inj           uint64 // nolint: unused
	xcr0                uint64
	reserved_12         [16]uint8   // nolint: unused
	x87_dp              uint64      // nolint: unused
	mxcsr               uint32      // nolint: unused
	x87_ftw             uint16      // nolint: unused
	x87_fsw             uint16      // nolint: unused
	x87_fcw             uint16      // nolint: unused
	x87_fop             uint16      // nolint: unused
	x87_ds              uint16      // nolint: unused
	x87_cs              uint16      // nolint: unused
	x87_rip             uint64      // nolint: unused
	fpreg_x87           [80]uint8   // nolint: unused
	fpreg_xmm           [256]uint8  // nolint: unused
	fpreg_ymm           [256]uint8  // nolint: unused
	unused              [2448]uint8 // nolint: unused
}

type vmsaBuilder struct {
	apEIP   uint64
	vcpuSig VCPUSig
}

func (v *vmsaBuilder) buildPage(i int) ([]byte, error) {
	eip := uint64(0xfffffff0) // BSP (first vcpu)
	if i > 0 {
		eip = v.apEIP
	}
	saveArea := sevEsSaveArea{
		es:           vmcbSeg{0, 0x93, 0xffff, 0},
		cs:           vmcbSeg{0xf000, 0x9b, 0xffff, eip & 0xffff0000},
		ss:           vmcbSeg{0, 0x93, 0xffff, 0},
		ds:           vmcbSeg{0, 0x93, 0xffff, 0},
		fs:           vmcbSeg{0, 0x93, 0xffff, 0},
		gs:           vmcbSeg{0, 0x93, 0xffff, 0},
		gdtr:         vmcbSeg{0, 0, 0xffff, 0},
		idtr:         vmcbSeg{0, 0, 0xffff, 0},
		ldtr:         vmcbSeg{0, 0x82, 0xffff, 0},
		tr:           vmcbSeg{0, 0x8b, 0xffff, 0},
		efer:         0x1000, // KVM enables EFER_SVME
		cr4:          0x40,   // KVM enables X86_CR4_MCE
		cr0:          0x10,
		dr7:          0x400,
		dr6:          0xffff0ff0,
		rflags:       0x2,
		rip:          eip & 0xffff,
		g_pat:        0x7040600070406, // PAT MSR: See AMD APM Vol 2, Section A.3
		rdx:          uint64(v.vcpuSig),
		sev_features: 0, // SEV-ES
		xcr0:         0x1,
	}
	page := new(bytes.Buffer)
	err := binary.Write(page, binary.LittleEndian, saveArea)
	if err != nil {
		return []byte{}, err
	}
	return page.Bytes(), nil
}
