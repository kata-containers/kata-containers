//
// Copyright (c) 2017 Intel Corporation
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
	"io/ioutil"
	"os"
	"reflect"
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestFileCopySuccessful(t *testing.T) {
	fileContent := "testContent"

	srcFile, err := ioutil.TempFile("", "test_src_copy")
	if err != nil {
		t.Fatal(err)
	}
	defer os.Remove(srcFile.Name())
	defer srcFile.Close()

	dstFile, err := ioutil.TempFile("", "test_dst_copy")
	if err != nil {
		t.Fatal(err)
	}
	defer os.Remove(dstFile.Name())

	dstPath := dstFile.Name()

	if err := dstFile.Close(); err != nil {
		t.Fatal(err)
	}

	if _, err := srcFile.WriteString(fileContent); err != nil {
		t.Fatal(err)
	}

	if err := fileCopy(srcFile.Name(), dstPath); err != nil {
		t.Fatal(err)
	}

	dstContent, err := ioutil.ReadFile(dstPath)
	if err != nil {
		t.Fatal(err)
	}

	if string(dstContent) != fileContent {
		t.Fatalf("Got %q\nExpecting %q", string(dstContent), fileContent)
	}

	srcInfo, err := srcFile.Stat()
	if err != nil {
		t.Fatal(err)
	}

	dstInfo, err := os.Stat(dstPath)
	if err != nil {
		t.Fatal(err)
	}

	if dstInfo.Mode() != srcInfo.Mode() {
		t.Fatalf("Got FileMode %d\nExpecting FileMode %d", dstInfo.Mode(), srcInfo.Mode())
	}

	if dstInfo.IsDir() != srcInfo.IsDir() {
		t.Fatalf("Got IsDir() = %t\nExpecting IsDir() = %t", dstInfo.IsDir(), srcInfo.IsDir())
	}

	if dstInfo.Size() != srcInfo.Size() {
		t.Fatalf("Got Size() = %d\nExpecting Size() = %d", dstInfo.Size(), srcInfo.Size())
	}
}

func TestFileCopySourceEmptyFailure(t *testing.T) {
	if err := fileCopy("", "testDst"); err == nil {
		t.Fatal("This test should fail because source path is empty")
	}
}

func TestFileCopyDestinationEmptyFailure(t *testing.T) {
	if err := fileCopy("testSrc", ""); err == nil {
		t.Fatal("This test should fail because destination path is empty")
	}
}

func TestFileCopySourceNotExistFailure(t *testing.T) {
	srcFile, err := ioutil.TempFile("", "test_src_copy")
	if err != nil {
		t.Fatal(err)
	}

	srcPath := srcFile.Name()

	if err := srcFile.Close(); err != nil {
		t.Fatal(err)
	}

	if err := os.Remove(srcPath); err != nil {
		t.Fatal(err)
	}

	if err := fileCopy(srcPath, "testDest"); err == nil {
		t.Fatal("This test should fail because source file does not exist")
	}
}

func TestGenerateRandomBytes(t *testing.T) {
	bytesNeeded := 8
	randBytes, err := generateRandomBytes(bytesNeeded)
	if err != nil {
		t.Fatal(err)
	}

	if len(randBytes) != bytesNeeded {
		t.Fatalf("Failed to generate %d random bytes", bytesNeeded)
	}
}

func TestRevereString(t *testing.T) {
	str := "Teststr"
	reversed := reverseString(str)

	if reversed != "rtstseT" {
		t.Fatal("Incorrect String Reversal")
	}
}

func TestWriteToFile(t *testing.T) {
	err := writeToFile("/file-does-not-exist", []byte("test-data"))
	assert.NotNil(t, err)

	tmpFile, err := ioutil.TempFile("", "test_append_file")
	assert.Nil(t, err)

	filename := tmpFile.Name()
	defer os.Remove(filename)

	tmpFile.Close()

	testData := []byte("test-data")
	err = writeToFile(filename, testData)
	assert.Nil(t, err)

	data, err := ioutil.ReadFile(filename)
	assert.Nil(t, err)

	assert.True(t, reflect.DeepEqual(testData, data))
}

func TestConstraintsToVCPUs(t *testing.T) {
	assert := assert.New(t)

	vcpus := ConstraintsToVCPUs(0, 100)
	assert.Zero(vcpus)

	vcpus = ConstraintsToVCPUs(100, 0)
	assert.Zero(vcpus)

	expectedVCPUs := uint(4)
	vcpus = ConstraintsToVCPUs(4000, 1000)
	assert.Equal(expectedVCPUs, vcpus)

	vcpus = ConstraintsToVCPUs(4000, 1200)
	assert.Equal(expectedVCPUs, vcpus)
}
