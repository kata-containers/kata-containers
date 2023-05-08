// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"sync"
	"time"

	"github.com/pkg/errors"
)

const (
	defaultCheckInterval = 5 * time.Second
	watcherChannelSize   = 128
)

var monitorLog = virtLog.WithField("subsystem", "virtcontainers/monitor")

// nolint: govet
type monitor struct {
	watchers []chan error
	sandbox  *Sandbox

	wg sync.WaitGroup
	sync.Mutex

	stopCh        chan bool
	checkInterval time.Duration

	running bool
}

func newMonitor(s *Sandbox) *monitor {
	// there should only be one monitor for one sandbox,
	// so it's safe to let monitorLog as a global variable.
	monitorLog = monitorLog.WithField("sandbox", s.ID())
	return &monitor{
		sandbox:       s,
		checkInterval: defaultCheckInterval,
		stopCh:        make(chan bool, 1),
	}
}

func (m *monitor) newWatcher(ctx context.Context) (chan error, error) {
	m.Lock()
	defer m.Unlock()

	watcher := make(chan error, watcherChannelSize)
	m.watchers = append(m.watchers, watcher)

	if !m.running {
		m.running = true
		m.wg.Add(1)

		// create and start agent watcher
		go func() {
			tick := time.NewTicker(m.checkInterval)
			for {
				select {
				case <-m.stopCh:
					tick.Stop()
					m.wg.Done()
					return
				case <-tick.C:
					m.watchHypervisor(ctx)
					m.watchAgent(ctx)
				}
			}
		}()
	}

	return watcher, nil
}

func (m *monitor) notify(ctx context.Context, err error) {
	monitorLog.WithError(err).Warn("notify on errors")
	m.sandbox.agent.markDead(ctx)

	m.Lock()
	defer m.Unlock()

	if !m.running {
		return
	}

	// a watcher is not supposed to close the channel
	// but just in case...
	defer func() {
		if x := recover(); x != nil {
			monitorLog.Warnf("watcher closed channel: %v", x)
		}
	}()

	for _, c := range m.watchers {
		monitorLog.WithError(err).Warn("write error to watcher")
		// throw away message can not write to channel
		// make it not stuck, the first error is useful.
		select {
		case c <- err:

		default:
			monitorLog.WithField("channel-size", watcherChannelSize).Warnf("watcher channel is full, throw notify message")
		}
	}
}

func (m *monitor) stop() {
	// wait outside of monitor lock for the watcher channel to exit.
	defer m.wg.Wait()
	monitorLog.Info("stopping monitor")

	m.Lock()
	defer m.Unlock()

	if !m.running {
		return
	}

	m.stopCh <- true
	defer func() {
		m.watchers = nil
		m.running = false
	}()

	// a watcher is not supposed to close the channel
	// but just in case...
	defer func() {
		if x := recover(); x != nil {
			monitorLog.Warnf("watcher closed channel: %v", x)
		}
	}()

	for _, c := range m.watchers {
		close(c)
	}
}

func (m *monitor) watchAgent(ctx context.Context) {
	err := m.sandbox.agent.check(ctx)
	if err != nil {
		// TODO: define and export error types
		m.notify(ctx, errors.Wrapf(err, "failed to ping agent"))
	}
}

func (m *monitor) watchHypervisor(ctx context.Context) error {
	if err := m.sandbox.hypervisor.Check(); err != nil {
		m.notify(ctx, errors.Wrapf(err, "failed to ping hypervisor process"))
		return err
	}
	return nil
}
