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
	defaultCheckInterval = 1 * time.Second
	watcherChannelSize   = 128
)

type monitor struct {
	sync.Mutex

	sandbox       *Sandbox
	checkInterval time.Duration
	watchers      []chan error
	wg            sync.WaitGroup
	running       bool
	stopCh        chan bool
}

func newMonitor(s *Sandbox) *monitor {
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
			virtLog.Warnf("watcher closed channel: %v", x)
		}
	}()

	for _, c := range m.watchers {
		// throw away message can not write to channel
		// make it not stuck, the first error is useful.
		select {
		case c <- err:

		default:
			virtLog.WithField("channel-size", watcherChannelSize).Warnf("watcher channel is full, throw notify message")
		}
	}
}

func (m *monitor) stop() {
	// wait outside of monitor lock for the watcher channel to exit.
	defer m.wg.Wait()

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
			virtLog.Warnf("watcher closed channel: %v", x)
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
	if err := m.sandbox.hypervisor.check(); err != nil {
		m.notify(ctx, errors.Wrapf(err, "failed to ping hypervisor process"))
		return err
	}
	return nil
}
