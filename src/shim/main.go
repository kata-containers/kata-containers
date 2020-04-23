// Copyright 2017 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"bufio"
	"errors"
	"flag"
	"fmt"
	"io"
	"io/ioutil"
	"log/syslog"
	"net"
	"net/url"
	"os"
	"os/signal"
	"runtime"
	"strconv"
	"strings"
	"sync"
	"time"

	"github.com/kata-containers/agent/protocols/client"
	opentracing "github.com/opentracing/opentracing-go"
	"github.com/sirupsen/logrus"
	lSyslog "github.com/sirupsen/logrus/hooks/syslog"
	context "golang.org/x/net/context"
)

type socketAddress struct {
	scheme string
	path   string
	port   uint32
}

func (s *socketAddress) string() string {
	return fmt.Sprintf("%s:%s:%d", s.scheme, s.path, s.port)
}

const (
	shimName    = "kata-shim"
	exitFailure = 1
	exitSuccess = 0
	// Max number of threads the shim should consume.
	// We choose 6 as we want a couple of threads for the runtime (gc etc.)
	// and couple of threads for our parallel user code, such as the copy
	// code in shim.go
	maxThreads = 6

	// timeout for dialing to a hybrid vsock
	hybridVSockDialTimeout = time.Second

	// maximum number of tries to connect hybrid vsock
	hybridVSockMaxConnectTries = 20

	// delay before trying to connect again
	hybridVSockConnectDelay = time.Millisecond * 300
)

// version is the shim version. This variable is populated at build time.
var version = "unknown"

var debug bool

// if true, coredump when an internal error occurs or a fatal signal is received
var crashOnError = false

// if true, enable opentracing support.
var tracing = false

var shimLog *logrus.Entry

func logger() *logrus.Entry {
	if shimLog != nil {
		return shimLog
	}
	return logrus.NewEntry(logrus.StandardLogger())
}

func initLogger(logLevel, container, execID string, announceFields logrus.Fields, loggerOutput io.Writer) error {
	shimLog = logrus.WithFields(logrus.Fields{
		"name":      shimName,
		"pid":       os.Getpid(),
		"source":    "shim",
		"container": container,
		"exec-id":   execID,
	})

	shimLog.Logger.Formatter = &logrus.TextFormatter{TimestampFormat: time.RFC3339Nano}

	level, err := logrus.ParseLevel(logLevel)
	if err != nil {
		return err
	}

	shimLog.Logger.SetLevel(level)

	shimLog.Logger.Out = loggerOutput

	hook, err := lSyslog.NewSyslogHook("", "", syslog.LOG_INFO|syslog.LOG_USER, shimName)
	if err == nil {
		shimLog.Logger.AddHook(hook)
	}

	logger().WithFields(announceFields).Info("announce")

	return nil
}

func setThreads() {
	// If GOMAXPROCS has not been set, restrict our thread usage
	// so we don't grow many idle threads on large core count systems,
	// which un-necessarily consume host PID space (and thus set an
	// artificial max limit on the number of concurrent containers we can
	// run)
	if os.Getenv("GOMAXPROCS") == "" {
		if runtime.NumCPU() > maxThreads {
			runtime.GOMAXPROCS(maxThreads)
		}
	}
}

func socketAddr(uri string) (socketAddress, error) {
	if uri == "" {
		return socketAddress{}, errors.New("empty uri")

	}
	addr, err := url.Parse(uri)
	if err != nil {
		return socketAddress{}, err
	}

	switch addr.Scheme {
	case "", client.UnixSocketScheme:
		return socketAddress{
			scheme: client.UnixSocketScheme,
			path:   addr.Host + addr.Path,
			port:   0,
		}, nil
	case client.HybridVSockScheme:
		hvsocket := strings.Split(addr.Path, ":")
		// expected path:port
		if len(hvsocket) != 2 {
			return socketAddress{}, fmt.Errorf("Invalid hybrid vsock scheme: %s", uri)
		}

		var port uint64
		if port, err = strconv.ParseUint(hvsocket[1], 10, 32); err != nil {
			return socketAddress{}, fmt.Errorf("Invalid hybrid vsock port %s: %v", uri, err)
		}

		return socketAddress{
			scheme: client.HybridVSockScheme,
			path:   hvsocket[0],
			port:   uint32(port),
		}, nil
	default:
		return socketAddress{}, errors.New("invalid address scheme")
	}
}

func socketDial(addr socketAddress) (net.Conn, error) {
	switch addr.scheme {
	case client.UnixSocketScheme:
		return net.Dial("unix", addr.path)
	case client.HybridVSockScheme:
		var err error
		var conn net.Conn
		for i := 0; i < hybridVSockMaxConnectTries; i++ {
			conn, err = client.HybridVSockDialer(addr.string(), hybridVSockDialTimeout)
			if err == nil {
				c := conn.(*net.UnixConn)
				if f, e := c.File(); e == nil && f != nil {
					return conn, nil
				}
			}
			time.Sleep(hybridVSockConnectDelay)
		}
		return nil, fmt.Errorf("unable to connect hybrid vsock: %v", err)
	default:
		return nil, errors.New("invalid socket address")
	}
}

func printAgentLogs(sock string) error {
	// Don't return an error if nothing has been provided. This flag is optional.
	if sock == "" {
		return nil
	}

	agentLogsAddr, err := socketAddr(sock)
	if err != nil {
		logger().WithField("socket-address", sock).WithError(err).Fatal("invalid agent logs socket address")
		return err
	}

	// Check permissions socket for "other" is 0.
	// For security reasons, the socket shouldn't be accessible
	// for the "other" group.
	fileInfo, err := os.Stat(agentLogsAddr.path)
	if err != nil {
		return err
	}

	otherMask := 0007
	other := int(fileInfo.Mode().Perm()) & otherMask
	if other != 0 {
		return fmt.Errorf("All socket permissions for 'other' should be disabled, got %3.3o", other)
	}

	// Allow log messages coming from the agent to be distinguished from
	// messages originating from the shim itself.
	agentLogger := logger().WithFields(logrus.Fields{
		"source": "agent",
	})

	go func() {
		conn, err := socketDial(agentLogsAddr)
		if err != nil {
			agentLogger.WithError(err).Error("Could not connect logs socket")
			return
		}
		scanner := bufio.NewScanner(conn)
		for scanner.Scan() {
			agentLogger.Infof("%s\n", scanner.Text())
		}

		if err := scanner.Err(); err != nil {
			logger().WithError(err).Error("Failed reading agent logs from socket")
		}
	}()

	return nil
}

func realMain(ctx context.Context) (exitCode int) {
	var (
		logLevel        string
		agentAddr       string
		container       string
		execID          string
		agentLogsSocket string
		terminal        bool
		proxyExitCode   bool
		showVersion     bool
	)

	setThreads()

	flag.BoolVar(&debug, "debug", false, "enable debug mode")
	flag.BoolVar(&tracing, "trace", false, "enable opentracing support")
	flag.BoolVar(&showVersion, "version", false, "display program version and exit")
	flag.StringVar(&logLevel, "log", "warn", "set shim log level: debug, info, warn, error, fatal or panic")
	flag.StringVar(&agentAddr, "agent", "", "agent gRPC socket endpoint")

	flag.StringVar(&container, "container", "", "container id for the shim")
	flag.StringVar(&execID, "exec-id", "", "process id for the shim")
	flag.BoolVar(&terminal, "terminal", false, "specify if a terminal is setup")
	flag.BoolVar(&proxyExitCode, "proxy-exit-code", true, "proxy exit code of the process")
	flag.StringVar(&agentLogsSocket, "agent-logs-socket", "", "socket to listen on to retrieve agent logs")

	flag.Parse()

	if showVersion {
		fmt.Printf("%v version %v\n", shimName, version)
		return exitSuccess
	}

	if logLevel == "debug" {
		debug = true
	}

	if debug {
		crashOnError = true
	}

	if agentAddr == "" || container == "" || execID == "" {
		logger().WithField("agentAddr", agentAddr).WithField("container", container).WithField("exec-id", execID).Error("container ID, exec ID and agent socket endpoint must be set")
		return exitFailure
	}

	announceFields := logrus.Fields{
		"version":         version,
		"debug":           debug,
		"log-level":       logLevel,
		"agent-socket":    agentAddr,
		"terminal":        terminal,
		"proxy-exit-code": proxyExitCode,
		"tracing":         tracing,
	}

	// The final parameter makes sure all output going to stdout/stderr is discarded.
	err := initLogger(logLevel, container, execID, announceFields, ioutil.Discard)
	if err != nil {
		logger().WithError(err).WithField("loglevel", logLevel).Error("invalid log level")
		return exitFailure
	}

	// Initialise tracing now the logger is ready
	tracer, err := createTracer(shimName)
	if err != nil {
		logger().WithError(err).Fatal("failed to setup tracing")
		return exitFailure
	}

	// create root span
	span := tracer.StartSpan("realMain")
	ctx = opentracing.ContextWithSpan(ctx, span)
	defer span.Finish()

	if err := printAgentLogs(agentLogsSocket); err != nil {
		logger().WithError(err).Fatal("failed to print agent logs")
		return exitFailure
	}

	shim, err := newShim(ctx, agentAddr, container, execID)
	if err != nil {
		logger().WithError(err).Error("failed to create new shim")
		return exitFailure
	}

	// winsize
	if terminal {
		termios, err := setupTerminal(int(os.Stdin.Fd()))
		if err != nil {
			logger().WithError(err).Error("failed to set raw terminal")
			return exitFailure
		}
		defer restoreTerminal(int(os.Stdin.Fd()), termios)
	}

	// signals
	sigc := shim.handleSignals(ctx, os.Stdin)
	defer signal.Stop(sigc)

	// This wait call cannot be deferred and has to wait for every
	// input/output to return before the code tries to go further
	// and wait for the process. Indeed, after the process has been
	// waited for, we cannot expect to do any more calls related to
	// this process since it is going to be removed from the agent.
	wg := &sync.WaitGroup{}

	// Encapsulate the call the I/O handling function in a span here since
	// that function returns quickly and we want to know how long I/O
	// took.
	stdioSpan, _ := trace(ctx, "proxyStdio")

	// Add a tag to allow the I/O to be filtered out.
	stdioSpan.SetTag("category", "interactive")

	shim.proxyStdio(wg, terminal)

	wg.Wait()

	stdioSpan.Finish()

	// wait until exit
	exitcode, err := shim.wait()
	if err != nil {
		logger().WithError(err).WithField("exec-id", execID).Error("failed waiting for process")
		return exitFailure
	} else if proxyExitCode {
		logger().WithField("exitcode", exitcode).Info("using shim to proxy exit code")
		if exitcode != 0 {
			return int(exitcode)
		}
	}

	return exitSuccess
}

func main() {
	// create a new empty context
	ctx := context.Background()

	defer handlePanic(ctx)

	exitCode := realMain(ctx)

	stopTracing(ctx)

	os.Exit(exitCode)
}
