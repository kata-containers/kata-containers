//
// Copyright (c) 2016 Intel Corporation
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//

package virtcontainers

import (
	"fmt"
	"io/ioutil"
	"os"
	"path/filepath"
	"reflect"
	"testing"
)

func testSetHypervisorType(t *testing.T, value string, expected HypervisorType) {
	var hypervisorType HypervisorType

	err := (&hypervisorType).Set(value)
	if err != nil {
		t.Fatal(err)
	}

	if hypervisorType != expected {
		t.Fatal()
	}
}

func TestSetQemuHypervisorType(t *testing.T) {
	testSetHypervisorType(t, "qemu", QemuHypervisor)
}

func TestSetMockHypervisorType(t *testing.T) {
	testSetHypervisorType(t, "mock", MockHypervisor)
}

func TestSetUnknownHypervisorType(t *testing.T) {
	var hypervisorType HypervisorType

	err := (&hypervisorType).Set("unknown")
	if err == nil {
		t.Fatal()
	}

	if hypervisorType == QemuHypervisor ||
		hypervisorType == MockHypervisor {
		t.Fatal()
	}
}

func testStringFromHypervisorType(t *testing.T, hypervisorType HypervisorType, expected string) {
	hypervisorTypeStr := (&hypervisorType).String()
	if hypervisorTypeStr != expected {
		t.Fatal()
	}
}

func TestStringFromQemuHypervisorType(t *testing.T) {
	hypervisorType := QemuHypervisor
	testStringFromHypervisorType(t, hypervisorType, "qemu")
}

func TestStringFromMockHypervisorType(t *testing.T) {
	hypervisorType := MockHypervisor
	testStringFromHypervisorType(t, hypervisorType, "mock")
}

func TestStringFromUnknownHypervisorType(t *testing.T) {
	var hypervisorType HypervisorType
	testStringFromHypervisorType(t, hypervisorType, "")
}

func testNewHypervisorFromHypervisorType(t *testing.T, hypervisorType HypervisorType, expected hypervisor) {
	hy, err := newHypervisor(hypervisorType)
	if err != nil {
		t.Fatal(err)
	}

	if reflect.DeepEqual(hy, expected) == false {
		t.Fatal()
	}
}

func TestNewHypervisorFromQemuHypervisorType(t *testing.T) {
	hypervisorType := QemuHypervisor
	expectedHypervisor := &qemu{}
	testNewHypervisorFromHypervisorType(t, hypervisorType, expectedHypervisor)
}

func TestNewHypervisorFromMockHypervisorType(t *testing.T) {
	hypervisorType := MockHypervisor
	expectedHypervisor := &mockHypervisor{}
	testNewHypervisorFromHypervisorType(t, hypervisorType, expectedHypervisor)
}

func TestNewHypervisorFromUnknownHypervisorType(t *testing.T) {
	var hypervisorType HypervisorType

	hy, err := newHypervisor(hypervisorType)
	if err == nil {
		t.Fatal()
	}

	if hy != nil {
		t.Fatal()
	}
}

func testHypervisorConfigValid(t *testing.T, hypervisorConfig *HypervisorConfig, expected bool) {
	ret, _ := hypervisorConfig.valid()
	if ret != expected {
		t.Fatal()
	}
}

func TestHypervisorConfigNoKernelPath(t *testing.T) {
	hypervisorConfig := &HypervisorConfig{
		KernelPath:     "",
		ImagePath:      fmt.Sprintf("%s/%s", testDir, testImage),
		HypervisorPath: fmt.Sprintf("%s/%s", testDir, testHypervisor),
	}

	testHypervisorConfigValid(t, hypervisorConfig, false)
}

func TestHypervisorConfigNoImagePath(t *testing.T) {
	hypervisorConfig := &HypervisorConfig{
		KernelPath:     fmt.Sprintf("%s/%s", testDir, testKernel),
		ImagePath:      "",
		HypervisorPath: fmt.Sprintf("%s/%s", testDir, testHypervisor),
	}

	testHypervisorConfigValid(t, hypervisorConfig, false)
}

func TestHypervisorConfigNoHypervisorPath(t *testing.T) {
	hypervisorConfig := &HypervisorConfig{
		KernelPath:     fmt.Sprintf("%s/%s", testDir, testKernel),
		ImagePath:      fmt.Sprintf("%s/%s", testDir, testImage),
		HypervisorPath: "",
	}

	testHypervisorConfigValid(t, hypervisorConfig, true)
}

func TestHypervisorConfigIsValid(t *testing.T) {
	hypervisorConfig := &HypervisorConfig{
		KernelPath:     fmt.Sprintf("%s/%s", testDir, testKernel),
		ImagePath:      fmt.Sprintf("%s/%s", testDir, testImage),
		HypervisorPath: fmt.Sprintf("%s/%s", testDir, testHypervisor),
	}

	testHypervisorConfigValid(t, hypervisorConfig, true)
}

func TestHypervisorConfigDefaults(t *testing.T) {
	hypervisorConfig := &HypervisorConfig{
		KernelPath:     fmt.Sprintf("%s/%s", testDir, testKernel),
		ImagePath:      fmt.Sprintf("%s/%s", testDir, testImage),
		HypervisorPath: "",
	}
	testHypervisorConfigValid(t, hypervisorConfig, true)

	hypervisorConfigDefaultsExpected := &HypervisorConfig{
		KernelPath:        fmt.Sprintf("%s/%s", testDir, testKernel),
		ImagePath:         fmt.Sprintf("%s/%s", testDir, testImage),
		HypervisorPath:    "",
		DefaultVCPUs:      defaultVCPUs,
		DefaultMemSz:      defaultMemSzMiB,
		DefaultBridges:    defaultBridges,
		BlockDeviceDriver: defaultBlockDriver,
		DefaultMaxVCPUs:   defaultMaxQemuVCPUs,
	}
	if reflect.DeepEqual(hypervisorConfig, hypervisorConfigDefaultsExpected) == false {
		t.Fatal()
	}
}

func TestAppendParams(t *testing.T) {
	paramList := []Param{
		{
			Key:   "param1",
			Value: "value1",
		},
	}

	expectedParams := []Param{
		{
			Key:   "param1",
			Value: "value1",
		},
		{
			Key:   "param2",
			Value: "value2",
		},
	}

	paramList = appendParam(paramList, "param2", "value2")
	if reflect.DeepEqual(paramList, expectedParams) == false {
		t.Fatal()
	}
}

func testSerializeParams(t *testing.T, params []Param, delim string, expected []string) {
	result := SerializeParams(params, delim)
	if reflect.DeepEqual(result, expected) == false {
		t.Fatal()
	}
}

func TestSerializeParamsNoParamNoValue(t *testing.T) {
	params := []Param{
		{
			Key:   "",
			Value: "",
		},
	}
	var expected []string

	testSerializeParams(t, params, "", expected)
}

func TestSerializeParamsNoParam(t *testing.T) {
	params := []Param{
		{
			Value: "value1",
		},
	}

	expected := []string{"value1"}

	testSerializeParams(t, params, "", expected)
}

func TestSerializeParamsNoValue(t *testing.T) {
	params := []Param{
		{
			Key: "param1",
		},
	}

	expected := []string{"param1"}

	testSerializeParams(t, params, "", expected)
}

func TestSerializeParamsNoDelim(t *testing.T) {
	params := []Param{
		{
			Key:   "param1",
			Value: "value1",
		},
	}

	expected := []string{"param1", "value1"}

	testSerializeParams(t, params, "", expected)
}

func TestSerializeParams(t *testing.T) {
	params := []Param{
		{
			Key:   "param1",
			Value: "value1",
		},
	}

	expected := []string{"param1=value1"}

	testSerializeParams(t, params, "=", expected)
}

func testDeserializeParams(t *testing.T, parameters []string, expected []Param) {
	result := DeserializeParams(parameters)
	if reflect.DeepEqual(result, expected) == false {
		t.Fatal()
	}
}

func TestDeserializeParamsNil(t *testing.T) {
	var parameters []string
	var expected []Param

	testDeserializeParams(t, parameters, expected)
}

func TestDeserializeParamsNoParamNoValue(t *testing.T) {
	parameters := []string{
		"",
	}

	var expected []Param

	testDeserializeParams(t, parameters, expected)
}

func TestDeserializeParamsNoValue(t *testing.T) {
	parameters := []string{
		"param1",
	}
	expected := []Param{
		{
			Key: "param1",
		},
	}

	testDeserializeParams(t, parameters, expected)
}

func TestDeserializeParams(t *testing.T) {
	parameters := []string{
		"param1=value1",
	}

	expected := []Param{
		{
			Key:   "param1",
			Value: "value1",
		},
	}

	testDeserializeParams(t, parameters, expected)
}

func TestAddKernelParamValid(t *testing.T) {
	var config HypervisorConfig

	expected := []Param{
		{"foo", "bar"},
	}

	err := config.AddKernelParam(expected[0])
	if err != nil || reflect.DeepEqual(config.KernelParams, expected) == false {
		t.Fatal()
	}
}

func TestAddKernelParamInvalid(t *testing.T) {
	var config HypervisorConfig

	invalid := []Param{
		{"", "bar"},
	}

	err := config.AddKernelParam(invalid[0])
	if err == nil {
		t.Fatal()
	}
}

func TestGetHostMemorySizeKb(t *testing.T) {

	type testData struct {
		contents       string
		expectedResult int
		expectError    bool
	}

	data := []testData{
		{
			`
			MemTotal:      1 kB
			MemFree:       2 kB
			SwapTotal:     3 kB
			SwapFree:      4 kB
			`,
			1024,
			false,
		},
		{
			`
			MemFree:       2 kB
			SwapTotal:     3 kB
			SwapFree:      4 kB
			`,
			0,
			true,
		},
	}

	dir, err := ioutil.TempDir("", "")
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(dir)

	file := filepath.Join(dir, "meminfo")
	if _, err := getHostMemorySizeKb(file); err == nil {
		t.Fatalf("expected failure as file %q does not exist", file)
	}

	for _, d := range data {
		if err := ioutil.WriteFile(file, []byte(d.contents), os.FileMode(0640)); err != nil {
			t.Fatal(err)
		}
		defer os.Remove(file)

		hostMemKb, err := getHostMemorySizeKb(file)

		if (d.expectError && err == nil) || (!d.expectError && err != nil) {
			t.Fatalf("got %d, input %v", hostMemKb, d)
		}

		if reflect.DeepEqual(hostMemKb, d.expectedResult) {
			t.Fatalf("got %d, input %v", hostMemKb, d)
		}
	}
}

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

func testRunningOnVMMSuccessful(t *testing.T, cpuInfoContent []byte, expectedErr bool, expected bool) {
	f, err := ioutil.TempFile("", "cpuinfo")
	if err != nil {
		t.Fatal(err)
	}
	defer os.Remove(f.Name())
	defer f.Close()

	n, err := f.Write(cpuInfoContent)
	if err != nil {
		t.Fatal(err)
	}
	if n != len(cpuInfoContent) {
		t.Fatalf("Only %d bytes written out of %d expected", n, len(cpuInfoContent))
	}

	running, err := RunningOnVMM(f.Name())
	if !expectedErr && err != nil {
		t.Fatalf("This test should succeed: %v", err)
	} else if expectedErr && err == nil {
		t.Fatalf("This test should fail")
	}

	if running != expected {
		t.Fatalf("Expecting running on VMM = %t, Got %t", expected, running)
	}
}

func TestRunningOnVMMFalseSuccessful(t *testing.T) {
	testRunningOnVMMSuccessful(t, dataFlagsFieldWithoutHypervisor, false, false)
}

func TestRunningOnVMMTrueSuccessful(t *testing.T) {
	testRunningOnVMMSuccessful(t, dataFlagsFieldWithHypervisor, false, true)
}

func TestRunningOnVMMNoFlagsFieldFailure(t *testing.T) {
	testRunningOnVMMSuccessful(t, dataWithoutFlagsField, true, false)
}

func TestRunningOnVMMNotExistingCPUInfoPathFailure(t *testing.T) {
	f, err := ioutil.TempFile("", "cpuinfo")
	if err != nil {
		t.Fatal(err)
	}

	filePath := f.Name()

	f.Close()
	os.Remove(filePath)

	if _, err := RunningOnVMM(filePath); err == nil {
		t.Fatalf("Should fail because %q file path does not exist", filePath)
	}
}
