// Copyright (c) 2017-2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"path"

	"github.com/mlafeldt/pkgcloud"
	"github.com/sirupsen/logrus"
)

type PCClient struct {
	*pkgcloud.Client
	Repo string
}

func (pc *PCClient) PackagecloudClient(user string, token string, repo string) error {
	client, err := pkgcloud.NewClient(token)
	if err != nil {
		return err
	}

	*pc = PCClient{client, path.Join(user, repo)}
	pc.ShowProgress(true)

	return nil
}

func (pc PCClient) PackagecloudList() ([]pkgcloud.Package, error) {
	logrus.WithFields(logrus.Fields{
		"repo": pc.Repo,
	}).Debug("Packagecloud listing package(s)")
	return pc.All(pc.Repo)
}

func (pc PCClient) PackagecloudSearchPackage(pkg string) ([]pkgcloud.Package, error) {
	logrus.WithFields(logrus.Fields{
		"repo": pc.Repo,
		"pkg":  pkg,
	}).Debug("Packagecloud searching package")
	return pc.Search(pc.Repo, pkg, "", "", 0)
}

func (pc PCClient) PackagecloudPush(packages []string, distro string) error {
	logrus.WithFields(logrus.Fields{
		"repo":   pc.Repo,
		"#":      len(packages),
		"distro": distro,
	}).Debug("Packagecloud pushing package")
	for _, pkg := range packages {
		if err := pc.CreatePackage(pc.Repo, distro, pkg); err != nil {
			return err
		}
	}

	return nil
}

func (pc PCClient) PackagecloudDelete(filename string, distro string) error {
	logrus.WithFields(logrus.Fields{
		"repo":     pc.Repo,
		"filename": filename,
		"distro":   distro,
	}).Debug("Packagecloud delete package")
	return pc.Destroy(pc.Repo, path.Join(distro, filename))
}
