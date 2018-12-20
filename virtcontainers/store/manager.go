// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package store

import (
	"context"
	"net/url"
	"sync"

	opentracing "github.com/opentracing/opentracing-go"
	"github.com/sirupsen/logrus"
)

// Item represents a virtcontainers items that will be managed through the store.
type Item uint8

const (
	// Configuration represents a configuration item to be stored
	Configuration Item = iota

	// State represents a state item to be stored.
	State

	// Network represents a networking item to be stored.
	Network

	// Hypervisor represents an hypervisor item to be stored.
	Hypervisor

	// Agent represents a agent item to be stored.
	Agent

	// Process represents a container process item to be stored.
	Process

	// Lock represents a lock item to be stored.
	Lock

	// Mounts represents a set of mounts related item to be stored.
	Mounts

	// Devices represents a set of devices related item to be stored.
	Devices

	// DeviceIDs represents a set of reference IDs item to be stored.
	DeviceIDs
)

func (i Item) String() string {
	switch i {
	case Configuration:
		return "Configuration"
	case State:
		return "State"
	case Network:
		return "Network"
	case Hypervisor:
		return "Hypervisor"
	case Agent:
		return "Agent"
	case Process:
		return "Process"
	case Lock:
		return "Lock"
	case Mounts:
		return "Mounts"
	case Devices:
		return "Devices"
	case DeviceIDs:
		return "Device IDs"
	}

	return ""
}

// Store is an opaque structure representing a virtcontainers Store.
type Store struct {
	sync.RWMutex
	ctx context.Context

	url    string
	scheme string
	path   string
	host   string
}

// New will return a new virtcontainers Store.
// If there is already a Store for the URL, we will re-use it.
// Otherwise a new Store is created.
func New(ctx context.Context, storeURL string) (*Store, error) {
	u, err := url.Parse(storeURL)
	if err != nil {
		return nil, err
	}

	return &Store{
		ctx:    ctx,
		url:    storeURL,
		scheme: u.Scheme,
		path:   u.Path,
		host:   u.Host,
	}, nil
}

var storeLog = logrus.WithField("source", "virtcontainers/store")

// Logger returns a logrus logger appropriate for logging Store messages
func (s *Store) Logger() *logrus.Entry {
	return storeLog.WithFields(logrus.Fields{
		"subsystem": "store",
		"path":      s.path,
	})
}

func (s *Store) trace(name string) (opentracing.Span, context.Context) {
	if s.ctx == nil {
		s.Logger().WithField("type", "bug").Error("trace called before context set")
		s.ctx = context.Background()
	}

	span, ctx := opentracing.StartSpanFromContext(s.ctx, name)

	span.SetTag("subsystem", "store")
	span.SetTag("path", s.path)

	return span, ctx
}

// Load loads a virtcontainers item from a Store.
func (s *Store) Load(item Item, data interface{}) error {
	span, _ := s.trace("Load")
	defer span.Finish()

	span.SetTag("item", item)

	s.RLock()
	defer s.RUnlock()

	return nil
}

// Store stores a virtcontainers item into a Store.
func (s *Store) Store(item Item, data interface{}) error {
	span, _ := s.trace("Store")
	defer span.Finish()

	span.SetTag("item", item)

	s.Lock()
	defer s.Unlock()

	return nil
}
