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

// start is the proxy start implementation for kata builtin proxy.
// It starts the console watcher for the guest.
// It returns agentURL to let agent connect directly.
func (p *kataBuiltInProxy) start(sandbox *Sandbox, params proxyParams) (int, string, error) {
	if p.consoleWatched() {
		return -1, "", fmt.Errorf("kata builtin proxy running for sandbox %s", p.sandboxID)
	}

	p.sandboxID = sandbox.id
	console, err := sandbox.hypervisor.getSandboxConsole(sandbox.id)
	if err != nil {
		return -1, "", err
	}

	err = p.watchConsole(consoleProtoUnix, console, params.logger)
	if err != nil {
		return -1, "", err
	}

	return -1, params.agentURL, nil
}

// stop is the proxy stop implementation for kata builtin proxy.
func (p *kataBuiltInProxy) stop(sandbox *Sandbox, pid int) error {
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
			fmt.Printf("[SB-%s] vmconsole: %s\n", p.sandboxID, scanner.Text())
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
