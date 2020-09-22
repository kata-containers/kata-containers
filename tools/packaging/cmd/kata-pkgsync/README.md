# Kata OBS to Packagecloud sync tool

* [How it works](#how-it-works)
* [Detailed behaviour](#detailed-behaviour)
* [Install and Usage](#install-and-usage)

`kata-pkgsync` is a tool to synchronize Kata package from OBS to Packagecloud.

## How it works

`kata-pkgsync` autonomously discovers OBS packages, repositories, and architectures
in a OBS project.
It can detect:
- which of the binary files published on OBS are already stored on Packagecloud,
- which ones still needs to be synchronized,
- which packages on Packagecloud are orphans, i.e. do not have a corresponding
file published on OBS.

Based on this information, `kata-pkgsyncs` can download only the necessary
files from OBS, upload them on Packagecloud, and delete orphans Packagecloud packages.


## Detailed behaviour

This is the sequence of tasks executed:

1. Fetch the configuration from a YAML config file.
2. For each OBS project specified, retrieve the available repositories,
architectures, packages. For each combination of `{repository,architecture,package}`,
retrieve the list of the build artifacts (i.e. the "rpm" / "deb" package files).
3. Get the list of files/packages already uploaded on Packagecloud.
This is to avoid re-uploading packages already sent to Packagecloud.
4. Build a list of files that needs to be synchronized from OBS to Packagecloud,
and identify which of the Packagecloud files have a corresponding file published
on OBS.
5. Download the identified files from OBS. The download phase create a local cache
of packages, to avoid re-downloading files if already done.
6. Upload the identified files to Packagecloud.
7. Optionally, delete orphans files from Packagecloud.

## Install and Usage

Install with:
```
$ go get github.com/kata-containers/kata-containers`
```

Create your configuration:
```
$ cd $GOPATH/src/github.com/kata-containers/kata-containers/tools/packaging/cmd/kata-pkgsync
$ cp config-example.yaml config.yaml
```

Run in "default" mode:
```
$ ~/go/bin/kata-pkgsync
```
See the help (`kata-pkgsync -h`) for more details.
