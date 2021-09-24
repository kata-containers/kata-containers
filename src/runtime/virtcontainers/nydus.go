package virtcontainers

import (
	"bufio"
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"io/ioutil"
	"net"
	"net/http"
	"os"
	"os/exec"
	"strings"
	"syscall"
	"time"

	"github.com/cenkalti/backoff"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils/katatrace"
	"github.com/pkg/errors"
	log "github.com/sirupsen/logrus"
)

const (
	infoEndpoint  = "/api/v1/daemon"
	mountEndpoint = "/api/v1/mount"

	defaultHttpClientTimeout = 30 * time.Second
	contentType              = "application/json"

	nydusRafsType          = "rafs"
	nydusPassthroughfsType = "passthrough_fs"

	configPrefix   = "config="
	snapshotPrefix = "snapshotdir="

	sharedPathInGuest = "/containers"
)

var nydusdTracingTags = map[string]string{
	"source":    "runtime",
	"package":   "virtcontainers",
	"subsystem": "nydusd",
}

type nydusd struct {
	pid         int
	path        string
	sockPath    string
	apiSockPath string
	sourcePath  string
	logLevel    string
	extraArgs   []string
}

func (nd *nydusd) Start(ctx context.Context, onQuit onQuitFunc) (int, error) {
	span, _ := katatrace.Trace(ctx, nd.Logger(), "Start", nydusdTracingTags)
	defer span.End()
	pid := 0

	args := []string{
		"--log-level", "info",
		"--apisock", nd.apiSockPath,
		"--sock", nd.sockPath,
	}
	if len(nd.extraArgs) > 0 {
		args = append(args, nd.extraArgs...)
	}
	cmd := exec.Command(nd.path, args...)
	stderr, err := cmd.StderrPipe()
	if err != nil {
		return pid, err
	}
	nd.Logger().WithField("path", nd.path).Info()
	nd.Logger().WithField("args", strings.Join(args, " ")).Info()
	if err := cmd.Start(); err != nil {
		return pid, err
	}
	// Monitor nydusd's stderr and stop sandbox if nydusd quits
	go func() {
		scanner := bufio.NewScanner(stderr)
		for scanner.Scan() {
			nd.Logger().WithField("source", "nydusd").Info(scanner.Text())
		}
		nd.Logger().Info("nydusd quits")
		// Wait to release resources of nydusd process
		cmd.Process.Wait()
		if onQuit != nil {
			onQuit()
		}
	}()
	if err := nd.setupPassthroughFS(); err != nil {
		return pid, err
	}
	nd.pid = cmd.Process.Pid
	return nd.pid, nil
}

func (nd *nydusd) setupPassthroughFS() error {
	if err := os.MkdirAll(nd.sourcePath, DirMode); err != nil {
		return err
	}
	nc, err := NewNydusClient(nd.apiSockPath)
	if err != nil {
		return err
	}
	nd.Logger().WithField("from", nd.sourcePath).
		WithField("dest", sharedPathInGuest).Info("prepare mount passthroughfs")

	mr := NewMountRequest(nydusPassthroughfsType, nd.sourcePath, "")
	return nc.Mount(sharedPathInGuest, mr)
}

func (nd *nydusd) MountRAFS(opt MountOption) error {
	nc, err := NewNydusClient(nd.apiSockPath)
	if err != nil {
		return err
	}
	nd.Logger().WithField("from", opt.bootstrap).
		WithField("dest", opt.mountpoint).Info("prepare mount rafs")

	mr := NewMountRequest(nydusRafsType, opt.bootstrap, opt.daemonConfig)
	return nc.Mount(opt.mountpoint, mr)
}

func (nd *nydusd) UmountRAFS(mountpoint string) error {
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
	return nil
}

func (nd *nydusd) Logger() *log.Entry {
	return virtLog.WithField("subsystem", "nydusd")
}

func (nd *nydusd) kill(ctx context.Context) (err error) {
	span, _ := katatrace.Trace(ctx, nd.Logger(), "kill", nydusdTracingTags)
	defer span.End()

	if nd.pid == 0 {
		return errors.New("invalid nydusd PID(0)")
	}
	err = syscall.Kill(nd.pid, syscall.SIGTERM)
	if err != nil {
		nd.pid = 0
	}
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

type MountOption struct {
	mountpoint   string
	bootstrap    string
	daemonConfig string
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
			Timeout:   defaultHttpClientTimeout,
			Transport: transport,
		},
	}, nil
}

func waitUntilSocketReady(sock string) error {
	b := &backoff.ExponentialBackOff{
		InitialInterval:     300 * time.Microsecond,
		RandomizationFactor: 0.5,
		Multiplier:          2,
		MaxInterval:         1 * time.Second,
		MaxElapsedTime:      5 * time.Second,
		Clock:               backoff.SystemClock,
	}
	b.Reset()
	err := backoff.Retry(func() error {
		_, err := os.Stat(sock)
		return err
	}, b)
	if err != nil {
		return errors.Wrap(err, "nydusd daemon socket not ready")
	}
	return nil
}

func buildTransport(sock string) (http.RoundTripper, error) {
	err := waitUntilSocketReady(sock)
	if err != nil {
		return nil, err
	}
	return &http.Transport{
		MaxIdleConns:          10,
		IdleConnTimeout:       10 * time.Second,
		ExpectContinueTimeout: 1 * time.Second,
		DialContext: func(ctx context.Context, _, _ string) (net.Conn, error) {
			dialer := &net.Dialer{
				Timeout:   5 * time.Second,
				KeepAlive: 5 * time.Second,
			}
			return dialer.DialContext(ctx, "unix", sock)
		},
	}, nil
}

func (c *NydusClient) CheckStatus() (DaemonInfo, error) {
	resp, err := c.httpClient.Get(fmt.Sprintf("http://unix%s", infoEndpoint))
	if err != nil {
		return DaemonInfo{}, err
	}
	defer resp.Body.Close()
	b, err := ioutil.ReadAll(resp.Body)
	if err != nil {
		return DaemonInfo{}, err
	}
	var info DaemonInfo
	if err = json.Unmarshal(b, &info); err != nil {
		return DaemonInfo{}, err
	}
	return info, nil
}

func (c *NydusClient) Mount(mountPoint string, mr *MountRequest) error {
	requestURL := fmt.Sprintf("http://unix%s?mountpoint=%s", mountEndpoint, mountPoint)
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

func (c *NydusClient) Umount(sharedMountPoint string) error {
	requestURL := fmt.Sprintf("http://unix%s?mountpoint=%s", mountEndpoint, sharedMountPoint)
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
	b, err := ioutil.ReadAll(r)
	if err != nil {
		return err
	}
	var errMessage ErrorMessage
	if err = json.Unmarshal(b, &errMessage); err != nil {
		return err
	}
	return errors.New(errMessage.Message)
}

func parseConfigs(configs []string) (string, string, error) {
	handleErr := func(msg string) (string, string, error) {
		return "", "", errors.New(msg)
	}
	if len(configs) != 2 {
		msg := fmt.Sprintf("config should has two items, %v", configs)
		return handleErr(msg)
	}

	var (
		daemonConfig string
		snapshotDir  string
	)
	for _, str := range configs {
		if strings.HasPrefix(str, configPrefix) {
			daemonConfig = strings.TrimPrefix(str, configPrefix)
		} else if strings.HasPrefix(str, snapshotPrefix) {
			snapshotDir = strings.TrimPrefix(str, snapshotPrefix)
		} else {
		}
	}
	if len(daemonConfig) == 0 || len(snapshotDir) == 0 {
		msg := fmt.Sprintf("daemonConfig or snapshotDir has wrong format, %v", configs)
		return handleErr(msg)
	}
	return daemonConfig, snapshotDir, nil
}
