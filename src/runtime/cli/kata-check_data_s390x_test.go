// Copyright (c) 2018 IBM
//
// SPDX-License-Identifier: Apache-2.0
//

package main

const testCPUInfoTemplate = `
vendor_id       : IBM/S390
# processors    : 4
bogomips per cpu: 20325.00
max thread id   : 0
features	: esan3 zarch stfle msa ldisp eimm dfp edat etf3eh highgprs te vx sie 
cache0          : level=1 type=Data scope=Private size=128K line_size=256 associativity=8
cache1          : level=1 type=Instruction scope=Private size=96K line_size=256 associativity=6
cache2          : level=2 type=Data scope=Private size=2048K line_size=256 associativity=8
cache3          : level=2 type=Instruction scope=Private size=2048K line_size=256 associativity=8
cache4          : level=3 type=Unified scope=Shared size=65536K line_size=256 associativity=16
cache5          : level=4 type=Unified scope=Shared size=491520K line_size=256 associativity=30
processor 0: version = FF,  identification = FFFFFF,  machine = 2964
`
