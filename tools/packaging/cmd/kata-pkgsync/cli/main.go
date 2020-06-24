// Copyright (c) 2019 SUSE LLC
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"flag"
	"fmt"
	"os"
	"strings"

	"github.com/marcov/obsgo"
	"github.com/mlafeldt/pkgcloud"
	"github.com/sirupsen/logrus"
	pb "gopkg.in/cheggaaa/pb.v1"
)

//nolint[:gochecknoglobals]
var (
	// empty variables are set by "go build -ldflags" option
	name           = ""
	version        = ""
	commit         = ""
	defaultConfig  = "config.yaml"
	defaultOBSDest = "obs-packages"
)

func usage() {
	fmt.Fprintf(flag.CommandLine.Output(), `NAME:
   %s - Synchronize packages from OBS to Packagecloud

USAGE:
   %s [options] [config file]

By default %s reads the configuration from a file named %s. This can be
overridden by passing the path to a config file.

Options:
`, name, name, name, defaultConfig)

	flag.PrintDefaults()
}

func getOBSProjects(cfgProjects map[string]CfgOBSProject) []obsgo.Project {
	var projects []obsgo.Project

	for n, p := range cfgProjects {
		proj := obsgo.Project{
			User:     p.Auth.User,
			Password: p.Auth.Password,
		}

		if len(p.Archs) == 0 {
			p.Archs = append(p.Archs, "")
		}

		if len(p.Releases) == 0 {
			p.Releases = append(p.Releases, "")
		}

		// Kata projects names format is "project:release:architecture"
		for _, arch := range p.Archs {
			for _, release := range p.Releases {
				var fullname strings.Builder

				fullname.WriteString(n)
				if arch != "" {
					fmt.Fprintf(&fullname, ":%s", arch)
				}
				if release != "" {
					fmt.Fprintf(&fullname, ":%s", release)
				}

				proj.Name = fullname.String()
				projects = append(projects, proj)
			}
		}
	}

	return projects
}

func getXferBinaries(pkg obsgo.PackageInfo, pcDistro string,
	pcPackages []pkgcloud.Package, pcPackagesNeeded *[]bool) []obsgo.PkgBinary {

	var xferBins []obsgo.PkgBinary
	for _, src := range pkg.Files {
		found := false
		for i, dst := range pcPackages {
			logrus.WithFields(logrus.Fields{
				"source-file":        src.Filename,
				"source-distro":      pcDistro,
				"destination-file":   dst.Filename,
				"destination-distro": dst.DistroVersion,
			}).Debug("Checking package")
			if pcDistro == dst.DistroVersion && src.Filename == dst.Filename {
				logrus.WithFields(logrus.Fields{
					"filename": src.Filename,
					"distro":   pcDistro,
				}).Debug("Package file already on Packagecloud")
				found = true
				(*pcPackagesNeeded)[i] = true
				break
			}
		}
		if !found {
			logrus.WithFields(logrus.Fields{
				"filename": src.Filename,
				"distro":   pcDistro,
			}).Debug("Package file NOT on Packagecloud")
			xferBins = append(xferBins, src)
		}
	}

	return xferBins
}

func main() {
	flag.Usage = usage
	verbose := flag.Bool("debug", false, "debug mode")
	dlPath := flag.String("dir", defaultOBSDest, "Destination directory of packages download from OBS")
	dryRun := flag.Bool("dry-run", false, "dry-run mode (do not download/upload files)")
	pcDelete := flag.Bool("delete", false, "Delete Packagecloud packages that are not published on OBS")
	showVersion := flag.Bool("version", false, "show the version")

	flag.Parse()

	if *showVersion {
		fmt.Printf("%s %s (commit %v)\n", name, version, commit)
		os.Exit(0)
	}

	if *verbose {
		fmt.Println("Starting in debug mode...")
		logrus.SetLevel(logrus.DebugLevel)
	}

	var configFile string
	if len(flag.Args()) > 0 {
		configFile = flag.Args()[0]
	} else {
		configFile = defaultConfig
	}
	logrus.Debugf("Using config file %s", configFile)

	cfg, err := getConfig(configFile)
	if err != nil {
		logrus.WithError(err).Error("Failed to read config")
		os.Exit(-1)
	}
	logrus.Debugf("Configuration file content: %+v", cfg)

	var pc PCClient
	if err := pc.PackagecloudClient(
		cfg.Packagecloud.Auth.User,
		cfg.Packagecloud.Auth.Token,
		cfg.Packagecloud.Repo); err != nil {
		logrus.WithError(err).Error("Failed to create a Packagecloud client instance")
		os.Exit(-1)
	}
	logrus.WithFields(logrus.Fields{
		"repo": pc.Repo,
	}).Info("Retrieving Packagecloud list of files")

	pcPackages, err := pc.PackagecloudList()
	if err != nil {
		logrus.WithFields(logrus.Fields{
			"repo":  pc.Repo,
			"error": err,
		}).Error("Failed to retrieve Packagecloud list of files")
		os.Exit(-1)
	}

	// lookup table for pcPackages packages that should NOT be deleted from Packagecloud
	pcPackagesNeeded := make([]bool, len(pcPackages))

	projects := getOBSProjects(cfg.OBSProjects)
	for _, proj := range projects {
		logrus.WithFields(logrus.Fields{
			"OBS project": proj.Name,
		}).Info("Retrieving packages info")

		obsPackages, err := proj.FindAllPackages()
		if err != nil {
			logrus.WithError(err).WithFields(logrus.Fields{
				"project": proj.Name,
			}).Error("Failed to get OBS packages")
			os.Exit(-1)
		}

		logrus.WithFields(logrus.Fields{
			"OBS project": proj.Name,
		}).Infof("Found %d packages", len(obsPackages))

		totalXferred := 0
		for _, pkg := range obsPackages {
			pcDistro, found := cfg.DistroMapping[pkg.Repo]
			if !found {
				logrus.WithFields(logrus.Fields{
					"OBS Repo": pkg.Repo,
				}).Warn("No mapped Packagecloud distro specified")
				os.Exit(-1)
			}

			if pcDistro == "" {
				logrus.WithFields(logrus.Fields{
					"package":  pkg.Name,
					"OBS Repo": pkg.Repo,
				}).Warn("Repo not supported by Packagecloud")
				continue
			}

			xferBins := getXferBinaries(pkg, pcDistro, pcPackages, &pcPackagesNeeded)
			if len(xferBins) == 0 {
				logrus.WithFields(logrus.Fields{
					"package":  pkg.Name,
					"OBS Repo": pkg.Repo,
				}).Infof("All %d files already on Packagecloud", len(pkg.Files))
				continue
			}

			logrus.WithFields(logrus.Fields{
				"pkg":     pkg.Name,
				"repo":    pkg.Repo,
				"# files": len(xferBins),
			}).Info("Downloading from OBS")

			if *dryRun {
				continue
			}

			pkg.Files = xferBins
			paths, err := proj.DownloadPackageFiles(pkg, *dlPath)
			if err != nil {
				logrus.WithError(err).Warnf("Failed to download binaries for %s on %s/%s", pkg.Name, pc.Repo, pcDistro)
				continue
			}

			logrus.WithFields(logrus.Fields{
				"pkg":     pkg.Name,
				"distro":  pcDistro,
				"# files": len(xferBins),
			}).Info("Uploading to Packagecloud")

			err = pc.PackagecloudPush(paths, pcDistro)
			if err != nil {
				logrus.WithError(err).WithFields(logrus.Fields{
					"package": pkg.Name,
					"distro":  pcDistro,
				}).Error("Failed to push binaries to Packagecloud")
			}

			totalXferred += len(xferBins)
		}

		logrus.WithFields(logrus.Fields{
			"OBS project":       proj.Name,
			"Packagecloud Repo": pc.Repo,
		}).Infof("Successfully transferred %d files", totalXferred)
	}

	if !*pcDelete {
		return
	}

	logrus.WithFields(logrus.Fields{
		"Repo": pc.Repo,
	}).Info("Finding and deleting Packagecloud files not published on OBS")
	progressBar := pb.New(len(pcPackages))
	progressBar.SetMaxWidth(100)
	progressBar.Start()
	totalDeleted := 0
	for i, pkg := range pcPackages {
		if !pcPackagesNeeded[i] {
			if *dryRun {
				continue
			}
			if err := pc.PackagecloudDelete(pkg.Filename, pkg.DistroVersion); err != nil {
				logrus.WithError(err).WithFields(logrus.Fields{
					"name":   pkg.Filename,
					"distro": pkg.DistroVersion,
				}).Error("Failed to delete package on Packagecloud")
			}
			totalDeleted++
		}
		progressBar.Increment()
	}
	progressBar.Finish()

	logrus.WithFields(logrus.Fields{
		"Packagecloud Repo": pc.Repo,
	}).Infof("Deleted %d files", totalDeleted)
}
