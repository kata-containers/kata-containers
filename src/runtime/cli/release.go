// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"io/ioutil"
	"net/http"
	"os"
	"strings"

	"github.com/blang/semver"
)

type ReleaseCmd int

type releaseDetails struct {
	version  semver.Version
	date     string
	url      string
	filename string
}

const (
	// A release URL is expected to be prefixed with this value
	projectAPIURL = "https://api.github.com/repos/" + projectORG

	releasesSuffix  = "/releases"
	downloadsSuffix = releasesSuffix + "/download"

	// Kata 1.x
	kata1xRepo            = "runtime"
	kataLegacyReleaseURL  = projectAPIURL + "/" + kata1xRepo + releasesSuffix
	kataLegacyDownloadURL = projectURL + "/" + kata1xRepo + downloadsSuffix

	// Kata 2.x or newer
	kata2xRepo      = "kata-containers"
	kataReleaseURL  = projectAPIURL + "/" + kata2xRepo + releasesSuffix
	kataDownloadURL = projectURL + "/" + kata2xRepo + downloadsSuffix

	// Environment variable that can be used to override a release URL
	ReleaseURLEnvVar = "KATA_RELEASE_URL"

	RelCmdList  ReleaseCmd = iota
	RelCmdCheck ReleaseCmd = iota

	msgNoReleases        = "No releases available"
	msgNoNewerRelease    = "No newer release available"
	errNoNetChecksAsRoot = "No network checks allowed running as super user"
)

func (c ReleaseCmd) Valid() bool {
	switch c {
	case RelCmdCheck, RelCmdList:
		return true
	default:
		return false
	}
}

func downloadURLIsValid(url string) error {
	if url == "" {
		return errors.New("URL cannot be blank")
	}

	if strings.HasPrefix(url, kataDownloadURL) ||
		strings.HasPrefix(url, kataLegacyDownloadURL) {
		return nil
	}

	return fmt.Errorf("Download URL %q is not valid", url)
}

func releaseURLIsValid(url string) error {
	if url == "" {
		return errors.New("URL cannot be blank")
	}

	if url == kataReleaseURL || url == kataLegacyReleaseURL {
		return nil
	}

	return fmt.Errorf("Release URL %q is not valid", url)
}

func getReleaseURL(currentVersion semver.Version) (url string, err error) {
	major := currentVersion.Major

	if major == 0 {
		return "", fmt.Errorf("invalid current version: %v", currentVersion)
	} else if major == 1 {
		url = kataLegacyReleaseURL
	} else {
		url = kataReleaseURL
	}

	if value := os.Getenv(ReleaseURLEnvVar); value != "" {
		url = value
	}

	if err := releaseURLIsValid(url); err != nil {
		return "", err
	}

	return url, nil
}

func ignoreRelease(release releaseDetails, includeAll bool) bool {
	if includeAll {
		return false
	}

	if len(release.version.Pre) > 0 {
		// Pre-releases are ignored by default
		return true
	}

	return false
}

// Returns a release version and release object from the specified map.
func makeRelease(release map[string]interface{}) (version string, details releaseDetails, err error) {
	key := "tag_name"

	version, ok := release[key].(string)
	if !ok {
		return "", details, fmt.Errorf("failed to find key %s in release data", key)
	}

	if version == "" {
		return "", details, fmt.Errorf("release version cannot be blank")
	}

	releaseSemver, err := semver.Make(version)
	if err != nil {
		return "", details, fmt.Errorf("release %q has invalid semver version: %v", version, err)
	}

	key = "assets"

	assetsArray, ok := release[key].([]interface{})
	if !ok {
		return "", details, fmt.Errorf("failed to find key %s in release version %q data", key, version)
	}

	if len(assetsArray) == 0 {
		// GitHub auto-creates the source assets, but binaries have to
		// be built and uploaded for a release.
		return "", details, fmt.Errorf("no binary assets for release %q", version)
	}

	var createDate string
	var filename string
	var downloadURL string

	assets := assetsArray[0]

	key = "browser_download_url"

	downloadURL, ok = assets.(map[string]interface{})[key].(string)
	if !ok {
		return "", details, fmt.Errorf("failed to find key %s in release version %q asset data", key, version)
	}

	if err := downloadURLIsValid(downloadURL); err != nil {
		return "", details, err
	}

	key = "name"

	filename, ok = assets.(map[string]interface{})[key].(string)
	if !ok {
		return "", details, fmt.Errorf("failed to find key %s in release version %q asset data", key, version)
	}

	if filename == "" {
		return "", details, fmt.Errorf("Release %q asset missing filename", version)
	}

	key = "created_at"

	createDate, ok = assets.(map[string]interface{})[key].(string)
	if !ok {
		return "", details, fmt.Errorf("failed to find key %s in release version %q asset data", key, version)
	}

	if createDate == "" {
		return "", details, fmt.Errorf("Release %q asset missing creation date", version)
	}

	details = releaseDetails{
		version:  releaseSemver,
		date:     createDate,
		url:      downloadURL,
		filename: filename,
	}

	return version, details, nil
}

func readReleases(releasesArray []map[string]interface{}, includeAll bool) (versions []semver.Version,
	releases map[string]releaseDetails) {

	releases = make(map[string]releaseDetails)

	for _, release := range releasesArray {
		version, details, err := makeRelease(release)

		// Don't error if makeRelease() fails to construct a release.
		// There are many reasons a release may not be considered
		// valid, so just ignore the invalid ones.
		if err != nil {
			kataLog.WithField("version", version).WithError(err).Debug("ignoring invalid release version")
			continue
		}

		if ignoreRelease(details, includeAll) {
			continue
		}

		versions = append(versions, details.version)
		releases[version] = details
	}

	semver.Sort(versions)

	return versions, releases
}

// Note: Assumes versions is sorted in ascending order
func findNewestRelease(currentVersion semver.Version, versions []semver.Version) (bool, semver.Version, error) {
	var candidates []semver.Version

	if len(versions) == 0 {
		return false, semver.Version{}, errors.New("no versions available")
	}

	for _, version := range versions {
		if currentVersion.GTE(version) {
			// Ignore older releases (and the current one!)
			continue
		}

		candidates = append(candidates, version)
	}

	count := len(candidates)

	if count == 0 {
		return false, semver.Version{}, nil
	}

	return true, candidates[count-1], nil
}

func getReleases(releaseURL string, includeAll bool) ([]semver.Version, map[string]releaseDetails, error) {
	kataLog.WithField("url", releaseURL).Info("Looking for releases")

	if os.Geteuid() == 0 {
		return nil, nil, errors.New(errNoNetChecksAsRoot)
	}

	client := &http.Client{}

	resp, err := client.Get(releaseURL)
	if err != nil {
		return nil, nil, err
	}

	defer resp.Body.Close()

	releasesArray := []map[string]interface{}{}

	body, err := ioutil.ReadAll(resp.Body)
	if err != nil {
		return nil, nil, fmt.Errorf("failed to read release details: %v", err)
	} else if resp.StatusCode == http.StatusForbidden && bytes.Contains(body, []byte("limit exceeded")) {
		// Do not fail if rate limit is exceeded
		kataLog.WithField("url", releaseURL).
			Warn("API rate limit exceeded. Try again later. Read https://docs.github.com/apps/building-github-apps/understanding-rate-limits-for-github-apps for more information")
		return []semver.Version{}, map[string]releaseDetails{}, nil
	}

	if err := json.Unmarshal(body, &releasesArray); err != nil {
		return nil, nil, fmt.Errorf("failed to unpack release details: %v", err)
	}

	versions, releases := readReleases(releasesArray, includeAll)

	return versions, releases, nil
}

func getNewReleaseType(current semver.Version, latest semver.Version) (string, error) {
	if current.GT(latest) {
		return "", fmt.Errorf("current version %s newer than latest %s", current, latest)
	}

	if current.EQ(latest) {
		return "", fmt.Errorf("current version %s and latest are same", current)
	}

	var desc string

	if latest.Major > current.Major {
		if len(latest.Pre) > 0 {
			desc = "major pre-release"
		} else {
			desc = "major"
		}
	} else if latest.Minor > current.Minor {
		if len(latest.Pre) > 0 {
			desc = "minor pre-release"
		} else {
			desc = "minor"
		}
	} else if latest.Patch > current.Patch {
		if len(latest.Pre) > 0 {
			desc = "patch pre-release"
		} else {
			desc = "patch"
		}
	} else if latest.Patch == current.Patch && len(latest.Pre) > 0 {
		desc = "pre-release"
	} else if latest.Major == current.Major &&
		latest.Minor == current.Minor &&
		latest.Patch == current.Patch {
		if len(current.Pre) > 0 && len(latest.Pre) == 0 {
			desc = "major"
		}
	} else {
		return "", fmt.Errorf("BUG: unhandled scenario: current version: %s, latest version: %s", current, latest)
	}

	return desc, nil
}

func showLatestRelease(output *os.File, current semver.Version, details releaseDetails) error {
	latest := details.version

	desc, err := getNewReleaseType(current, latest)
	if err != nil {
		return err
	}

	fmt.Fprintf(output, "Newer %s release available: %s (url: %v, date: %v)\n",
		desc,
		details.version, details.url, details.date)

	return nil
}

func listReleases(output *os.File, current semver.Version, versions []semver.Version, releases map[string]releaseDetails) error {
	for _, version := range versions {
		details, ok := releases[version.String()]
		if !ok {
			return fmt.Errorf("Release %v has no details", version)
		}

		fmt.Fprintf(output, "%s;%s;%s\n", version, details.date, details.url)
	}

	return nil
}

func HandleReleaseVersions(cmd ReleaseCmd, currentVersion string, includeAll bool) error {
	if !cmd.Valid() {
		return fmt.Errorf("invalid release command: %v", cmd)
	}

	output := os.Stdout

	currentSemver, err := semver.Make(currentVersion)
	if err != nil {
		return fmt.Errorf("BUG: Current version of %s (%s) has invalid SemVer version: %v", name, currentVersion, err)
	}

	releaseURL, err := getReleaseURL(currentSemver)
	if err != nil {
		return err
	}

	versions, releases, err := getReleases(releaseURL, includeAll)
	if err != nil {
		return err
	}

	if cmd == RelCmdList {
		return listReleases(output, currentSemver, versions, releases)
	}

	if len(versions) == 0 {
		fmt.Fprintf(output, "%s\n", msgNoReleases)
		return nil
	}

	available, newest, err := findNewestRelease(currentSemver, versions)
	if err != nil {
		return err
	}

	if !available {
		fmt.Fprintf(output, "%s\n", msgNoNewerRelease)
		return nil
	}

	details, ok := releases[newest.String()]
	if !ok {
		return fmt.Errorf("Release %v has no details", newest)
	}

	return showLatestRelease(output, currentSemver, details)
}
