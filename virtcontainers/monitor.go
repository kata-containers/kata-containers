// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"sync"
	"time"
)

const defaultCheckInterval = 10 * time.Second

type monitor struct {
	sync.Mutex

	sandbox       *Sandbox
	checkInterval time.Duration
	watchers      []chan error
	running       bool
	stopCh        chan bool
	wg            sync.WaitGroup
}

func newMonitor(s *Sandbox) *monitor {
	return &monitor{
		sandbox:       s,
		checkInterval: defaultCheckInterval,
		stopCh:        make(chan bool, 1),
	}
}

func (m *monitor) newWatcher() (chan error, error) {
	m.Lock()
	defer m.Unlock()

	watcher := make(chan error, 1)
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
					m.watchAgent()
				}
			}
		}()
	}

	return watcher, nil
}

func (m *monitor) notify(err error) {
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
		c <- err
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

	defer func() {
		m.stopCh <- true
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

func (m *monitor) watchAgent() {
	err := m.sandbox.agent.check()
	if err != nil {
		m.notify(err)
	}
}
