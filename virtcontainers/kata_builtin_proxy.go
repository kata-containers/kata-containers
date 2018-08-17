// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"bufio"
	"fmt"
	"io"
	"net"

	"github.com/sirupsen/logrus"
)

var buildinProxyConsoleProto = consoleProtoUnix

// This is a kata builtin proxy implementation of the proxy interface. Kata proxy
// functionality is implemented inside the virtcontainers library.
type kataBuiltInProxy struct {
	sandboxID string
	conn      net.Conn
}

// check if the proxy has watched the vm console.
func (p *kataBuiltInProxy) consoleWatched() bool {
	return p.conn != nil
}

func (p *kataBuiltInProxy) validateParams(params proxyParams) error {
	if len(params.id) == 0 || len(params.agentURL) == 0 || len(params.consoleURL) == 0 {
		return fmt.Errorf("Invalid proxy parameters %+v", params)
	}

	if params.logger == nil {
		return fmt.Errorf("Invalid proxy parameter: proxy logger is not set")
	}

	return nil
}

// start is the proxy start implementation for kata builtin proxy.
// It starts the console watcher for the guest.
// It returns agentURL to let agent connect directly.
func (p *kataBuiltInProxy) start(params proxyParams) (int, string, error) {
	if err := p.validateParams(params); err != nil {
		return -1, "", err
	}

	if p.consoleWatched() {
		return -1, "", fmt.Errorf("kata builtin proxy running for sandbox %s", params.id)
	}

	params.logger.Info("Starting builtin kata proxy")

	p.sandboxID = params.id
	err := p.watchConsole(buildinProxyConsoleProto, params.consoleURL, params.logger)
	if err != nil {
		p.sandboxID = ""
		return -1, "", err
	}

	return -1, params.agentURL, nil
}

// stop is the proxy stop implementation for kata builtin proxy.
func (p *kataBuiltInProxy) stop(pid int) error {
	if p.conn != nil {
		p.conn.Close()
		p.conn = nil
		p.sandboxID = ""
	}
	return nil
}

func (p *kataBuiltInProxy) watchConsole(proto, console string, logger *logrus.Entry) (err error) {
	var (
		scanner *bufio.Scanner
		conn    net.Conn
	)

	switch proto {
	case consoleProtoUnix:
		conn, err = net.Dial("unix", console)
		if err != nil {
			return err
		}
	// TODO: add pty console support for kvmtools
	case consoleProtoPty:
		fallthrough
	default:
		return fmt.Errorf("unknown console proto %s", proto)
	}

	p.conn = conn

	go func() {
		scanner = bufio.NewScanner(conn)
		for scanner.Scan() {
			logger.WithFields(logrus.Fields{
				"sandbox":   p.sandboxID,
				"vmconsole": scanner.Text(),
			}).Debug("reading guest console")
		}

		if err := scanner.Err(); err != nil {
			if err == io.EOF {
				logger.Info("console watcher quits")
			} else {
				logger.WithError(err).WithFields(logrus.Fields{
					"console-protocol": proto,
					"console-socket":   console,
				}).Error("Failed to read agent logs")
			}
		}
	}()

	return nil
}
