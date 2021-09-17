// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"
	"os"
	"strings"
	"testing"

	"github.com/blang/semver"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils"
	"github.com/stretchr/testify/assert"
)

var currentSemver semver.Version
var expectedReleasesURL string

func init() {
	var err error
	currentSemver, err = semver.Make(katautils.VERSION)

	if err != nil {
		panic(fmt.Sprintf("failed to create semver for testing: %v", err))
	}

	if currentSemver.Major == 1 {
		expectedReleasesURL = kataLegacyReleaseURL
	} else {
		expectedReleasesURL = kataReleaseURL
	}
}

func TestReleaseCmd(t *testing.T) {
	assert := assert.New(t)

	for i, value := range []ReleaseCmd{RelCmdCheck, RelCmdList} {
		assert.True(value.Valid(), "test[%d]: %+v", i, value)
	}

	for i, value := range []int{-1, 2, 42, 255} {
		invalid := ReleaseCmd(i)

		assert.False(invalid.Valid(), "test[%d]: %+v", i, value)
	}
}

func TestGetReleaseURL(t *testing.T) {
	assert := assert.New(t)

	const kata1xURL = "https://api.github.com/repos/kata-containers/runtime/releases"
	const kata2xURL = "https://api.github.com/repos/kata-containers/kata-containers/releases"

	type testData struct {
		currentVersion string
		expectedURL    string
		expectError    bool
	}

	data := []testData{
		{"0.0.0", "", true},
		{"1.0.0", kata1xURL, false},
		{"1.9999.9999", kata1xURL, false},
		{"2.0.0-alpha3", kata2xURL, false},
		{"2.9999.9999", kata2xURL, false},
	}

	for i, d := range data {
		msg := fmt.Sprintf("test[%d]: %+v", i, d)

		ver, err := semver.Make(d.currentVersion)
		msg = fmt.Sprintf("%s, version: %v, error: %v", msg, ver, err)

		assert.NoError(err, msg)

		url, err := getReleaseURL(ver)
		if d.expectError {
			assert.Error(err, msg)
		} else {
			assert.NoError(err, msg)
			assert.Equal(url, d.expectedURL, msg)
			assert.True(strings.HasPrefix(url, projectAPIURL), msg)
		}
	}

	url, err := getReleaseURL(currentSemver)
	assert.NoError(err)

	assert.Equal(url, expectedReleasesURL)

	assert.True(strings.HasPrefix(url, projectAPIURL))

}

func TestGetReleaseURLEnvVar(t *testing.T) {
	assert := assert.New(t)

	type testData struct {
		envVarValue string
		expectedURL string
		expectError bool
	}

	data := []testData{
		{"", expectedReleasesURL, false},
		{"http://google.com", "", true},
		{"https://katacontainers.io", "", true},
		{"https://github.com/kata-containers/runtime/releases/latest", "", true},
		{"https://github.com/kata-containers/kata-containers/releases/latest", "", true},
		{expectedReleasesURL, expectedReleasesURL, false},
	}

	assert.Equal(os.Getenv("KATA_RELEASE_URL"), "")
	defer os.Setenv("KATA_RELEASE_URL", "")

	for i, d := range data {
		msg := fmt.Sprintf("test[%d]: %+v", i, d)

		err := os.Setenv("KATA_RELEASE_URL", d.envVarValue)
		msg = fmt.Sprintf("%s, error: %v", msg, err)

		assert.NoError(err, msg)

		url, err := getReleaseURL(currentSemver)
		if d.expectError {
			assert.Errorf(err, msg)
		} else {
			assert.NoErrorf(err, msg)
			assert.Equal(d.expectedURL, url, msg)
		}
	}
}

func TestMakeRelease(t *testing.T) {
	assert := assert.New(t)

	type testData struct {
		release         map[string]interface{}
		expectedVersion string
		expectedDetails releaseDetails
		expectError     bool
	}

	invalidRel1 := map[string]interface{}{"foo": 1}
	invalidRel2 := map[string]interface{}{"foo": "bar"}
	invalidRel3 := map[string]interface{}{"foo": true}

	testDate := "2020-09-01T22:10:44Z"
	testRelVersion := "1.2.3"
	testFilename := "kata-static-1.12.0-alpha1-x86_64.tar.xz"
	testURL := fmt.Sprintf("https://github.com/kata-containers/runtime/releases/download/%s/%s", testRelVersion, testFilename)

	testSemver, err := semver.Make(testRelVersion)
	assert.NoError(err)

	invalidRelMissingVersion := map[string]interface{}{}

	invalidRelInvalidVersion := map[string]interface{}{
		"tag_name": "not.valid.semver",
	}

	invalidRelMissingAssets := map[string]interface{}{
		"tag_name": testRelVersion,
		"name":     testFilename,
		"assets":   []interface{}{},
	}

	invalidAssetsMissingURL := []interface{}{
		map[string]interface{}{
			"name":       testFilename,
			"created_at": testDate,
		},
	}

	invalidAssetsMissingFile := []interface{}{
		map[string]interface{}{
			"browser_download_url": testURL,
			"created_at":           testDate,
		},
	}

	invalidAssetsMissingDate := []interface{}{
		map[string]interface{}{
			"name":                 testFilename,
			"browser_download_url": testURL,
		},
	}

	validAssets := []interface{}{
		map[string]interface{}{
			"browser_download_url": testURL,
			"name":                 testFilename,
			"created_at":           testDate,
		},
	}

	invalidRelAssetsMissingURL := map[string]interface{}{
		"tag_name": testRelVersion,
		"name":     testFilename,
		"assets":   invalidAssetsMissingURL,
	}

	invalidRelAssetsMissingFile := map[string]interface{}{
		"tag_name": testRelVersion,
		"name":     testFilename,
		"assets":   invalidAssetsMissingFile,
	}

	invalidRelAssetsMissingDate := map[string]interface{}{
		"tag_name": testRelVersion,
		"name":     testFilename,
		"assets":   invalidAssetsMissingDate,
	}

	validRel := map[string]interface{}{
		"tag_name": testRelVersion,
		"name":     testFilename,
		"assets":   validAssets,
	}

	validReleaseDetails := releaseDetails{
		version:  testSemver,
		date:     testDate,
		url:      testURL,
		filename: testFilename,
	}

	data := []testData{
		{invalidRel1, "", releaseDetails{}, true},
		{invalidRel2, "", releaseDetails{}, true},
		{invalidRel3, "", releaseDetails{}, true},
		{invalidRelMissingVersion, "", releaseDetails{}, true},
		{invalidRelInvalidVersion, "", releaseDetails{}, true},
		{invalidRelMissingAssets, "", releaseDetails{}, true},
		{invalidRelAssetsMissingURL, "", releaseDetails{}, true},
		{invalidRelAssetsMissingFile, "", releaseDetails{}, true},
		{invalidRelAssetsMissingDate, "", releaseDetails{}, true},

		{validRel, testRelVersion, validReleaseDetails, false},
	}

	for i, d := range data {
		msg := fmt.Sprintf("test[%d]: %+v", i, d)

		version, details, err := makeRelease(d.release)
		msg = fmt.Sprintf("%s, version: %v, details: %+v, error: %v", msg, version, details, err)

		if d.expectError {
			assert.Error(err, msg)
			continue
		}

		assert.NoError(err, msg)
		assert.Equal(d.expectedVersion, version, msg)
		assert.Equal(d.expectedDetails, details, msg)
	}
}

func TestReleaseURLIsValid(t *testing.T) {
	assert := assert.New(t)

	type testData struct {
		url         string
		expectError bool
	}

	data := []testData{
		{"", true},
		{"foo", true},
		{"foo bar", true},
		{"https://google.com", true},
		{projectAPIURL, true},

		{kataLegacyReleaseURL, false},
		{kataReleaseURL, false},
	}

	for i, d := range data {
		msg := fmt.Sprintf("test[%d]: %+v", i, d)

		err := releaseURLIsValid(d.url)
		msg = fmt.Sprintf("%s, error: %v", msg, err)

		if d.expectError {
			assert.Error(err, msg)
		} else {
			assert.NoError(err, msg)
		}
	}
}

func TestDownloadURLIsValid(t *testing.T) {
	assert := assert.New(t)

	type testData struct {
		url         string
		expectError bool
	}

	validKata1xDownload := "https://github.com/kata-containers/runtime/releases/download/1.12.0-alpha1/kata-static-1.12.0-alpha1-x86_64.tar.xz"
	validKata2xDownload := "https://github.com/kata-containers/kata-containers/releases/download/2.0.0-alpha3/kata-static-2.0.0-alpha3-x86_64.tar.xz"

	data := []testData{
		{"", true},
		{"foo", true},
		{"foo bar", true},
		{"https://google.com", true},
		{katautils.PROJECTURL, true},
		{validKata1xDownload, false},
		{validKata2xDownload, false},
	}

	for i, d := range data {
		msg := fmt.Sprintf("test[%d]: %+v", i, d)

		err := downloadURLIsValid(d.url)
		msg = fmt.Sprintf("%s, error: %v", msg, err)

		if d.expectError {
			assert.Error(err, msg)
		} else {
			assert.NoError(err, msg)
		}
	}
}

func TestIgnoreRelease(t *testing.T) {
	assert := assert.New(t)

	type testData struct {
		details      releaseDetails
		includeAll   bool
		expectIgnore bool
	}

	verWithoutPreRelease, err := semver.Make("2.0.0")
	assert.NoError(err)

	verWithPreRelease, err := semver.Make("2.0.0-alpha3")
	assert.NoError(err)

	relWithoutPreRelease := releaseDetails{
		version: verWithoutPreRelease,
	}

	relWithPreRelease := releaseDetails{
		version: verWithPreRelease,
	}

	data := []testData{
		{relWithoutPreRelease, false, false},
		{relWithoutPreRelease, true, false},
		{relWithPreRelease, false, true},
		{relWithPreRelease, true, false},
	}

	for i, d := range data {
		msg := fmt.Sprintf("test[%d]: %+v", i, d)

		ignore := ignoreRelease(d.details, d.includeAll)

		if d.expectIgnore {
			assert.True(ignore, msg)
		} else {
			assert.False(ignore, msg)
		}
	}
}

func TestGetReleases(t *testing.T) {
	assert := assert.New(t)

	url := "foo"
	expectedErrMsg := "unsupported protocol scheme"

	for _, includeAll := range []bool{true, false} {
		euid := os.Geteuid()

		msg := fmt.Sprintf("includeAll: %v, euid: %v", includeAll, euid)

		_, _, err := getReleases(url, includeAll)
		msg = fmt.Sprintf("%s, error: %v", msg, err)

		assert.Error(err, msg)

		if euid == 0 {
			assert.Equal(err.Error(), errNoNetChecksAsRoot, msg)
		} else {
			assert.True(strings.Contains(err.Error(), expectedErrMsg), msg)
		}
	}
}

func TestFindNewestRelease(t *testing.T) {
	assert := assert.New(t)

	type testData struct {
		versions        []semver.Version
		currentVer      semver.Version
		expectVersion   semver.Version
		expectError     bool
		expectAvailable bool
	}

	ver1, err := semver.Make("1.11.1")
	assert.NoError(err)

	ver2, err := semver.Make("1.11.3")
	assert.NoError(err)

	ver3, err := semver.Make("2.0.0")
	assert.NoError(err)

	data := []testData{
		{[]semver.Version{}, semver.Version{}, semver.Version{}, true, false},
		{[]semver.Version{}, ver1, semver.Version{}, true, false},
		{[]semver.Version{ver1}, ver1, semver.Version{}, false, false},
		{[]semver.Version{ver1}, ver2, semver.Version{}, false, false},
		{[]semver.Version{ver2}, ver1, ver2, false, true},
		{[]semver.Version{ver3}, ver1, ver3, false, true},
		{[]semver.Version{ver2, ver3}, ver1, ver3, false, true},
		{[]semver.Version{ver1, ver3}, ver2, ver3, false, true},
		{[]semver.Version{ver1}, ver2, semver.Version{}, false, false},
	}

	for i, d := range data {
		msg := fmt.Sprintf("test[%d]: %+v", i, d)

		available, version, err := findNewestRelease(d.currentVer, d.versions)
		msg = fmt.Sprintf("%s, available: %v, version: %v, error: %v", msg, available, version, err)

		if d.expectError {
			assert.Error(err, msg)
			continue
		}

		assert.NoError(err, msg)

		if !d.expectAvailable {
			assert.False(available, msg)
			continue
		}

		assert.Equal(d.expectVersion, version)
	}
}

func TestGetNewReleaseType(t *testing.T) {
	assert := assert.New(t)

	type testData struct {
		currentVer  string
		latestVer   string
		result      string
		expectError bool
	}

	data := []testData{
		// Check build metadata (ignored for version comparisons)
		{"2.0.0+build", "2.0.0", "", true},
		{"2.0.0+build-1", "2.0.0+build-2", "", true},
		{"1.12.0+build", "1.12.0", "", true},

		{"2.0.0-rc3+foo", "2.0.0", "major", false},
		{"2.0.0-rc3+foo", "2.0.0-rc4", "pre-release", false},
		{"1.12.0+foo", "1.13.0", "minor", false},

		{"1.12.0+build", "2.0.0", "major", false},
		{"1.12.0+build", "1.13.0", "minor", false},
		{"1.12.0-rc2+build", "1.12.1", "patch", false},
		{"1.12.0-rc2+build", "1.12.1-foo", "patch pre-release", false},
		{"1.12.0-rc4+wibble", "1.12.0", "major", false},

		{"2.0.0-alpha3", "1.0.0", "", true},
		{"1.0.0", "1.0.0", "", true},
		{"2.0.0", "1.0.0", "", true},

		{"1.0.0", "2.0.0", "major", false},
		{"2.0.0-alpha3", "2.0.0-alpha4", "pre-release", false},
		{"1.0.0", "2.0.0-alpha3", "major pre-release", false},

		{"1.0.0", "1.1.2", "minor", false},
		{"1.0.0", "1.1.2-pre2", "minor pre-release", false},
		{"1.0.0", "1.1.2-foo", "minor pre-release", false},

		{"1.0.0", "1.0.3", "patch", false},
		{"1.0.0-beta29", "1.0.0-beta30", "pre-release", false},
		{"1.0.0", "1.0.3-alpha99.1b", "patch pre-release", false},

		{"2.0.0-rc0", "2.0.0", "major", false},
		{"2.0.0-rc1", "2.0.0", "major", false},

		{"1.12.0-rc0", "1.12.0", "major", false},
		{"1.12.0-rc5", "1.12.0", "major", false},
	}

	for i, d := range data {
		msg := fmt.Sprintf("test[%d]: %+v", i, d)

		current, err := semver.Make(d.currentVer)
		msg = fmt.Sprintf("%s, current: %v, error: %v", msg, current, err)

		assert.NoError(err, msg)

		latest, err := semver.Make(d.latestVer)
		assert.NoError(err, msg)

		desc, err := getNewReleaseType(current, latest)
		if d.expectError {
			assert.Error(err, msg)
			continue
		}

		assert.NoError(err, msg)
		assert.Equal(d.result, desc, msg)
	}
}
