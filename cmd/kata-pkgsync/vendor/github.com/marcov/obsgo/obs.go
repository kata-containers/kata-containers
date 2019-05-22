//Package obsgo implements some of the Open Build Service APIs [1]
//for interacting with an OBS project from a Go application.
//
//At the moment uniquely a limited subset of APIs are implemented, focusing on
//those needed to retrieve information about packages, and downloading build
//artifacts.
//
// [1]: https://build.opensuse.org/apidocs/index
package obsgo

import (
	"fmt"
	"os"
	"path"
	"path/filepath"
	"regexp"
	"strconv"

	"github.com/pkg/errors"
	"github.com/sirupsen/logrus"
	pb "gopkg.in/cheggaaa/pb.v1"
)

// Project represents an OBS project
type Project struct {
	// Name of the project
	Name string
	// Username needed to access the project with APIs
	User string
	// Password needed to access the project with APIs
	Password string
}

// PackageInfo groups information related to an OBS package.
type PackageInfo struct {
	// Name of the package
	Name string
	// Path of the package used for APIs queries
	Path string
	// Repository of the package
	Repo string
	// Architecture of the binary files built for the Package
	Arch string
	// The list of binary files built for the package
	Files []PkgBinary
}

// Given a PackageInfo instance, returns all binary Package files published
// on the OBS project, whose names match the binaryPackageRE regular expression.
func (proj *Project) PackageBinaries(pkg *PackageInfo) error {
	debArchitectures := map[string]string{
		"x86_64":  "amd64",
		"aarch64": "arm64",
		"ppc64le": "ppc64el",
		"s390x":   "s390x",
	}
	debArch, ok := debArchitectures[pkg.Arch]
	if !ok {
		return errors.Errorf("Cannot find corresponding debian architecture to %s", pkg.Arch)
	}
	debExtensionRE := fmt.Sprintf(`_(all|%s)\.deb`, debArch)
	rpmExtensionRE := fmt.Sprintf(`\.(noarch|%s)\.rpm`, pkg.Arch)
	binaryPackageRE := fmt.Sprintf(`(%s|%s)$`, rpmExtensionRE, debExtensionRE)

	pkg.Path = path.Join(pkg.Repo, pkg.Arch, pkg.Name)
	logrus.WithFields(logrus.Fields{
		"path": pkg.Path,
	}).Debug("Retrieving OBS package binaries")
	allBins, err := proj.listBinaries(pkg.Path)
	if err != nil {
		return errors.Wrapf(err, "Failed to get get list of OBS binaries")
	}

	re := regexp.MustCompile(binaryPackageRE)

	for _, b := range allBins {
		logrus.WithFields(logrus.Fields{
			"file": b,
		}).Debug("OBS processing package file")
		if re.Match([]byte(b.Filename)) {
			pkg.Files = append(pkg.Files, b)
		}
	}

	return nil
}

// Returns all the packages files published on the OBS project.
func (proj *Project) FindAllPackages() ([]PackageInfo, error) {
	var pkgList []PackageInfo

	logrus.WithFields(logrus.Fields{
		"project": proj.Name,
	}).Debug("Finding all OBS packages and files")

	progressBar := pb.New(0)
	progressBar.SetMaxWidth(100)
	progressBar.Start()
	defer progressBar.Finish()

	repos, err := proj.ListRepos()
	if err != nil {
		return pkgList, errors.Wrapf(err, "failed to get list of repos for project %s\n", proj.Name)
	}

	for _, repo := range repos {
		archs, err := proj.ListArchs(repo)
		if err != nil {
			return pkgList, errors.Wrapf(err, "failed to get list of archs for project %s\n", proj.Name)
		}

		for _, arch := range archs {
			pkgs, err := proj.ListPackages(repo, arch)
			if err != nil {
				return pkgList, errors.Wrapf(err, "failed to get list of pkgs for project %s\n", proj.Name)
			}

			for _, pkg := range pkgs {
				if progressBar.Get() == 0 {
					progressBar.SetTotal(len(repos) * len(pkgs) * len(archs))
				}

				progressBar.Increment()

				newPkg := PackageInfo{
					Name: pkg,
					Repo: repo,
					Arch: arch,
				}

				err := proj.PackageBinaries(&newPkg)
				if err != nil {
					return pkgList, err
				}

				pkgList = append(pkgList, newPkg)
			}
		}
	}

	return pkgList, nil
}

// Downloads all the files specified in the passed pkgInfo argument, and returns
// a slice with a list of the locally downloaded files.
func (proj *Project) DownloadPackageFiles(pkgInfo PackageInfo, root string) ([]string, error) {
	logrus.WithFields(logrus.Fields{
		"project": proj.Name,
		"repo":    pkgInfo.Repo,
	}).Debug("Downloading OBS package files")

	progressBar := pb.New(len(pkgInfo.Files))
	progressBar.SetMaxWidth(100)
	progressBar.Start()
	defer progressBar.Finish()

	filePaths := make([]string, 0, len(pkgInfo.Files))
	for _, f := range pkgInfo.Files {
		remotePath := path.Join(pkgInfo.Path, f.Filename)
		localFile := filepath.Join(root, proj.Name, remotePath)
		filePaths = append(filePaths, localFile)

		info, err := os.Stat(localFile)
		if !(err == nil || os.IsNotExist(err)) {
			return filePaths, err
		}

		fsize, err := strconv.Atoi(f.Size)
		if err != nil {
			return filePaths, errors.Wrapf(err, "could not parse file size %s", localFile)
		}

		if info != nil && info.Size() == int64(fsize) {
			logrus.WithFields(logrus.Fields{
				"filename": f.Filename,
			}).Debug("OBS file already downloaded")
			progressBar.Increment()
			continue
		}

		err = os.MkdirAll(filepath.Dir(localFile), 0700)
		if err != nil {
			return filePaths, errors.Wrapf(err, "could not mkdir path %s", remotePath)
		}

		destFile, err := os.Create(localFile)
		if err != nil {
			return filePaths, errors.Wrapf(err, "could not create local file %s", localFile)
		}

		logrus.WithFields(logrus.Fields{
			"filename": f.Filename,
		}).Debug("Downloading OBS file")

		err = proj.downloadBinary(remotePath, destFile)
		if err != nil {
			return filePaths, errors.Wrapf(err, "could not download binary at %s", remotePath)
		}

		progressBar.Increment()
	}

	return filePaths, nil
}

// Returns a string slice with a list of repositories available in the project
// proj.
func (proj *Project) ListRepos() ([]string, error) {
	return proj.listDirectories("")
}

// Returns a string slice with a list of target architectures available in the
// repository repo inside project proj.
func (proj *Project) ListArchs(repo string) ([]string, error) {
	return proj.listDirectories(repo)
}

// Returns a string slice with a list of packages for the given architecture arch,
// repository repo inside the project proj.
func (proj *Project) ListPackages(repo, arch string) ([]string, error) {
	url := path.Join(repo, arch)
	return proj.listDirectories(url)
}
