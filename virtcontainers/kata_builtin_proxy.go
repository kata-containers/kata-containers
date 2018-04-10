//
// Copyright (c) 2018 HyperHQ Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
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
	podID string
	conn  net.Conn
}

// start is the proxy start implementation for kata builtin proxy.
// It starts the console watcher for the guest.
// It returns agentURL to let agent connect directly.
func (p *kataBuiltInProxy) start(pod Pod, params proxyParams) (int, string, error) {
	if p.conn != nil {
		return -1, "", fmt.Errorf("kata builtin proxy running for pod %s", p.podID)
	}

	p.podID = pod.id
	console := pod.hypervisor.getPodConsole(pod.id)
	err := p.watchConsole(consoleProtoUnix, console, params.logger)
	if err != nil {
		return -1, "", err
	}

	return -1, params.agentURL, nil
}

// stop is the proxy stop implementation for kata builtin proxy.
func (p *kataBuiltInProxy) stop(pod Pod, pid int) error {
	if p.conn != nil {
		p.conn.Close()
		p.conn = nil
		p.podID = ""
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
			fmt.Printf("[POD-%s] vmconsole: %s\n", p.podID, scanner.Text())
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
