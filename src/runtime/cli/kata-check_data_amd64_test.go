// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

const testCPUInfoTemplate = `
processor	: 0
vendor_id	: {{.VendorID}}
cpu family	: 6
model		: 61
model name	: Intel(R) Core(TM) i7-5600U CPU @ 2.60GHz
stepping	: 4
microcode	: 0x25
cpu MHz		: 1999.987
cache size	: 4096 KB
physical id	: 0
siblings	: 4
core id		: 0
cpu cores	: 2
apicid		: 0
initial apicid	: 0
fpu		: yes
fpu_exception	: yes
cpuid level	: 20
wp		: yes
flags		: {{.Flags}}
bugs		:
bogomips	: 5188.36
clflush size	: 64
cache_alignment	: 64
address sizes	: 39 bits physical, 48 bits virtual
power management:

processor	: 1
vendor_id	: {{.VendorID}}
cpu family	: 6
model		: 61
model name	: Intel(R) Core(TM) i7-5600U CPU @ 2.60GHz
stepping	: 4
microcode	: 0x25
cpu MHz		: 1999.987
cache size	: 4096 KB
physical id	: 0
siblings	: 4
core id		: 0
cpu cores	: 2
apicid		: 1
initial apicid	: 1
fpu		: yes
fpu_exception	: yes
cpuid level	: 20
wp		: yes
flags		: {{.Flags}}
bugs		:
bogomips	: 5194.90
clflush size	: 64
cache_alignment	: 64
address sizes	: 39 bits physical, 48 bits virtual
power management:

`
