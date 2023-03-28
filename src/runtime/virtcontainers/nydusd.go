// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"bufio"
	"bytes"
	"context"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"io"
	"net"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"strings"
	"syscall"
	"time"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils/katatrace"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils/retry"
	"github.com/pkg/errors"
	"github.com/sirupsen/logrus"
)

const (
	infoEndpoint  = "http://unix/api/v1/daemon"
	mountEndpoint = "http://unix/api/v1/mount"

	nydusdDaemonStateRunning = "RUNNING"

	nydusdStopTimeoutSecs = 5

	defaultHttpClientTimeoutSecs = 30 * time.Second
	contentType                  = "application/json"

	maxIdleConns              = 10
	idleConnTimeoutSecs       = 10 * time.Second
	dialTimoutSecs            = 5 * time.Second
	keepAliveSecs             = 5 * time.Second
	expectContinueTimeoutSecs = 1 * time.Second

	// Registry Acceleration File System which is nydus provide to accelerate image load
	nydusRafs = "rafs"
	// used to shared directories between host and guest
	nydusPassthroughfs = "passthrough_fs"

	sharedPathInGuest = "/containers"
)

var (
	nydusdTracingTags = map[string]string{
		"source":    "runtime",
		"package":   "virtcontainers",
		"subsystem": "nydusd",
	}

	errNydusdDaemonPathInvalid  = errors.New("nydusd daemon path is invalid")
	errNydusdSockPathInvalid    = errors.New("nydusd sock path is invalid")
	errNydusdAPISockPathInvalid = errors.New("nydusd api sock path is invalid")
	errNydusdSourcePathInvalid  = errors.New("nydusd resource path is invalid")
	errNydusdNotSupport         = errors.New("nydusd only supports the QEMU/CLH hypervisor currently (see https://github.com/kata-containers/kata-containers/issues/3654)")
)

type nydusd struct {
	startFn         func(cmd *exec.Cmd) error // for mock testing
	waitFn          func() error              // for mock
	setupShareDirFn func() error              // for mock testing
	path            string
	sockPath        string
	apiSockPath     string
	sourcePath      string
	extraArgs       []string
	pid             int
	debug           bool
}

func (nd *nydusd) Start(ctx context.Context, onQuit onQuitFunc) (int, error) {
	span, _ := katatrace.Trace(ctx, nd.Logger(), "Start", nydusdTracingTags)
	defer span.End()
	pid := 0

	if err := nd.valid(); err != nil {
		return pid, err
	}

	args, err := nd.args()
	if err != nil {
		return pid, err
	}
	cmd := exec.Command(nd.path, args...)
	stdout, err := cmd.StdoutPipe()
	if err != nil {
		return pid, err
	}

	cmd.Stderr = cmd.Stdout

	fields := logrus.Fields{
		"path": nd.path,
		"args": strings.Join(args, " "),
	}
	nd.Logger().WithFields(fields).Info("starting nydusd")
	if err := nd.startFn(cmd); err != nil {
		return pid, errors.Wrap(err, "failed to start nydusd")
	}
	nd.Logger().WithFields(fields).Info("nydusd started")

	// Monitor nydusd's stdout/stderr and stop sandbox if nydusd quits
	go func() {
		scanner := bufio.NewScanner(stdout)
		for scanner.Scan() {
			nd.Logger().Info(scanner.Text())
		}
		nd.Logger().Warn("nydusd quits")
		// Wait to release resources of nydusd process
		_, err = cmd.Process.Wait()
		if err != nil {
			nd.Logger().WithError(err).Warn("nydusd quits")
		}
		if onQuit != nil {
			onQuit()
		}
	}()

	nd.Logger().Info("waiting nydusd API server ready")
	waitFn := nd.waitUntilNydusAPIServerReady
	// waitFn may be set by a mock function for test
	if nd.waitFn != nil {
		waitFn = nd.waitFn
	}
	if err := waitFn(); err != nil {
		return pid, errors.Wrap(err, "failed to wait nydusd API server ready")
	}
	nd.Logger().Info("nydusd API server ready, begin to setup share dir")

	if err := nd.setupShareDirFn(); err != nil {
		return pid, errors.Wrap(err, "failed to setup share dir for nydus")
	}

	nd.Logger().Info("nydusd setup share dir completed")

	nd.pid = cmd.Process.Pid
	return nd.pid, nil
}

func (nd *nydusd) args() ([]string, error) {
	logLevel := "info"
	if nd.debug {
		logLevel = "debug"
	}
	args := []string{
		"virtiofs",
		"--log-level", logLevel,
		"--apisock", nd.apiSockPath,
		"--sock", nd.sockPath,
	}
	if len(nd.extraArgs) > 0 {
		args = append(args, nd.extraArgs...)
	}
	return args, nil
}

func checkPathValid(path string) error {
	if len(path) == 0 {
		return errors.New("path is empty")
	}
	absPath, err := filepath.Abs(path)
	if err != nil {
		return err
	}
	dir := filepath.Dir(absPath)
	if _, err := os.Stat(dir); err != nil {
		return err
	}
	return nil
}

func (nd *nydusd) valid() error {
	if err := checkPathValid(nd.sockPath); err != nil {
		nd.Logger().WithError(err).Info("check nydusd sock path err")
		return errNydusdSockPathInvalid
	}
	if err := checkPathValid(nd.apiSockPath); err != nil {
		nd.Logger().WithError(err).Info("check nydusd api sock path err")
		return errNydusdAPISockPathInvalid
	}

	if err := checkPathValid(nd.path); err != nil {
		nd.Logger().WithError(err).Info("check nydusd daemon path err")
		return errNydusdDaemonPathInvalid
	}
	if err := checkPathValid(nd.sourcePath); err != nil {
		nd.Logger().WithError(err).Info("check nydusd daemon path err")
		return errNydusdSourcePathInvalid
	}
	return nil
}

func (nd *nydusd) setupPassthroughFS() error {
	nd.Logger().WithField("from", nd.sourcePath).
		WithField("dest", sharedPathInGuest).Info("prepare mount passthroughfs")

	nc, err := NewNydusClient(nd.apiSockPath)
	if err != nil {
		return err
	}

	mr := NewMountRequest(nydusPassthroughfs, nd.sourcePath, "")
	return nc.Mount(sharedPathInGuest, mr)
}

func (nd *nydusd) waitUntilNydusAPIServerReady() error {
	return retry.Do(func() error {
		nc, err := NewNydusClient(nd.apiSockPath)
		if err != nil {
			return err
		}

		di, err := nc.CheckStatus()
		if err != nil {
			return err
		}
		if di.State == nydusdDaemonStateRunning {
			return nil
		}
		return fmt.Errorf("Nydusd daemon is not running: %s", di.State)
	},
		retry.Attempts(20),
		retry.LastErrorOnly(true),
		retry.Delay(20*time.Millisecond))
}

func (nd *nydusd) Mount(opt MountOption) error {
	nc, err := NewNydusClient(nd.apiSockPath)
	if err != nil {
		return err
	}
	nd.Logger().WithField("from", opt.source).
		WithField("dest", opt.mountpoint).Info("prepare mount rafs")

	mr := NewMountRequest(nydusRafs, opt.source, opt.config)
	return nc.Mount(opt.mountpoint, mr)
}

func (nd *nydusd) Umount(mountpoint string) error {
	nc, err := NewNydusClient(nd.apiSockPath)
	if err != nil {
		return err
	}
	nd.Logger().WithField("mountpoint", mountpoint).Info("umount rafs")
	return nc.Umount(mountpoint)
}

func (nd *nydusd) Stop(ctx context.Context) error {
	if err := nd.kill(ctx); err != nil {
		nd.Logger().WithError(err).WithField("pid", nd.pid).Warn("kill nydusd failed")
		return nil
	}

	err := os.Remove(nd.sockPath)
	if err != nil {
		nd.Logger().WithError(err).WithField("path", nd.sockPath).Warn("removing nydusd socket failed")
	}
	err = os.Remove(nd.apiSockPath)
	if err != nil {
		nd.Logger().WithError(err).WithField("path", nd.apiSockPath).Warn("removing nydusd api socket failed")
	}
	return nil
}

func (nd *nydusd) Logger() *logrus.Entry {
	return hvLogger.WithField("subsystem", "nydusd")
}

func (nd *nydusd) kill(ctx context.Context) (err error) {
	span, _ := katatrace.Trace(ctx, nd.Logger(), "kill", nydusdTracingTags)
	defer span.End()

	if nd.pid <= 0 {
		nd.Logger().WithField("invalid-nydusd-pid", nd.pid).Warn("cannot kill nydusd")
		return nil
	}
	if err := utils.WaitLocalProcess(nd.pid, nydusdStopTimeoutSecs, syscall.SIGTERM, nd.Logger()); err != nil {
		nd.Logger().WithError(err).Warn("kill nydusd err")
	}
	nd.pid = 0
	return err
}

type BuildTimeInfo struct {
	PackageVer string `json:"package_ver"`
	GitCommit  string `json:"git_commit"`
	BuildTime  string `json:"build_time"`
	Profile    string `json:"profile"`
	Rustc      string `json:"rustc"`
}

type DaemonInfo struct {
	ID      string        `json:"id"`
	Version BuildTimeInfo `json:"version"`
	State   string        `json:"state"`
}

type ErrorMessage struct {
	Code    string `json:"code"`
	Message string `json:"message"`
}

type MountRequest struct {
	FsType string `json:"fs_type"`
	Source string `json:"source"`
	Config string `json:"config"`
}

func NewMountRequest(fsType, source, config string) *MountRequest {
	return &MountRequest{
		FsType: fsType,
		Source: source,
		Config: config,
	}
}

type Interface interface {
	CheckStatus() (DaemonInfo, error)
	Mount(string, *MountRequest) error
	Umount(sharedMountPoint string) error
}

type NydusClient struct {
	httpClient *http.Client
}

func NewNydusClient(sock string) (Interface, error) {
	transport, err := buildTransport(sock)
	if err != nil {
		return nil, err
	}
	return &NydusClient{
		httpClient: &http.Client{
			Timeout:   defaultHttpClientTimeoutSecs,
			Transport: transport,
		},
	}, nil
}

func waitUntilSocketReady(sock string) error {
	return retry.Do(func() error {
		if _, err := os.Stat(sock); err != nil {
			return err
		}
		return nil
	},
		retry.Attempts(3),
		retry.LastErrorOnly(true),
		retry.Delay(100*time.Millisecond))
}

func buildTransport(sock string) (http.RoundTripper, error) {
	err := waitUntilSocketReady(sock)
	if err != nil {
		return nil, err
	}
	return &http.Transport{
		MaxIdleConns:          maxIdleConns,
		IdleConnTimeout:       idleConnTimeoutSecs,
		ExpectContinueTimeout: expectContinueTimeoutSecs,
		DialContext: func(ctx context.Context, _, _ string) (net.Conn, error) {
			dialer := &net.Dialer{
				Timeout:   dialTimoutSecs,
				KeepAlive: keepAliveSecs,
			}
			return dialer.DialContext(ctx, "unix", sock)
		},
	}, nil
}

func (c *NydusClient) CheckStatus() (DaemonInfo, error) {
	resp, err := c.httpClient.Get(infoEndpoint)
	if err != nil {
		return DaemonInfo{}, err
	}
	defer resp.Body.Close()
	b, err := io.ReadAll(resp.Body)
	if err != nil {
		return DaemonInfo{}, err
	}
	var info DaemonInfo
	if err = json.Unmarshal(b, &info); err != nil {
		return DaemonInfo{}, err
	}
	return info, nil
}

func checkRafsMountPointValid(mp string) bool {
	// refer to https://github.com/opencontainers/runc/blob/master/libcontainer/factory_linux.go#L30
	re := regexp.MustCompile(`/rafs/[\w+-\.]+/lowerdir`)
	return re.MatchString(mp)
}

func (c *NydusClient) checkMountPoint(mountPoint string, fsType string) error {
	switch fsType {
	case nydusPassthroughfs:
		// sharedir has been checked in args check.
		return nil
	case nydusRafs:
		// nydusRafs mountpoint path format: /rafs/<container_id>/lowerdir
		if checkRafsMountPointValid(mountPoint) {
			return nil
		}
		return fmt.Errorf("rafs mountpoint %s is invalid", mountPoint)
	default:
		return errors.New("unsupported filesystem type")
	}
}

func (c *NydusClient) Mount(mountPoint string, mr *MountRequest) error {
	if err := c.checkMountPoint(mountPoint, mr.FsType); err != nil {
		return errors.Wrap(err, "check mount point err")
	}
	requestURL := fmt.Sprintf("%s?mountpoint=%s", mountEndpoint, mountPoint)
	body, err := json.Marshal(mr)
	if err != nil {
		return errors.Wrap(err, "failed to create mount request")
	}

	resp, err := c.httpClient.Post(requestURL, contentType, bytes.NewBuffer(body))
	if err != nil {
		return err
	}
	defer resp.Body.Close()
	if resp.StatusCode == http.StatusNoContent {
		return nil
	}
	return handleMountError(resp.Body)
}

func (c *NydusClient) Umount(mountPoint string) error {
	requestURL := fmt.Sprintf("%s?mountpoint=%s", mountEndpoint, mountPoint)
	req, err := http.NewRequest(http.MethodDelete, requestURL, nil)
	if err != nil {
		return err
	}
	resp, err := c.httpClient.Do(req)
	if err != nil {
		return err
	}
	defer resp.Body.Close()
	if resp.StatusCode == http.StatusNoContent {
		return nil
	}
	return handleMountError(resp.Body)
}

func handleMountError(r io.Reader) error {
	b, err := io.ReadAll(r)
	if err != nil {
		return err
	}
	var errMessage ErrorMessage
	if err = json.Unmarshal(b, &errMessage); err != nil {
		return err
	}
	return errors.New(errMessage.Message)
}

/*
   rootfs mount format: Type: fuse.nydus-overlayfs, Source: overlay
   Options：[lowerdir=/foo/lower2:/foo/lower1,upperdir=/foo/upper,workdir=/foo/work,extraoption={source: xxx, config: xxx, snapshotdir: xxx}]
*/

type extraOption struct {
	Source      string `json:"source"`
	Config      string `json:"config"`
	Snapshotdir string `json:"snapshotdir"`
}

const extraOptionKey = "extraoption="

func parseExtraOption(options []string) (*extraOption, error) {
	extraOpt := ""
	for _, opt := range options {
		if strings.HasPrefix(opt, extraOptionKey) {
			extraOpt = strings.TrimPrefix(opt, extraOptionKey)
		}
	}
	if len(extraOpt) == 0 {
		return nil, errors.New("no extraoption found")
	}

	opt, err := base64.StdEncoding.DecodeString(extraOpt)
	if err != nil {
		return nil, errors.Wrap(err, "base64 decoding err")
	}

	no := &extraOption{}
	if err := json.Unmarshal(opt, no); err != nil {
		return nil, errors.Wrapf(err, "json unmarshal err")
	}
	if len(no.Config) == 0 || len(no.Snapshotdir) == 0 || len(no.Source) == 0 {
		return nil, fmt.Errorf("extra option is not correct, %+v", no)
	}

	return no, nil
}
