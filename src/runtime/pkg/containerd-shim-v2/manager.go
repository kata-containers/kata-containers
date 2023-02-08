package containerdshim

import (
	"context"
	"os"
	sysexec "os/exec"
	goruntime "runtime"
	"syscall"
	"time"

	"github.com/containerd/containerd/errdefs"
	"github.com/containerd/containerd/namespaces"
	"github.com/containerd/containerd/runtime/v2/shim"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils/katatrace"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/oci"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/utils"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/compatoci"
	"github.com/pkg/errors"
	"github.com/sirupsen/logrus"
	"golang.org/x/sys/unix"
)

// NewShimManager returns an implementation of the shim manager
// using kata-containers
func NewShimManager(name string) shim.Manager {
	return &manager{
		name: name,
	}
}

type manager struct {
	name string
	id   string
}

func (m *manager) Name() string {
	return m.name
}

func (m *manager) Start(ctx context.Context, id string, opts shim.StartOpts) (_ string, retErr error) {
	m.id = id

	bundlePath, err := os.Getwd()
	if err != nil {
		return "", err
	}

	address, err := getAddress(ctx, bundlePath, opts.Address, id)
	if err != nil {
		return "", err
	}
	if address != "" {
		if err := shim.WriteAddress("address", address); err != nil {
			return "", err
		}
		return address, nil
	}

	cmd, err := newCommand(ctx, id, opts.ContainerdBinary, opts.Address)
	if err != nil {
		return "", err
	}

	address, err = shim.SocketAddress(ctx, opts.Address, id)
	if err != nil {
		return "", err
	}

	socket, err := shim.NewSocket(address)

	if err != nil {
		if !shim.SocketEaddrinuse(err) {
			return "", err
		}
		if err := shim.RemoveSocket(address); err != nil {
			return "", errors.Wrap(err, "remove already used socket")
		}
		if socket, err = shim.NewSocket(address); err != nil {
			return "", err
		}
	}

	defer func() {
		if retErr != nil {
			socket.Close()
			_ = shim.RemoveSocket(address)
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

	if err = shim.WritePidFile("shim.pid", cmd.Process.Pid); err != nil {
		return "", err
	}
	if err = shim.WriteAddress("address", address); err != nil {
		return "", err
	}
	return address, nil
}

func (m *manager) Stop(ctx context.Context, id string) (_ shim.StopStatus, err error) {
	span, spanCtx := katatrace.Trace(ctx, shimLog, "Cleanup", shimTracingTags)
	defer span.End()

	//Since the binary cleanup will return the DeleteResponse from stdout to
	//containerd, thus we must make sure there is no any outputs in stdout except
	//the returned response, thus here redirect the log to stderr in case there's
	//any log output to stdout.
	logrus.SetOutput(os.Stderr)

	defer func() {
		err = toGRPC(err)
	}()

	if m.id == "" {
		return shim.StopStatus{},
			errdefs.ToGRPCf(errdefs.ErrInvalidArgument, "the container id is empty, please specify the container id")
	}

	path, err := os.Getwd()
	if err != nil {
		return shim.StopStatus{}, err
	}

	ociSpec, err := compatoci.ParseConfigJSON(path)
	if err != nil {
		return shim.StopStatus{}, err
	}

	containerType, err := oci.ContainerType(ociSpec)
	if err != nil {
		return shim.StopStatus{}, err
	}

	switch containerType {
	case vc.PodSandbox, vc.SingleContainer:
		err = cleanupContainer(spanCtx, m.id, m.id, path)
		if err != nil {
			return shim.StopStatus{}, err
		}
	case vc.PodContainer:
		sandboxID, err := oci.SandboxID(ociSpec)
		if err != nil {
			return shim.StopStatus{}, err
		}

		err = cleanupContainer(spanCtx, sandboxID, m.id, path)
		if err != nil {
			return shim.StopStatus{}, err
		}
	}

	if err != nil {
		return shim.StopStatus{}, err
	}
	return shim.StopStatus{
		ExitStatus: int(128 + uint32(unix.SIGKILL)),
		ExitedAt:   time.Now(),
	}, nil
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
	opts := ctx.Value(shim.OptsKey{}).(shim.Opts)
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
