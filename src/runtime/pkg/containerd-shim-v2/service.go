// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"os"
	sysexec "os/exec"
	goruntime "runtime"
	"runtime/debug"
	"sync"
	"syscall"
	"time"

	eventstypes "github.com/containerd/containerd/api/events"
	"github.com/containerd/containerd/api/types/task"
	"github.com/containerd/containerd/errdefs"
	"github.com/containerd/containerd/namespaces"
	cdruntime "github.com/containerd/containerd/runtime"
	cdshim "github.com/containerd/containerd/runtime/v2/shim"
	taskAPI "github.com/containerd/containerd/runtime/v2/task"
	ptypes "github.com/gogo/protobuf/types"
	"github.com/pkg/errors"
	"github.com/sirupsen/logrus"
	otelTrace "go.opentelemetry.io/otel/trace"
	"golang.org/x/sys/unix"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils/katatrace"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/oci"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/utils"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/compatoci"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
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
)

var (
	empty                     = &ptypes.Empty{}
	_     taskAPI.TaskService = (taskAPI.TaskService)(&service{})
)

// concrete virtcontainer implementation
var vci vc.VC = &vc.VCImpl{}

// shimLog is logger for shim package
var shimLog = logrus.WithFields(logrus.Fields{
	"source": "containerd-kata-shim-v2",
	"name":   "containerd-shim-v2",
})

// New returns a new shim service that can be used via GRPC
func New(ctx context.Context, id string, publisher cdshim.Publisher, shutdown func()) (cdshim.Shim, error) {
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
		logrus.SetLevel(logrus.WarnLevel)
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

	// hypervisor pid, Since this shimv2 cannot get the container processes pid from VM,
	// thus for the returned values needed pid, just return the hypervisor's
	// pid directly.
	hpid uint32

	// shim's pid
	pid uint32
}

func newCommand(ctx context.Context, id, containerdBinary, containerdAddress string) (*sysexec.Cmd, error) {
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
		"-publish-binary", containerdBinary,
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

// StartShim is a binary call that starts a kata shimv2 service which will
// implement the ShimV2 APIs such as create/start/update etc containers.
func (s *service) StartShim(ctx context.Context, opts cdshim.StartOpts) (_ string, retErr error) {
	logrus.WithFields(logrus.Fields{"func": "StartShim"}).Info("Service Call")
	bundlePath, err := os.Getwd()
	if err != nil {
		return "", err
	}

	address, err := getAddress(ctx, bundlePath, opts.Address, opts.ID)
	if err != nil {
		return "", err
	}
	if address != "" {
		if err := cdshim.WriteAddress("address", address); err != nil {
			return "", err
		}
		return address, nil
	}

	cmd, err := newCommand(ctx, opts.ID, opts.ContainerdBinary, opts.Address)
	if err != nil {
		return "", err
	}

	address, err = cdshim.SocketAddress(ctx, opts.Address, opts.ID)
	if err != nil {
		return "", err
	}

	socket, err := cdshim.NewSocket(address)

	if err != nil {
		if !cdshim.SocketEaddrinuse(err) {
			return "", err
		}
		if err := cdshim.RemoveSocket(address); err != nil {
			return "", errors.Wrap(err, "remove already used socket")
		}
		if socket, err = cdshim.NewSocket(address); err != nil {
			return "", err
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
		return "", err
	}

	cmd.ExtraFiles = append(cmd.ExtraFiles, f)

	goruntime.LockOSThread()
	if os.Getenv("SCHED_CORE") != "" {
		if err := utils.Create(utils.ProcessGroup); err != nil {
			return "", errors.Wrap(err, "enable sched core support")
		}
	}

	if err := cmd.Start(); err != nil {
		return "", err
	}

	goruntime.UnlockOSThread()

	defer func() {
		if retErr != nil {
			cmd.Process.Kill()
		}
	}()

	if err = cdshim.WritePidFile("shim.pid", cmd.Process.Pid); err != nil {
		return "", err
	}
	if err = cdshim.WriteAddress("address", address); err != nil {
		return "", err
	}
	return address, nil
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

// Cleanup is a binary call that cleans up resources used by the shim
func (s *service) Cleanup(ctx context.Context) (_ *taskAPI.DeleteResponse, err error) {
	logrus.WithFields(logrus.Fields{"func": "Cleanup"}).Info("Service Call")
	span, spanCtx := katatrace.Trace(s.rootCtx, shimLog, "Cleanup", shimTracingTags)
	defer span.End()
	logrus.Info("at Cleanup")

	debug.PrintStack()

	//Since the binary cleanup will return the DeleteResponse from stdout to
	//containerd, thus we must make sure there is no any outputs in stdout except
	//the returned response, thus here redirect the log to stderr in case there's
	//any log output to stdout.
	logrus.SetOutput(os.Stderr)

	defer func() {
		err = toGRPC(err)
	}()

	if s.id == "" {
		logrus.Info("id empty")
		return nil, errdefs.ToGRPCf(errdefs.ErrInvalidArgument, "the container id is empty, please specify the container id")
	}

	path, err := os.Getwd()
	if err != nil {
		logrus.Info("Getwd")
		return nil, err
	}
	logrus.Info("calling parse")
	ociSpec, err := compatoci.ParseConfigJSON(path)
	if err != nil {
		logrus.Info("error parsing")
		return nil, err
	}

	containerType, err := oci.ContainerType(ociSpec)
	if err != nil {
		return nil, err
	}

	switch containerType {
	case vc.PodSandbox, vc.SingleContainer:
		logrus.Info("Clean up 1")
		err = cleanupContainer(spanCtx, s.id, s.id, path)
		if err != nil {
			return nil, err
		}
	case vc.PodContainer:
		sandboxID, err := oci.SandboxID(ociSpec)
		if err != nil {
			return nil, err
		}
		logrus.Info("Clean up 2")
		err = cleanupContainer(spanCtx, sandboxID, s.id, path)
		if err != nil {
			return nil, err
		}
	}

	return &taskAPI.DeleteResponse{
		ExitedAt:   time.Now(),
		ExitStatus: 128 + uint32(unix.SIGKILL),
	}, nil
}

// Create a new sandbox or container with the underlying OCI runtime
func (s *service) Create(ctx context.Context, r *taskAPI.CreateTaskRequest) (_ *taskAPI.CreateTaskResponse, err error) {
	logrus.WithFields(logrus.Fields{"func": "Create", "ID": r.ID}).Info("Service Call")
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
		logrus.Info("Create 1")
		return nil, err
	}

	type Result struct {
		container *container
		err       error
	}
	ch := make(chan Result, 1)
	go func() {
		container, err := create(ctx, s, r) // succeeds for podSandbox but fails for podContainer
		ch <- Result{container, err}
	}()

	select {
	case <-ctx.Done():
		logrus.Info("Create 2")
		return nil, errors.Errorf("create container timeout: %v", r.ID)
	case res := <-ch:
		container := res.container
		if res.err != nil {
			logrus.WithError(res.err).Info("Create 3")
			container, _ = newContainer(nil, r, virtcontainers.PodContainer, nil, true) // res.container   !! only create a fake container when create function fails
			// return nil, res.err
		}

		container.status = task.StatusCreated

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

		logrus.Info("Create 4")
		return &taskAPI.CreateTaskResponse{
			Pid: s.hpid,
		}, nil
	}
}

// Start a process
func (s *service) Start(ctx context.Context, r *taskAPI.StartRequest) (_ *taskAPI.StartResponse, err error) {
	logrus.WithFields(logrus.Fields{"func": "Start", "ID": r.ID}).Info("Service Call")
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

	// temp
	_ = spanCtx

	c, err := s.getContainer(r.ID)

	if err != nil {
		return nil, err
	}

	// pretend the container is actually running
	c.status = task.StatusRunning

	// hold the send lock so that the start events are sent before any exit events in the error case
	s.eventSendMu.Lock()
	defer s.eventSendMu.Unlock()

	// do no want to start container inside of vm

	// start a container
	// if r.ExecID == "" {
	// 	err = startContainer(spanCtx, s, c)
	// 	if err != nil {
	// 		return nil, errdefs.ToGRPC(err)
	// 	}
	// 	s.send(&eventstypes.TaskStart{
	// 		ContainerID: c.id,
	// 		Pid:         s.hpid,
	// 	})
	// } else {
	// 	//start an exec
	// 	_, err = startExec(spanCtx, s, r.ID, r.ExecID)
	// 	if err != nil {
	// 		return nil, errdefs.ToGRPC(err)
	// 	}
	// 	s.send(&eventstypes.TaskExecStarted{
	// 		ContainerID: c.id,
	// 		ExecID:      r.ExecID,
	// 		Pid:         s.hpid,
	// 	})
	// }

	// moved from inside the above if statement
	s.send(&eventstypes.TaskStart{
		ContainerID: c.id,
		Pid:         s.hpid,
	})

	return &taskAPI.StartResponse{
		Pid: s.hpid,
	}, nil
}

// Delete the initial process and container
func (s *service) Delete(ctx context.Context, r *taskAPI.DeleteRequest) (_ *taskAPI.DeleteResponse, err error) {
	logrus.WithFields(logrus.Fields{"func": "Delete", "ID": r.ID}).Info("Service Call")
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

	// temp
	_ = spanCtx

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
			ExitedAt:    c.exitTime,
		})

		// 	return &taskAPI.DeleteResponse{
		// 		ExitStatus: c.exit,
		// 		ExitedAt:   c.exitTime,
		// 		Pid:        s.hpid,
		// 	}, nil
		// }
		// //deal with the exec case
		// execs, err := c.getExec(r.ExecID)
		// if err != nil {
		// 	return nil, err
	}

	delete(c.execs, r.ExecID)

	return &taskAPI.DeleteResponse{
		ExitStatus: 0,          // uint32(execs.exitCode),
		ExitedAt:   time.Now(), // execs.exitTime,
		Pid:        s.hpid,
	}, nil
}

// Exec an additional process inside the container
func (s *service) Exec(ctx context.Context, r *taskAPI.ExecProcessRequest) (_ *ptypes.Empty, err error) {
	logrus.WithFields(logrus.Fields{"func": "Exec", "ID": r.ID}).Info("Service Call")
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

	// TODO trace getContainer callers
	c, err := s.getContainer(r.ID)
	if err != nil {
		return nil, err
	}

	if execs := c.execs[r.ExecID]; execs != nil {
		return nil, errdefs.ToGRPCf(errdefs.ErrAlreadyExists, "id %s", r.ExecID)
	}

	execs, err := newExec(c, r.Stdin, r.Stdout, r.Stderr, r.Terminal, r.Spec)
	if err != nil {
		return nil, errdefs.ToGRPC(err)
	}

	c.execs[r.ExecID] = execs

	s.send(&eventstypes.TaskExecAdded{
		ContainerID: c.id,
		ExecID:      r.ExecID,
	})

	return empty, nil
}

// ResizePty of a process
func (s *service) ResizePty(ctx context.Context, r *taskAPI.ResizePtyRequest) (_ *ptypes.Empty, err error) {
	logrus.WithFields(logrus.Fields{"func": "ResizePty", "ID": r.ID}).Info("Service Call")
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
	logrus.WithFields(logrus.Fields{"func": "State", "ID": r.ID}).Info("Service Call")
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

	// remove getConatiner?
	// c, err := s.getContainer(r.ID)

	logrus.WithField("ExecID", r.ExecID).Info("State ExecID")

	// if err != nil {
	// 	logrus.WithError(err).Info("first return !!!")
	// 	return nil, err
	// }

	c, err := s.getContainer(r.ID)
	// logrus.WithField("Status", r.ExecID).Info("State ExecID")
	if err != nil {
		// return nil, err
		return &taskAPI.StateResponse{
			ID:     r.ID,
			Pid:    s.hpid,
			Status: task.StatusCreated,
		}, nil
	}

	if r.ExecID == "" {
		logrus.WithError(err).Info("second return !!!")
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
			ExitedAt:   c.exitTime,
		}, nil
	}

	//deal with exec case
	execs, err := c.getExec(r.ExecID)
	if err != nil {
		return nil, err
	}

	logrus.Info("third return !!!")

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
		ExitedAt:   execs.exitTime,
	}, nil
}

// Pause the container
func (s *service) Pause(ctx context.Context, r *taskAPI.PauseRequest) (_ *ptypes.Empty, err error) {
	logrus.WithFields(logrus.Fields{"func": "Pause", "ID": r.ID}).Info("Service Call")
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

	c.status = task.StatusPausing

	err = s.sandbox.PauseContainer(spanCtx, r.ID)
	if err == nil {
		c.status = task.StatusPaused
		s.send(&eventstypes.TaskPaused{
			ContainerID: c.id,
		})
		return empty, nil
	}

	if status, err := s.getContainerStatus(c.id); err != nil {
		c.status = task.StatusUnknown
	} else {
		c.status = status
	}

	return empty, err
}

// Resume the container
func (s *service) Resume(ctx context.Context, r *taskAPI.ResumeRequest) (_ *ptypes.Empty, err error) {
	logrus.WithFields(logrus.Fields{"func": "Resume", "ID": r.ID}).Info("Service Call")
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
		c.status = task.StatusRunning
		s.send(&eventstypes.TaskResumed{
			ContainerID: c.id,
		})
		return empty, nil
	}

	if status, err := s.getContainerStatus(c.id); err != nil {
		c.status = task.StatusUnknown
	} else {
		c.status = status
	}

	return empty, err
}

// Kill a process with the provided signal
func (s *service) Kill(ctx context.Context, r *taskAPI.KillRequest) (_ *ptypes.Empty, err error) {
	logrus.WithFields(logrus.Fields{"func": "Kill", "ID": r.ID}).Info("Service Call")
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

	// trying to call cleanup from kill ourselves and stop function early

	s.sendL(&eventstypes.TaskExit{
		ContainerID: c.id,
		ID:          c.id,
		Pid:         0,
		ExitStatus:  0,
		ExitedAt:    time.Now(),
	})

	_, err = s.Cleanup(ctx)
	logrus.WithField("Exit code", err).Info("exiting with")

	c.status = task.StatusStopped
	return empty, nil

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
	if (signum == syscall.SIGKILL || signum == syscall.SIGTERM) && processStatus == task.StatusStopped {
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
	logrus.WithFields(logrus.Fields{"func": "Pids", "ID": r.ID}).Info("Service Call")
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
func (s *service) CloseIO(ctx context.Context, r *taskAPI.CloseIORequest) (_ *ptypes.Empty, err error) {
	logrus.WithFields(logrus.Fields{"func": "CloseIO", "ID": r.ID}).Info("Service Call")
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
func (s *service) Checkpoint(ctx context.Context, r *taskAPI.CheckpointTaskRequest) (_ *ptypes.Empty, err error) {
	logrus.WithFields(logrus.Fields{"func": "Checkpoint", "ID": r.ID}).Info("Service Call")
	shimLog.WithField("container", r.ID).Debug("Checkpoint() start")
	defer shimLog.WithField("container", r.ID).Debug("Checkpoint() end")
	span, _ := katatrace.Trace(s.rootCtx, shimLog, "Checkpoint", shimTracingTags)
	defer span.End()

	start := time.Now()
	defer func() {
		err = toGRPC(err)
		rpcDurationsHistogram.WithLabelValues("checkpoint").Observe(float64(time.Since(start).Nanoseconds() / int64(time.Millisecond)))
	}()

	return nil, errdefs.ToGRPCf(errdefs.ErrNotImplemented, "service Checkpoint")
}

// Connect returns shim information such as the shim's pid
func (s *service) Connect(ctx context.Context, r *taskAPI.ConnectRequest) (_ *taskAPI.ConnectResponse, err error) {
	logrus.WithFields(logrus.Fields{"func": "Connect", "ID": r.ID}).Info("Service Call")
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
		//Since kata cannot get the container's pid in VM, thus only return the hypervisor's pid.
		TaskPid: s.hpid,
	}, nil
}

func (s *service) Shutdown(ctx context.Context, r *taskAPI.ShutdownRequest) (_ *ptypes.Empty, err error) {
	logrus.WithFields(logrus.Fields{"func": "Shutdown", "ID": r.ID}).Info("Service Call")
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
	logrus.WithFields(logrus.Fields{"func": "Stats", "ID": r.ID}).Info("Service Call")
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
func (s *service) Update(ctx context.Context, r *taskAPI.UpdateTaskRequest) (_ *ptypes.Empty, err error) {
	logrus.WithFields(logrus.Fields{"func": "Update", "ID": r.ID}).Info("Service Call")
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

	// container not actually running, commented out to avoid errors

	_ = spanCtx

	// var resources *specs.LinuxResources
	// v, err := typeurl.UnmarshalAny(r.Resources)
	// if err != nil {
	// 	return nil, err
	// }
	// resources, ok := v.(*specs.LinuxResources)
	// if !ok {
	// 	return nil, errdefs.ToGRPCf(errdefs.ErrInvalidArgument, "Invalid resources type for %s", s.id)
	// }

	// err = s.sandbox.UpdateContainer(spanCtx, r.ID, *resources)
	// if err != nil {
	// 	return nil, errdefs.ToGRPC(err)
	// }

	return empty, nil
}

// Wait for a process to exit
func (s *service) Wait(ctx context.Context, r *taskAPI.WaitRequest) (_ *taskAPI.WaitResponse, err error) {
	logrus.WithFields(logrus.Fields{"func": "Wait", "ID": r.ID}).Info("Service Call")
	defer logrus.WithField("func", "Wait").Info("Service Call Exit")
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
		logrus.WithFields(logrus.Fields{"func": "Wait", "return": 1}).WithError(err).Info("Service Call Exit")
		return nil, err
	}

	//wait for container
	if r.ExecID == "" {
		ret = <-c.exitCh

		// refill the exitCh with the container process's exit code in case
		// there were other waits on this process.
		c.exitCh <- ret
	} else { //wait for exec
		execs, err := c.getExec(r.ExecID)
		if err != nil {
			logrus.WithFields(logrus.Fields{"func": "Wait", "return": 2}).WithError(err).Info("Service Call Exit")
			return nil, err
		}
		ret = <-execs.exitCh

		// refill the exitCh with the exec process's exit code in case
		// there were other waits on this process.
		execs.exitCh <- ret
	}

	logrus.WithFields(logrus.Fields{"func": "Wait", "return": 3}).Info("Service Call Exit")

	return &taskAPI.WaitResponse{
		ExitStatus: ret,
		ExitedAt:   c.exitTime,
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
		ExitedAt:    e.timestamp,
	})
}

func (s *service) getContainer(id string) (*container, error) {

	data, err := json.Marshal(s.containers)
	logrus.WithFields(logrus.Fields{
		"container":  id,
		"containers": string(data),
	}).WithError(err).Info("getContainer Debug")

	c := s.containers[id]

	// TODO trace callers to figure out where the failure is coming from

	// os.WriteFile("/tmp/callstack.txt", debug.Stack(), 0644)
	// logrus.Infof(string(debug.Stack()))

	if c == nil {
		return nil, errdefs.ToGRPCf(errdefs.ErrNotFound, "container does not exist %s", id)
	}

	return c, nil
}

func (s *service) getContainerStatus(containerID string) (task.Status, error) {
	cStatus, err := s.sandbox.StatusContainer(containerID)
	if err != nil {
		return task.StatusUnknown, err
	}

	var status task.Status
	switch cStatus.State.State {
	case types.StateReady:
		status = task.StatusCreated
	case types.StateRunning:
		status = task.StatusRunning
	case types.StatePaused:
		status = task.StatusPaused
	case types.StateStopped:
		status = task.StatusStopped
	}

	return status, nil
}
