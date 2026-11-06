// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"encoding/json"
	"flag"
	"fmt"
	"io"
	"os"
	sysexec "os/exec"
	"path/filepath"
	goruntime "runtime"
	"sync"
	"syscall"
	"time"

	eventstypes "github.com/containerd/containerd/api/events"
	bootapi "github.com/containerd/containerd/api/runtime/bootstrap/v1"
	taskAPI "github.com/containerd/containerd/api/runtime/task/v2"
	containerdtypes "github.com/containerd/containerd/api/types"
	"github.com/containerd/containerd/api/types/task"
	cdruntime "github.com/containerd/containerd/v2/core/runtime"
	"github.com/containerd/containerd/v2/defaults"
	"github.com/containerd/containerd/v2/pkg/namespaces"
	cdshim "github.com/containerd/containerd/v2/pkg/shim"
	"github.com/containerd/containerd/v2/pkg/shutdown"
	"github.com/containerd/containerd/v2/plugins"
	"github.com/containerd/errdefs"
	"github.com/containerd/errdefs/pkg/errgrpc"
	"github.com/containerd/plugin"
	"github.com/containerd/plugin/registry"
	"github.com/containerd/ttrpc"
	"github.com/containerd/typeurl/v2"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils/katatrace"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/oci"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/utils"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/compatoci"
	vctypes "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/opencontainers/runtime-spec/specs-go"
	"github.com/pkg/errors"
	"github.com/sirupsen/logrus"
	otelTrace "go.opentelemetry.io/otel/trace"
	"golang.org/x/sys/unix"
	emptypb "google.golang.org/protobuf/types/known/emptypb"
	"google.golang.org/protobuf/types/known/timestamppb"
)

// shimTracingTags defines tags for the trace span
var shimTracingTags = map[string]string{
	"source":  "runtime",
	"package": "containerdshim",
}

const (
	// Define the service's channel size, which is used for
	// reaping the exited processes exit state and forwarding
	// it to containerd as the containerd event format.
	bufferSize = 32

	chSize      = 128
	exitCode255 = 255

	// shimVersion is the version reported in the BootstrapResult. For
	// containerd >= 2.2 it gates supportSandboxAPIVersion (core/runtime/
	// v2/shim_manager.go): shims reporting >= 3 are assumed to serve the
	// v3 task service and the sandbox API with streaming I/O, so
	// containerd reuses the sandbox shim's connection for containers
	// without invoking "shim start", and relies on the sandbox API
	// instead of the shim-binary "delete" for cleanup. This shim serves
	// containerd.task.v2.Task and none of that, so it must report 2 —
	// the same value the legacy JSON path reports to CRI-O and
	// containerd <= 2.2, and what the pre-migration shim reported. Bump
	// only when the shim actually serves the v3 task service and the
	// sandbox API.
	shimVersion = 2
)

func init() {
	registry.Register(&plugin.Registration{
		Type: plugins.TTRPCPlugin,
		ID:   "task",
		Requires: []plugin.Type{
			plugins.EventPlugin,
			plugins.InternalPlugin,
		},
		InitFn: func(ic *plugin.InitContext) (any, error) {
			pp, err := ic.GetByID(plugins.EventPlugin, "publisher")
			if err != nil {
				return nil, err
			}
			ss, err := ic.GetByID(plugins.InternalPlugin, "shutdown")
			if err != nil {
				return nil, err
			}
			// The SDK only sets Opts.BundlePath for the "delete" action, so
			// the sandbox ID has to come from the "-id" flag it registers.
			idFlag := flag.Lookup("id")
			if idFlag == nil || idFlag.Value.String() == "" {
				return nil, fmt.Errorf("shim id flag is not set")
			}
			return New(ic.Context, idFlag.Value.String(), pp.(cdshim.Publisher), ss.(shutdown.Service).Shutdown)
		},
	})
}

var (
	empty                          = &emptypb.Empty{}
	_     taskAPI.TTRPCTaskService = taskAPI.TTRPCTaskService(&service{})
	_     cdshim.TTRPCService      = (*service)(nil)
)

// concrete virtcontainer implementation
var vci vc.VC = &vc.VCImpl{}

// shimLog is logger for shim package
var shimLog = logrus.WithFields(logrus.Fields{
	"source": "containerd-kata-shim-v2",
	"name":   "containerd-shim-v2",
})

// shimManager implements cdshim.Shim so the kata shim binary can be launched
// by containerd v2 via cdshim.RunShim.
type shimManager struct{ name string }

// NewManager returns a cdshim.Shim for use with cdshim.RunShim.
func NewManager(name string) cdshim.Shim { return &shimManager{name: name} }

func (m *shimManager) Name() string { return m.name }

// legacyBootstrapJSON returns the JSON bytes that legacy callers expect on
// stdout and a boolean indicating whether the legacy path is active.
//
// A caller is legacy when params.ContainerdVersion is empty:
//   - CRI-O sends no stdin at all; the SDK falls back to deprecated fields and
//     leaves ContainerdVersion unset.
//   - containerd <= 2.2.x writes a BootstrapParams via stdin but does not set
//     ContainerdVersion in that message.
//   - containerd >= 2.3 always sets ContainerdVersion to its own version string.
//
// The version in the JSON is fixed at 2; CRI-O and containerd <= 2.2 reject
// any value higher than that.
//
// TODO: Remove once CRI-O's ParseShimAddress supports protobuf BootstrapResult
// and the CI CRI-O version includes that fix.
// Should be helped by: https://github.com/cri-o/cri-o/pulls/10490
func legacyBootstrapJSON(params *bootapi.BootstrapParams, address string) ([]byte, bool) {
	if params.GetContainerdVersion() != "" {
		// Modern containerd (>= 2.3): let the SDK write protobuf as normal.
		return nil, false
	}

	type result struct {
		Version  int    `json:"version"`
		Address  string `json:"address"`
		Protocol string `json:"protocol"`
	}
	data, err := json.Marshal(result{Version: 2, Address: address, Protocol: "ttrpc"})
	if err != nil {
		// json.Marshal on a plain struct with only basic types cannot fail.
		panic(fmt.Sprintf("legacyBootstrapJSON: unexpected marshal error: %v", err))
	}
	return data, true
}

// writeLegacyBootstrap writes JSON to the real stdout, then swaps os.Stdout to
// /dev/null so the SDK's unconditional proto.Marshal+Write that follows our
// return is silently discarded.
func writeLegacyBootstrap(data []byte) error {
	if _, err := os.Stdout.Write(data); err != nil {
		return fmt.Errorf("write legacy bootstrap result: %w", err)
	}
	devNull, err := os.OpenFile(os.DevNull, os.O_WRONLY, 0)
	if err != nil {
		return fmt.Errorf("open /dev/null: %w", err)
	}
	os.Stdout = devNull
	return nil
}

func (m *shimManager) start(ctx context.Context, params *bootapi.BootstrapParams) (_ *bootapi.BootstrapResult, retErr error) {
	id := params.InstanceID
	containerdAddress := params.ContainerdGrpcAddress

	bundlePath, err := os.Getwd()
	if err != nil {
		return nil, err
	}

	address, err := getAddress(ctx, bundlePath, containerdAddress, id)
	if err != nil {
		return nil, err
	}
	if address != "" {
		return &bootapi.BootstrapResult{Version: shimVersion, Address: address, Protocol: "ttrpc"}, nil
	}

	cmd, err := newCommand(ctx, id, containerdAddress)
	if err != nil {
		return nil, err
	}

	socketDir := params.GetSocketDir()
	if socketDir == "" {
		socketDir = filepath.Join(defaults.DefaultStateDir, "s")
	}
	// The main ttrpc socket must always be named without the debug
	// suffix. Callers that re-run "shim start" for containers joining an
	// existing sandbox (CRI-O, and containerd without the podsandbox
	// sandboxer) recompute this address in getAddress() with debug
	// hardcoded to false, and containerd's own shims keep the main socket
	// at the non-debug hash too (the debug hash is reserved for an extra
	// pprof socket). Mixing the two leaves containerd/CRI-O dialing a
	// path where no socket exists.
	address, err = cdshim.CreateSocketAddress(ctx, socketDir, containerdAddress, id, false)
	if err != nil {
		return nil, err
	}

	socket, err := cdshim.NewSocket(address)
	if err != nil {
		if !cdshim.SocketEaddrinuse(err) {
			return nil, err
		}
		if err := cdshim.RemoveSocket(address); err != nil {
			return nil, errors.Wrap(err, "remove already used socket")
		}
		if socket, err = cdshim.NewSocket(address); err != nil {
			return nil, err
		}
	}

	defer func() {
		if retErr != nil {
			socket.Close()
			_ = cdshim.RemoveSocket(address)
		}
	}()

	f, err := socket.File()
	if err != nil {
		return nil, err
	}

	cmd.ExtraFiles = append(cmd.ExtraFiles, f)

	goruntime.LockOSThread()
	if os.Getenv("SCHED_CORE") != "" {
		if err := utils.Create(utils.ProcessGroup); err != nil {
			return nil, errors.Wrap(err, "enable sched core support")
		}
	}

	if err := setupMntNs(); err != nil {
		return nil, err
	}

	if err := cmd.Start(); err != nil {
		return nil, err
	}

	goruntime.UnlockOSThread()

	defer func() {
		if retErr != nil {
			cmd.Process.Kill()
		}
	}()

	if err = cdshim.WritePidFile("shim.pid", cmd.Process.Pid); err != nil {
		return nil, err
	}
	return &bootapi.BootstrapResult{Version: shimVersion, Address: address, Protocol: "ttrpc"}, nil
}

// Start launches the shim daemon and returns its bootstrap result.
//
// For modern callers (containerd >= 2.3, ContainerdVersion non-empty) the SDK
// serialises the returned BootstrapResult as protobuf to stdout unchanged.
//
// For legacy callers (CRI-O, containerd <= 2.2.x, ContainerdVersion empty) we
// write the JSON form of the result directly to the original os.Stdout, then
// redirect os.Stdout to /dev/null so that the SDK's unconditional protobuf
// write (pkg/shim/shim.go) is silently discarded.
func (m *shimManager) Start(ctx context.Context, params *bootapi.BootstrapParams) (*bootapi.BootstrapResult, error) {
	result, err := m.start(ctx, params)
	if err != nil {
		return nil, err
	}
	if jsonData, legacy := legacyBootstrapJSON(params, result.Address); legacy {
		if err := writeLegacyBootstrap(jsonData); err != nil {
			return nil, err
		}
	}
	return result, nil
}

// Stop implements the binary "delete" action. The v1 SDK dispatched this to
// service.Cleanup through its shimToManager adapter (containerd@v1.7
// runtime/v2/shim/shim.go); the v2 SDK calls Manager.Stop directly, so the
// cleanup itself has to happen here. containerd invokes this when the shim
// is dead or unreachable (cleanupAfterDeadShim), and for containers in
// restored sandboxes when the shim does not advertise sandbox API support.
func (m *shimManager) Stop(ctx context.Context, id string) (cdshim.StopStatus, error) {
	dr, err := cleanupBundle(ctx, id)
	if err != nil {
		return cdshim.StopStatus{}, err
	}

	status := cdshim.StopStatus{
		Pid:        int(dr.Pid),
		ExitStatus: int(dr.ExitStatus),
	}
	if dr.ExitedAt != nil {
		status.ExitedAt = dr.ExitedAt.AsTime()
	}
	return status, nil
}

func (m *shimManager) Info(ctx context.Context, _ io.Reader) (*containerdtypes.RuntimeInfo, error) {
	return &containerdtypes.RuntimeInfo{
		Name: m.name,
		Version: &containerdtypes.RuntimeVersion{
			Version:  katautils.VERSION,
			Revision: katautils.COMMIT,
		},
	}, nil
}

// New constructs the kata shim service inside the forked shim process.
func New(ctx context.Context, id string, publisher cdshim.Publisher, shutdown func()) (*service, error) {
	shimLog = shimLog.WithFields(logrus.Fields{
		"sandbox": id,
		"pid":     os.Getpid(),
	})
	// Discard the log before shim init its log output. Otherwise
	// it will output into stdio, from which containerd would like
	// to get the shim's socket address.
	logrus.SetOutput(io.Discard)
	opts := ctx.Value(cdshim.OptsKey{}).(cdshim.Opts)
	if !opts.Debug {
		logrus.SetLevel(logrus.InfoLevel)
	}
	vci.SetLogger(ctx, shimLog)
	katautils.SetLogger(ctx, shimLog, shimLog.Logger.Level)

	ns, found := namespaces.Namespace(ctx)
	if !found {
		return nil, fmt.Errorf("shim namespace cannot be empty")
	}

	s := &service{
		id:         id,
		pid:        uint32(os.Getpid()),
		ctx:        ctx,
		containers: make(map[string]*container),
		events:     make(chan interface{}, chSize),
		ec:         make(chan exit, bufferSize),
		cancel:     shutdown,
		namespace:  ns,
	}

	go s.processExits()

	forwarder := s.newEventsForwarder(ctx, publisher)
	go forwarder.forward()

	return s, nil
}

type exit struct {
	timestamp time.Time
	id        string
	execid    string
	pid       uint32
	status    int
}

// service is the shim implementation of a remote shim over GRPC
type service struct {
	sandbox vc.VCSandbox

	ctx      context.Context
	rootCtx  context.Context // root context for tracing
	rootSpan otelTrace.Span

	containers map[string]*container

	config *oci.RuntimeConfig

	monitor chan error
	ec      chan exit

	events chan interface{}

	cancel func()

	id string

	// Namespace from upper container engine
	namespace string

	mu          sync.Mutex
	eventSendMu sync.Mutex

	// agentMetrics bounds guest metrics collection and keeps capability/failure
	// state local to this sandbox.
	agentMetrics agentMetricsState

	// teardownWg tracks in-flight sandbox teardown started from wait().
	// Shutdown() waits on it so the sandbox run directory (watched by
	// kata-monitor) is removed before the shim exits, without having to
	// hold s.mu across the slow guest shutdown (which would block the
	// Delete() RPC and make containerd fall back to the `shim delete`
	// binary).
	teardownWg sync.WaitGroup

	// teardownOnce guarantees the sandbox Stop/Delete runs exactly once even
	// when both wait() (normal/early exit) and watchSandbox() (killed-VMM)
	// try to tear the sandbox down.  Sandbox.Stop/Delete are not internally
	// synchronized, so this serializes the two paths without holding s.mu
	// across the teardown.
	teardownOnce sync.Once

	// hypervisor pid, Since this shimv2 cannot get the container processes pid from VM,
	// thus for the returned values needed pid, just return the hypervisor's
	// pid directly.
	hpid uint32

	// shim's pid
	pid uint32
}

func newCommand(ctx context.Context, id, containerdAddress string) (*sysexec.Cmd, error) {
	ns, err := namespaces.NamespaceRequired(ctx)
	if err != nil {
		return nil, err
	}
	self, err := os.Executable()
	if err != nil {
		return nil, err
	}
	cwd, err := os.Getwd()
	if err != nil {
		return nil, err
	}
	args := []string{
		"-namespace", ns,
		"-address", containerdAddress,
		"-publish-binary", self,
		"-id", id,
	}
	opts := ctx.Value(cdshim.OptsKey{}).(cdshim.Opts)
	if opts.Debug {
		args = append(args, "-debug")
	}
	cmd := sysexec.Command(self, args...)
	cmd.Dir = cwd

	// Set the go max process to 2 in case the shim forks too much process
	cmd.Env = append(os.Environ(), "GOMAXPROCS=2")

	cmd.SysProcAttr = &syscall.SysProcAttr{
		Setpgid: true,
	}

	return cmd, nil
}

func setupMntNs() error {
	err := unix.Unshare(unix.CLONE_NEWNS)
	if err != nil {
		return err
	}

	err = unix.Mount("", "/", "", unix.MS_REC|unix.MS_SLAVE, "")
	if err != nil {
		err = fmt.Errorf("failed to mount with slave: %v", err)
		return err
	}

	err = unix.Mount("", "/", "", unix.MS_REC|unix.MS_SHARED, "")
	if err != nil {
		err = fmt.Errorf("failed to mount with shared: %v", err)
		return err
	}

	return nil
}

func (s *service) send(evt interface{}) {
	// for unit test, it will not initialize s.events
	if s.events != nil {
		s.events <- evt
	}
}

func (s *service) sendL(evt interface{}) {
	s.eventSendMu.Lock()
	if s.events != nil {
		s.events <- evt
	}
	s.eventSendMu.Unlock()
}

func getTopic(e interface{}) string {
	switch e.(type) {
	case *eventstypes.TaskCreate:
		return cdruntime.TaskCreateEventTopic
	case *eventstypes.TaskStart:
		return cdruntime.TaskStartEventTopic
	case *eventstypes.TaskOOM:
		return cdruntime.TaskOOMEventTopic
	case *eventstypes.TaskExit:
		return cdruntime.TaskExitEventTopic
	case *eventstypes.TaskDelete:
		return cdruntime.TaskDeleteEventTopic
	case *eventstypes.TaskExecAdded:
		return cdruntime.TaskExecAddedEventTopic
	case *eventstypes.TaskExecStarted:
		return cdruntime.TaskExecStartedEventTopic
	case *eventstypes.TaskPaused:
		return cdruntime.TaskPausedEventTopic
	case *eventstypes.TaskResumed:
		return cdruntime.TaskResumedEventTopic
	case *eventstypes.TaskCheckpointed:
		return cdruntime.TaskCheckpointedEventTopic
	default:
		shimLog.WithField("event-type", e).Warn("no topic for event type")
	}
	return cdruntime.TaskUnknownTopic
}

// cleanupBundle is the binary "delete" action: it tears down whatever the
// shim left behind for the container whose bundle is the current working
// directory (VM, network, mounts, runtime state) and returns that
// container's exit status.
func cleanupBundle(ctx context.Context, id string) (_ *taskAPI.DeleteResponse, err error) {
	span, spanCtx := katatrace.Trace(ctx, shimLog, "Cleanup", shimTracingTags)
	defer span.End()

	// Since the binary cleanup will return the DeleteResponse from stdout to
	// containerd, thus we must make sure there is no any outputs in stdout except
	// the returned response, thus here redirect the log to stderr in case there's
	// any log output to stdout.
	logrus.SetOutput(os.Stderr)

	defer func() {
		err = toGRPC(err)
	}()

	if id == "" {
		return nil, errgrpc.ToGRPCf(errdefs.ErrInvalidArgument, "the container id is empty, please specify the container id")
	}

	path, err := os.Getwd()
	if err != nil {
		return nil, err
	}

	ociSpec, err := compatoci.ParseConfigJSON(path)
	if err != nil {
		return nil, err
	}

	containerType, err := oci.ContainerType(ociSpec)
	if err != nil {
		return nil, err
	}

	switch containerType {
	case vc.PodSandbox, vc.SingleContainer:
		err = cleanupContainer(spanCtx, id, id, path)
		if err != nil {
			return nil, err
		}
	case vc.PodContainer:
		sandboxID, err := oci.SandboxID(ociSpec)
		if err != nil {
			return nil, err
		}

		err = cleanupContainer(spanCtx, sandboxID, id, path)
		if err != nil {
			return nil, err
		}
	}

	return &taskAPI.DeleteResponse{
		ExitedAt:   timestamppb.New(time.Now()),
		ExitStatus: 128 + uint32(unix.SIGKILL),
	}, nil
}

// Create a new sandbox or container with the underlying OCI runtime
func (s *service) Create(ctx context.Context, r *taskAPI.CreateTaskRequest) (_ *taskAPI.CreateTaskResponse, err error) {
	shimLog.WithField("container", r.ID).Debug("Create() start")
	defer shimLog.WithField("container", r.ID).Debug("Create() end")
	start := time.Now()
	defer func() {
		err = toGRPC(err)
		rpcDurationsHistogram.WithLabelValues("create").Observe(float64(time.Since(start).Nanoseconds() / int64(time.Millisecond)))
	}()

	s.mu.Lock()
	defer s.mu.Unlock()

	if err := katautils.VerifyContainerID(r.ID); err != nil {
		return nil, err
	}

	type Result struct {
		container *container
		err       error
	}
	ch := make(chan Result, 1)
	go func() {
		container, err := create(ctx, s, r)
		ch <- Result{container, err}
	}()

	select {
	case <-ctx.Done():
		return nil, errors.Errorf("create container timeout: %v", r.ID)
	case res := <-ch:
		if res.err != nil {
			return nil, res.err
		}
		container := res.container
		container.status = task.Status_CREATED

		s.containers[r.ID] = container

		s.send(&eventstypes.TaskCreate{
			ContainerID: r.ID,
			Bundle:      r.Bundle,
			Rootfs:      r.Rootfs,
			IO: &eventstypes.TaskIO{
				Stdin:    r.Stdin,
				Stdout:   r.Stdout,
				Stderr:   r.Stderr,
				Terminal: r.Terminal,
			},
			Checkpoint: r.Checkpoint,
			Pid:        s.hpid,
		})

		return &taskAPI.CreateTaskResponse{
			Pid: s.hpid,
		}, nil
	}
}

// Start a process
func (s *service) Start(ctx context.Context, r *taskAPI.StartRequest) (_ *taskAPI.StartResponse, err error) {
	shimLog.WithField("container", r.ID).Debug("Start() start")
	defer shimLog.WithField("container", r.ID).Debug("Start() end")
	span, spanCtx := katatrace.Trace(s.rootCtx, shimLog, "Start", shimTracingTags)
	defer span.End()

	start := time.Now()
	defer func() {
		err = toGRPC(err)
		rpcDurationsHistogram.WithLabelValues("start").Observe(float64(time.Since(start).Nanoseconds() / int64(time.Millisecond)))
	}()

	s.mu.Lock()
	defer s.mu.Unlock()

	c, err := s.getContainer(r.ID)
	if err != nil {
		return nil, err
	}

	// hold the send lock so that the start events are sent before any exit events in the error case
	s.eventSendMu.Lock()
	defer s.eventSendMu.Unlock()

	// start a container
	if r.ExecID == "" {
		err = startContainer(spanCtx, s, c)
		if err != nil {
			return nil, errgrpc.ToGRPC(err)
		}
		s.send(&eventstypes.TaskStart{
			ContainerID: c.id,
			Pid:         s.hpid,
		})
	} else {
		// start an exec
		_, err = startExec(spanCtx, s, r.ID, r.ExecID)
		if err != nil {
			return nil, errgrpc.ToGRPC(err)
		}
		s.send(&eventstypes.TaskExecStarted{
			ContainerID: c.id,
			ExecID:      r.ExecID,
			Pid:         s.hpid,
		})
	}

	return &taskAPI.StartResponse{
		Pid: s.hpid,
	}, nil
}

// Delete the initial process and container
func (s *service) Delete(ctx context.Context, r *taskAPI.DeleteRequest) (_ *taskAPI.DeleteResponse, err error) {
	shimLog.WithField("container", r.ID).Debug("Delete() start")
	defer shimLog.WithField("container", r.ID).Debug("Delete() end")
	span, spanCtx := katatrace.Trace(s.rootCtx, shimLog, "Delete", shimTracingTags)
	defer span.End()

	start := time.Now()
	defer func() {
		err = toGRPC(err)
		rpcDurationsHistogram.WithLabelValues("delete").Observe(float64(time.Since(start).Nanoseconds() / int64(time.Millisecond)))
	}()

	s.mu.Lock()
	defer s.mu.Unlock()

	c, err := s.getContainer(r.ID)
	if err != nil {
		return nil, err
	}

	if r.ExecID == "" {
		if err = deleteContainer(spanCtx, s, c); err != nil {
			return nil, err
		}

		s.send(&eventstypes.TaskDelete{
			ContainerID: c.id,
			Pid:         s.hpid,
			ExitStatus:  c.exit,
			ExitedAt:    timestamppb.New(c.exitTime),
		})

		return &taskAPI.DeleteResponse{
			ExitStatus: c.exit,
			ExitedAt:   timestamppb.New(c.exitTime),
			Pid:        s.hpid,
		}, nil
	}
	// deal with the exec case
	execs, err := c.getExec(r.ExecID)
	if err != nil {
		return nil, err
	}

	c.deleteExec(r.ExecID)

	return &taskAPI.DeleteResponse{
		ExitStatus: uint32(execs.exitCode),
		ExitedAt:   timestamppb.New(execs.exitTime),
		Pid:        s.hpid,
	}, nil
}

// Exec an additional process inside the container
func (s *service) Exec(ctx context.Context, r *taskAPI.ExecProcessRequest) (_ *emptypb.Empty, err error) {
	shimLog.WithField("container", r.ID).Debug("Exec() start")
	defer shimLog.WithField("container", r.ID).Debug("Exec() end")
	span, _ := katatrace.Trace(s.rootCtx, shimLog, "Exec", shimTracingTags)
	defer span.End()

	start := time.Now()
	defer func() {
		rpcDurationsHistogram.WithLabelValues("exec").Observe(float64(time.Since(start).Nanoseconds() / int64(time.Millisecond)))
		err = toGRPC(err)
	}()

	s.mu.Lock()
	defer s.mu.Unlock()

	c, err := s.getContainer(r.ID)
	if err != nil {
		return nil, err
	}

	if e, _ := c.getExec(r.ExecID); e != nil {
		return nil, errgrpc.ToGRPCf(errdefs.ErrAlreadyExists, "id %s", r.ExecID)
	}

	execs, err := newExec(c, r.Stdin, r.Stdout, r.Stderr, r.Terminal, r.Spec)
	if err != nil {
		return nil, errgrpc.ToGRPC(err)
	}

	c.setExec(r.ExecID, execs)

	s.send(&eventstypes.TaskExecAdded{
		ContainerID: c.id,
		ExecID:      r.ExecID,
	})

	return empty, nil
}

// ResizePty of a process
func (s *service) ResizePty(ctx context.Context, r *taskAPI.ResizePtyRequest) (_ *emptypb.Empty, err error) {
	shimLog.WithField("container", r.ID).Debug("ResizePty() start")
	defer shimLog.WithField("container", r.ID).Debug("ResizePty() end")
	span, spanCtx := katatrace.Trace(s.rootCtx, shimLog, "ResizePty", shimTracingTags)
	defer span.End()

	start := time.Now()
	defer func() {
		err = toGRPC(err)
		rpcDurationsHistogram.WithLabelValues("resize_pty").Observe(float64(time.Since(start).Nanoseconds() / int64(time.Millisecond)))
	}()

	s.mu.Lock()
	defer s.mu.Unlock()

	c, err := s.getContainer(r.ID)
	if err != nil {
		return nil, err
	}

	processID := c.id
	if r.ExecID != "" {
		execs, err := c.getExec(r.ExecID)
		if err != nil {
			return nil, err
		}
		execs.tty.height = r.Height
		execs.tty.width = r.Width

		processID = execs.id

	}
	err = s.sandbox.WinsizeProcess(spanCtx, c.id, processID, r.Height, r.Width)
	if err != nil {
		return nil, err
	}

	return empty, err
}

// State returns runtime state information for a process
func (s *service) State(ctx context.Context, r *taskAPI.StateRequest) (_ *taskAPI.StateResponse, err error) {
	shimLog.WithField("container", r.ID).Debug("State() start")
	defer shimLog.WithField("container", r.ID).Debug("State() end")
	span, _ := katatrace.Trace(s.rootCtx, shimLog, "State", shimTracingTags)
	defer span.End()

	start := time.Now()
	defer func() {
		err = toGRPC(err)
		rpcDurationsHistogram.WithLabelValues("state").Observe(float64(time.Since(start).Nanoseconds() / int64(time.Millisecond)))
	}()

	s.mu.Lock()
	defer s.mu.Unlock()

	c, err := s.getContainer(r.ID)
	if err != nil {
		return nil, err
	}

	if r.ExecID == "" {
		return &taskAPI.StateResponse{
			ID:         c.id,
			Bundle:     c.bundle,
			Pid:        s.hpid,
			Status:     c.status,
			Stdin:      c.stdin,
			Stdout:     c.stdout,
			Stderr:     c.stderr,
			Terminal:   c.terminal,
			ExitStatus: c.exit,
			ExitedAt:   timestamppb.New(c.exitTime),
		}, nil
	}

	// deal with exec case
	execs, err := c.getExec(r.ExecID)
	if err != nil {
		return nil, err
	}

	return &taskAPI.StateResponse{
		ID:         execs.id,
		Bundle:     c.bundle,
		Pid:        s.hpid,
		Status:     execs.status,
		Stdin:      execs.tty.stdin,
		Stdout:     execs.tty.stdout,
		Stderr:     execs.tty.stderr,
		Terminal:   execs.tty.terminal,
		ExitStatus: uint32(execs.exitCode),
		ExitedAt:   timestamppb.New(execs.exitTime),
	}, nil
}

// Pause the container
func (s *service) Pause(ctx context.Context, r *taskAPI.PauseRequest) (_ *emptypb.Empty, err error) {
	shimLog.WithField("container", r.ID).Debug("Pause() start")
	defer shimLog.WithField("container", r.ID).Debug("Pause() end")
	span, spanCtx := katatrace.Trace(s.rootCtx, shimLog, "Pause", shimTracingTags)
	defer span.End()

	start := time.Now()
	defer func() {
		err = toGRPC(err)
		rpcDurationsHistogram.WithLabelValues("pause").Observe(float64(time.Since(start).Nanoseconds() / int64(time.Millisecond)))
	}()

	s.mu.Lock()
	defer s.mu.Unlock()

	c, err := s.getContainer(r.ID)
	if err != nil {
		return nil, err
	}

	c.status = task.Status_PAUSING

	err = s.sandbox.PauseContainer(spanCtx, r.ID)
	if err == nil {
		c.status = task.Status_PAUSED
		s.send(&eventstypes.TaskPaused{
			ContainerID: c.id,
		})
		return empty, nil
	}

	if status, err := s.getContainerStatus(c.id); err != nil {
		c.status = task.Status_UNKNOWN
	} else {
		c.status = status
	}

	return empty, err
}

// Resume the container
func (s *service) Resume(ctx context.Context, r *taskAPI.ResumeRequest) (_ *emptypb.Empty, err error) {
	shimLog.WithField("container", r.ID).Debug("Resume() start")
	defer shimLog.WithField("container", r.ID).Debug("Resume() end")
	span, spanCtx := katatrace.Trace(s.rootCtx, shimLog, "Resume", shimTracingTags)
	defer span.End()

	start := time.Now()
	defer func() {
		err = toGRPC(err)
		rpcDurationsHistogram.WithLabelValues("resume").Observe(float64(time.Since(start).Nanoseconds() / int64(time.Millisecond)))
	}()

	s.mu.Lock()
	defer s.mu.Unlock()

	c, err := s.getContainer(r.ID)
	if err != nil {
		return nil, err
	}

	err = s.sandbox.ResumeContainer(spanCtx, c.id)
	if err == nil {
		c.status = task.Status_RUNNING
		s.send(&eventstypes.TaskResumed{
			ContainerID: c.id,
		})
		return empty, nil
	}

	if status, err := s.getContainerStatus(c.id); err != nil {
		c.status = task.Status_UNKNOWN
	} else {
		c.status = status
	}

	return empty, err
}

// Kill a process with the provided signal
func (s *service) Kill(ctx context.Context, r *taskAPI.KillRequest) (_ *emptypb.Empty, err error) {
	shimLog.WithField("container", r.ID).Debug("Kill() start")
	defer shimLog.WithField("container", r.ID).Debug("Kill() end")
	span, spanCtx := katatrace.Trace(s.rootCtx, shimLog, "Kill", shimTracingTags)
	defer span.End()

	start := time.Now()
	defer func() {
		err = toGRPC(err)
		rpcDurationsHistogram.WithLabelValues("kill").Observe(float64(time.Since(start).Nanoseconds() / int64(time.Millisecond)))
	}()

	s.mu.Lock()
	defer s.mu.Unlock()

	signum := syscall.Signal(r.Signal)

	c, err := s.getContainer(r.ID)
	if err != nil {
		return nil, err
	}

	processStatus := c.status
	processID := c.id
	if r.ExecID != "" {
		execs, err := c.getExec(r.ExecID)
		if err != nil {
			return nil, err
		}
		processID = execs.id
		if processID == "" {
			shimLog.WithFields(logrus.Fields{
				"sandbox":   s.sandbox.ID(),
				"container": c.id,
				"exec-id":   r.ExecID,
			}).Debug("Id of exec process to be signalled is empty")
			return empty, errors.New("The exec process does not exist")
		}
		processStatus = execs.status
	} else {
		r.All = true
	}

	// According to CRI specs, kubelet will call StopPodSandbox()
	// at least once before calling RemovePodSandbox, and this call
	// is idempotent, and must not return an error if all relevant
	// resources have already been reclaimed. And in that call it will
	// send a SIGKILL signal first to try to stop the container, thus
	// once the container has terminated, here should ignore this signal
	// and return directly.
	if (signum == syscall.SIGKILL || signum == syscall.SIGTERM) && processStatus == task.Status_STOPPED {
		shimLog.WithFields(logrus.Fields{
			"sandbox":   s.sandbox.ID(),
			"container": c.id,
			"exec-id":   r.ExecID,
		}).Debug("process has already stopped")
		return empty, nil
	}

	return empty, s.sandbox.SignalProcess(spanCtx, c.id, processID, signum, r.All)
}

// Pids returns all pids inside the container
// Since for kata, it cannot get the process's pid from VM,
// thus only return the hypervisor's pid directly.
func (s *service) Pids(ctx context.Context, r *taskAPI.PidsRequest) (_ *taskAPI.PidsResponse, err error) {
	shimLog.WithField("container", r.ID).Debug("Pids() start")
	defer shimLog.WithField("container", r.ID).Debug("Pids() end")
	span, _ := katatrace.Trace(s.rootCtx, shimLog, "Pids", shimTracingTags)
	defer span.End()

	var processes []*task.ProcessInfo

	start := time.Now()
	defer func() {
		err = toGRPC(err)
		rpcDurationsHistogram.WithLabelValues("pids").Observe(float64(time.Since(start).Nanoseconds() / int64(time.Millisecond)))
	}()

	pInfo := task.ProcessInfo{
		Pid: s.hpid,
	}
	processes = append(processes, &pInfo)

	return &taskAPI.PidsResponse{
		Processes: processes,
	}, nil
}

// CloseIO of a process
func (s *service) CloseIO(ctx context.Context, r *taskAPI.CloseIORequest) (_ *emptypb.Empty, err error) {
	shimLog.WithField("container", r.ID).Debug("CloseIO() start")
	defer shimLog.WithField("container", r.ID).Debug("CloseIO() end")
	span, _ := katatrace.Trace(s.rootCtx, shimLog, "CloseIO", shimTracingTags)
	defer span.End()

	start := time.Now()
	defer func() {
		err = toGRPC(err)
		rpcDurationsHistogram.WithLabelValues("close_io").Observe(float64(time.Since(start).Nanoseconds() / int64(time.Millisecond)))
	}()

	s.mu.Lock()
	defer s.mu.Unlock()

	c, err := s.getContainer(r.ID)
	if err != nil {
		return nil, err
	}

	stdin := c.stdinPipe
	stdinCloser := c.stdinCloser

	if r.ExecID != "" {
		execs, err := c.getExec(r.ExecID)
		if err != nil {
			return nil, err
		}
		stdin = execs.stdinPipe
		stdinCloser = execs.stdinCloser
	}

	// wait until the stdin io copy terminated, otherwise
	// some contents would not be forwarded to the process.
	<-stdinCloser
	if err := stdin.Close(); err != nil {
		return nil, errors.Wrap(err, "close stdin")
	}

	return empty, nil
}

// Checkpoint the container
func (s *service) Checkpoint(ctx context.Context, r *taskAPI.CheckpointTaskRequest) (_ *emptypb.Empty, err error) {
	shimLog.WithField("container", r.ID).Debug("Checkpoint() start")
	defer shimLog.WithField("container", r.ID).Debug("Checkpoint() end")
	span, _ := katatrace.Trace(s.rootCtx, shimLog, "Checkpoint", shimTracingTags)
	defer span.End()

	start := time.Now()
	defer func() {
		err = toGRPC(err)
		rpcDurationsHistogram.WithLabelValues("checkpoint").Observe(float64(time.Since(start).Nanoseconds() / int64(time.Millisecond)))
	}()

	return nil, errgrpc.ToGRPCf(errdefs.ErrNotImplemented, "service Checkpoint")
}

// Connect returns shim information such as the shim's pid
func (s *service) Connect(ctx context.Context, r *taskAPI.ConnectRequest) (_ *taskAPI.ConnectResponse, err error) {
	shimLog.WithField("container", r.ID).Debug("Connect() start")
	defer shimLog.WithField("container", r.ID).Debug("Connect() end")
	span, _ := katatrace.Trace(s.rootCtx, shimLog, "Connect", shimTracingTags)
	defer span.End()

	start := time.Now()
	defer func() {
		err = toGRPC(err)
		rpcDurationsHistogram.WithLabelValues("connect").Observe(float64(time.Since(start).Nanoseconds() / int64(time.Millisecond)))
	}()

	s.mu.Lock()
	defer s.mu.Unlock()

	return &taskAPI.ConnectResponse{
		ShimPid: s.pid,
		// Since kata cannot get the container's pid in VM, thus only return the hypervisor's pid.
		TaskPid: s.hpid,
	}, nil
}

// RegisterTTRPC registers the task service on the ttrpc server.
// This satisfies cdshim.TTRPCService so RunShim picks up the service.
func (s *service) RegisterTTRPC(server *ttrpc.Server) error {
	taskAPI.RegisterTTRPCTaskService(server, s)
	return nil
}

func (s *service) Shutdown(ctx context.Context, r *taskAPI.ShutdownRequest) (_ *emptypb.Empty, err error) {
	shimLog.WithField("container", r.ID).Debug("Shutdown() start")
	defer shimLog.WithField("container", r.ID).Debug("Shutdown() end")
	span, _ := katatrace.Trace(s.rootCtx, shimLog, "Shutdown", shimTracingTags)

	start := time.Now()
	defer func() {
		err = toGRPC(err)
		rpcDurationsHistogram.WithLabelValues("shutdown").Observe(float64(time.Since(start).Nanoseconds() / int64(time.Millisecond)))
	}()

	s.mu.Lock()
	if len(s.containers) != 0 {
		s.mu.Unlock()

		span.End()
		s.rootSpan.End()
		katatrace.StopTracing(s.rootCtx)

		return empty, nil
	}
	s.mu.Unlock()

	// Wait for any in-flight sandbox teardown started from wait() to finish
	// before we exit.  The teardown runs without holding s.mu (so it doesn't
	// block the Delete() RPC), so we cannot rely on s.mu above to serialize
	// with it.  Blocking here guarantees the sandbox run directory (watched
	// by kata-monitor) and the CRI state are gone before the shim exits.
	s.teardownWg.Wait()

	span.End()
	katatrace.StopTracing(s.rootCtx)

	s.cancel()

	// Since we only send an shutdown qmp command to qemu when do stopSandbox, and
	// didn't wait until qemu process's exit, thus we'd better to make sure it had
	// exited when shimv2 terminated. Thus here to do the last cleanup of the hypervisor.
	syscall.Kill(int(s.hpid), syscall.SIGKILL)

	// os.Exit() will terminate program immediately, the defer functions won't be executed,
	// so we add defer functions again before os.Exit().
	// Refer to https://pkg.go.dev/os#Exit
	shimLog.WithField("container", r.ID).Debug("Shutdown() end")
	rpcDurationsHistogram.WithLabelValues("shutdown").Observe(float64(time.Since(start).Nanoseconds() / int64(time.Millisecond)))

	os.Exit(0)

	// This will never be called, but this is only there to make sure the
	// program can compile.
	return empty, nil
}

func (s *service) Stats(ctx context.Context, r *taskAPI.StatsRequest) (_ *taskAPI.StatsResponse, err error) {
	shimLog.WithField("container", r.ID).Debug("Stats() start")
	defer shimLog.WithField("container", r.ID).Debug("Stats() end")
	span, spanCtx := katatrace.Trace(s.rootCtx, shimLog, "Stats", shimTracingTags)
	defer span.End()

	start := time.Now()
	defer func() {
		err = toGRPC(err)
		rpcDurationsHistogram.WithLabelValues("stats").Observe(float64(time.Since(start).Nanoseconds() / int64(time.Millisecond)))
	}()

	s.mu.Lock()
	defer s.mu.Unlock()

	c, err := s.getContainer(r.ID)
	if err != nil {
		return nil, err
	}

	data, err := marshalMetrics(spanCtx, s, c.id)
	if err != nil {
		return nil, err
	}

	return &taskAPI.StatsResponse{
		Stats: data,
	}, nil
}

// Update a running container
func (s *service) Update(ctx context.Context, r *taskAPI.UpdateTaskRequest) (_ *emptypb.Empty, err error) {
	shimLog.WithField("container", r.ID).Debug("Update() start")
	defer shimLog.WithField("container", r.ID).Debug("Update() end")
	span, spanCtx := katatrace.Trace(s.rootCtx, shimLog, "Update", shimTracingTags)
	defer span.End()

	start := time.Now()
	defer func() {
		err = toGRPC(err)
		rpcDurationsHistogram.WithLabelValues("update").Observe(float64(time.Since(start).Nanoseconds() / int64(time.Millisecond)))
	}()

	s.mu.Lock()
	defer s.mu.Unlock()

	var resources *specs.LinuxResources
	v, err := typeurl.UnmarshalAny(r.Resources)
	if err != nil {
		return nil, err
	}
	resources, ok := v.(*specs.LinuxResources)
	if !ok {
		return nil, errgrpc.ToGRPCf(errdefs.ErrInvalidArgument, "Invalid resources type for %s", s.id)
	}

	err = s.sandbox.UpdateContainer(spanCtx, r.ID, *resources)
	if err != nil {
		return nil, errgrpc.ToGRPC(err)
	}

	return empty, nil
}

// Wait for a process to exit
func (s *service) Wait(ctx context.Context, r *taskAPI.WaitRequest) (_ *taskAPI.WaitResponse, err error) {
	shimLog.WithField("container", r.ID).Debug("Wait() start")
	defer shimLog.WithField("container", r.ID).Debug("Wait() end")
	span, _ := katatrace.Trace(s.rootCtx, shimLog, "Wait", shimTracingTags)
	defer span.End()

	var ret uint32

	start := time.Now()
	defer func() {
		err = toGRPC(err)
		rpcDurationsHistogram.WithLabelValues("wait").Observe(float64(time.Since(start).Nanoseconds() / int64(time.Millisecond)))
	}()

	s.mu.Lock()
	c, err := s.getContainer(r.ID)
	s.mu.Unlock()

	if err != nil {
		return nil, err
	}

	// wait for container
	if r.ExecID == "" {
		ret = <-c.exitCh

		// refill the exitCh with the container process's exit code in case
		// there were other waits on this process.
		c.exitCh <- ret
	} else { // wait for exec
		execs, err := c.getExec(r.ExecID)
		if err != nil {
			return nil, err
		}
		ret = <-execs.exitCh

		// refill the exitCh with the exec process's exit code in case
		// there were other waits on this process.
		execs.exitCh <- ret
	}

	return &taskAPI.WaitResponse{
		ExitStatus: ret,
		ExitedAt:   timestamppb.New(c.exitTime),
	}, nil
}

func (s *service) processExits() {
	for e := range s.ec {
		s.checkProcesses(e)
	}
}

func (s *service) checkProcesses(e exit) {
	s.mu.Lock()
	defer s.mu.Unlock()

	id := e.execid
	if id == "" {
		id = e.id
	}

	s.sendL(&eventstypes.TaskExit{
		ContainerID: e.id,
		ID:          id,
		Pid:         e.pid,
		ExitStatus:  uint32(e.status),
		ExitedAt:    timestamppb.New(e.timestamp),
	})
}

func (s *service) getContainer(id string) (*container, error) {
	c := s.containers[id]

	if c == nil {
		return nil, errgrpc.ToGRPCf(errdefs.ErrNotFound, "container does not exist %s", id)
	}

	return c, nil
}

func (s *service) getContainerStatus(containerID string) (task.Status, error) {
	cStatus, err := s.sandbox.StatusContainer(containerID)
	if err != nil {
		return task.Status_UNKNOWN, err
	}

	var status task.Status
	switch cStatus.State.State {
	case vctypes.StateReady:
		status = task.Status_CREATED
	case vctypes.StateRunning:
		status = task.Status_RUNNING
	case vctypes.StatePaused:
		status = task.Status_PAUSED
	case vctypes.StateStopped:
		status = task.Status_STOPPED
	}

	return status, nil
}
