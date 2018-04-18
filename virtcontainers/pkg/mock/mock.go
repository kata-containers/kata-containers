// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package mock

import (
	"flag"
	"fmt"
	"io/ioutil"
	"net"
	"net/url"
	"os"
	"path/filepath"

	"google.golang.org/grpc"
)

// DefaultMockCCShimBinPath is populated at link time.
var DefaultMockCCShimBinPath string

// DefaultMockKataShimBinPath is populated at link time.
var DefaultMockKataShimBinPath string

// DefaultMockHookBinPath is populated at link time.
var DefaultMockHookBinPath string

// ShimStdoutOutput is the expected output sent by the mock shim on stdout.
const ShimStdoutOutput = "Some output on stdout"

// ShimStderrOutput is the expected output sent by the mock shim on stderr.
const ShimStderrOutput = "Some output on stderr"

// ShimMockConfig is the configuration structure for all virtcontainers shim mock implementations.
type ShimMockConfig struct {
	Name               string
	URLParamName       string
	ContainerParamName string
	TokenParamName     string
}

// StartShim is a common routine for starting a shim mock.
func StartShim(config ShimMockConfig) error {
	logDirPath, err := ioutil.TempDir("", config.Name+"-")
	if err != nil {
		return err
	}

	logFilePath := filepath.Join(logDirPath, "mock_"+config.Name+".log")

	f, err := os.Create(logFilePath)
	if err != nil {
		return err
	}
	defer f.Close()

	tokenFlag := flag.String(config.TokenParamName, "", "Container token")
	urlFlag := flag.String(config.URLParamName, "", "Agent URL")
	containerFlag := flag.String(config.ContainerParamName, "", "Container ID")

	flag.Parse()

	fmt.Fprintf(f, "INFO: Token = %s\n", *tokenFlag)
	fmt.Fprintf(f, "INFO: URL = %s\n", *urlFlag)
	fmt.Fprintf(f, "INFO: Container = %s\n", *containerFlag)

	if *tokenFlag == "" {
		err := fmt.Errorf("token should not be empty")
		fmt.Fprintf(f, "%s\n", err)
		return err
	}

	if *urlFlag == "" {
		err := fmt.Errorf("url should not be empty")
		fmt.Fprintf(f, "%s\n", err)
		return err
	}

	if _, err := url.Parse(*urlFlag); err != nil {
		err2 := fmt.Errorf("could not parse the URL %q: %s", *urlFlag, err)
		fmt.Fprintf(f, "%s\n", err2)
		return err2
	}

	if *containerFlag == "" {
		err := fmt.Errorf("container should not be empty")
		fmt.Fprintf(f, "%s\n", err)
		return err
	}

	// Print some traces to stdout
	fmt.Fprintf(os.Stdout, ShimStdoutOutput)
	os.Stdout.Close()

	// Print some traces to stderr
	fmt.Fprintf(os.Stderr, ShimStderrOutput)
	os.Stderr.Close()

	fmt.Fprintf(f, "INFO: Shim exited properly\n")

	return nil
}

// ProxyMock is the proxy mock interface.
// It allows for implementing different kind
// of containers proxies front end.
type ProxyMock interface {
	Start(URL string) error
	Stop() error
}

// ProxyUnixMock is the UNIX proxy mock
type ProxyUnixMock struct {
	ClientHandler func(c net.Conn)

	listener net.Listener
}

// ProxyGRPCMock is the gRPC proxy mock
type ProxyGRPCMock struct {
	// GRPCImplementer is the structure implementing
	// the GRPC interface we want the proxy to serve.
	GRPCImplementer interface{}

	// GRPCRegister is the registration routine for
	// the GRPC service.
	GRPCRegister func(s *grpc.Server, srv interface{})

	listener net.Listener
}

// Start starts the UNIX proxy mock
func (p *ProxyUnixMock) Start(URL string) error {
	if p.ClientHandler == nil {
		return fmt.Errorf("Missing client handler")
	}

	url, err := url.Parse(URL)
	if err != nil {
		return err
	}

	l, err := net.Listen(url.Scheme, url.Path)
	if err != nil {
		return err
	}

	p.listener = l

	go func() {
		defer func() {
			l.Close()
		}()

		for {
			conn, err := l.Accept()
			if err != nil {
				return
			}

			go p.ClientHandler(conn)
		}
	}()

	return nil
}

// Stop stops the UNIX proxy mock
func (p *ProxyUnixMock) Stop() error {
	if p.listener == nil {
		return fmt.Errorf("Missing proxy listener")
	}

	return p.listener.Close()
}

// Start starts the gRPC proxy mock
func (p *ProxyGRPCMock) Start(URL string) error {
	if p.GRPCImplementer == nil {
		return fmt.Errorf("Missing gRPC handler")
	}

	if p.GRPCRegister == nil {
		return fmt.Errorf("Missing gRPC registration routine")
	}

	url, err := url.Parse(URL)
	if err != nil {
		return err
	}

	l, err := net.Listen(url.Scheme, url.Path)
	if err != nil {
		return err
	}

	p.listener = l

	grpcServer := grpc.NewServer()
	p.GRPCRegister(grpcServer, p.GRPCImplementer)

	go func() {
		grpcServer.Serve(l)
	}()

	return nil
}

// Stop stops the gRPC proxy mock
func (p *ProxyGRPCMock) Stop() error {
	if p.listener == nil {
		return fmt.Errorf("Missing proxy listener")
	}

	return p.listener.Close()
}
