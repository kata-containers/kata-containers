// Copyright (c) 2019 ARM Limited
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"os"
	"testing"

	"github.com/stretchr/testify/assert"
)

var dataFlagsFieldWithoutHypervisor = []byte(`
fpu_exception   : yes
cpuid level     : 20
wp              : yes
flags           : fpu vme de pse tsc msr pae mce cx8 apic sep mtrr pge mca cmov pat pse36 clflush mmx fxsr sse sse2 ss ht syscall nx pdpe1gb rdtscp lm constant_tsc rep_good nopl xtopology eagerfpu pni pclmulqdq vmx ssse3 fma cx16 sse4_1 sse4_2 movbe popcnt aes xsave avx f16c rdrand lahf_lm abm 3dnowprefetch tpr_shadow vnmi ept vpid fsgsbase bmi1 hle avx2 smep bmi2 erms rtm rdseed adx smap xsaveopt
bugs            :
bogomips        : 4589.35
`)

var dataFlagsFieldWithHypervisor = []byte(`
fpu_exception   : yes
cpuid level     : 20
wp              : yes
flags           : fpu vme de pse tsc msr pae mce cx8 apic sep mtrr pge mca cmov pat pse36 clflush mmx fxsr sse sse2 ss ht syscall nx pdpe1gb rdtscp lm constant_tsc rep_good nopl xtopology eagerfpu pni pclmulqdq vmx ssse3 fma cx16 sse4_1 sse4_2 movbe popcnt aes xsave avx f16c rdrand hypervisor lahf_lm abm 3dnowprefetch tpr_shadow vnmi ept vpid fsgsbase bmi1 hle avx2 smep bmi2 erms rtm rdseed adx smap xsaveopt
bugs            :
bogomips        : 4589.35
`)

var dataWithoutFlagsField = []byte(`
fpu_exception   : yes
cpuid level     : 20
wp              : yes
bugs            :
bogomips        : 4589.35
`)

func TestRunningOnVMM(t *testing.T) {
	var data []testNestedVMMData

	//file cpuinfo doesn't contain 'hypervisor' flag
	dataNestedVMMFalseSuccessful := testNestedVMMData{
		content:     dataFlagsFieldWithoutHypervisor,
		expectedErr: false,
		expected:    false,
	}
	data = append(data, dataNestedVMMFalseSuccessful)

	//file cpuinfo contains 'hypervisor' flag
	dataNestedVMMTrueSuccessful := testNestedVMMData{
		content:     dataFlagsFieldWithHypervisor,
		expectedErr: false,
		expected:    true,
	}
	data = append(data, dataNestedVMMTrueSuccessful)

	//file cpuinfo  doesn't contain field flags
	dataNestedVMMWithoutFlagsField := testNestedVMMData{
		content:     dataWithoutFlagsField,
		expectedErr: true,
		expected:    false,
	}
	data = append(data, dataNestedVMMWithoutFlagsField)

	genericTestRunningOnVMM(t, data)
}

func TestRunningOnVMMNotExistingCPUInfoPathFailure(t *testing.T) {
	f, err := os.CreateTemp("", "cpuinfo")
	assert.NoError(t, err)

	filePath := f.Name()

	f.Close()
	os.Remove(filePath)
	_, err = RunningOnVMM(filePath)
	assert.Error(t, err)
}
