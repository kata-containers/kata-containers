// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package store

import (
	"context"
	"fmt"
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

	// UUID represents a set of uuids item to be stored.
	UUID
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

	backend backend
}

type manager struct {
	sync.RWMutex
	stores map[string]*Store
}

var stores = &manager{stores: make(map[string]*Store)}

func (m *manager) addStore(s *Store) (rs *Store, err error) {
	if s == nil {
		return nil, fmt.Errorf("Store can not be nil")
	}

	if s.url == "" {
		return nil, fmt.Errorf("Store URL can not be nil")
	}

	m.Lock()
	defer m.Unlock()

	if m.stores[s.url] == nil {
		m.stores[s.url] = s
	}

	return m.stores[s.url], nil
}

func (m *manager) removeStore(url string) {
	m.Lock()
	defer m.Unlock()

	delete(m.stores, url)
}

func (m *manager) findStore(url string) *Store {
	m.RLock()
	defer m.RUnlock()

	return m.stores[url]
}

// New will return a new virtcontainers Store.
// If there is already a Store for the URL, we will re-use it.
// Otherwise a new Store is created.
func New(ctx context.Context, storeURL string) (*Store, error) {
	// Do we already have such store?
	if s := stores.findStore(storeURL); s != nil {
		return s, nil
	}

	u, err := url.Parse(storeURL)
	if err != nil {
		return nil, err
	}

	s := &Store{
		ctx:    ctx,
		url:    storeURL,
		scheme: u.Scheme,
		path:   u.Path,
		host:   u.Host,
	}

	backend, err := newBackend(s.scheme)
	if err != nil {
		return nil, err
	}

	s.backend = backend

	// Create new backend
	if err = s.backend.new(ctx, s.path, s.host); err != nil {
		return nil, err
	}

	if s, err = stores.addStore(s); err != nil {
		return nil, err
	}

	return s, nil
}

// DeleteAll deletes all Stores from the manager.
func DeleteAll() {
	for _, s := range stores.stores {
		s.Delete()
	}
}

var storeLog = logrus.WithField("source", "virtcontainers/store")

// SetLogger sets the custom logger to be used by this package. If not called,
// the package will create its own logger.
func SetLogger(logger *logrus.Entry) {
	fields := storeLog.Data
	storeLog = logger.WithFields(fields)
}

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

	return s.backend.load(item, data)
}

// Store stores a virtcontainers item into a Store.
func (s *Store) Store(item Item, data interface{}) error {
	span, _ := s.trace("Store")
	defer span.Finish()

	span.SetTag("item", item)

	s.Lock()
	defer s.Unlock()

	return s.backend.store(item, data)
}

// Delete deletes all artifacts created by a Store.
// The Store is also removed from the manager.
func (s *Store) Delete() error {
	span, _ := s.trace("Store")
	defer span.Finish()

	s.Lock()
	defer s.Unlock()

	if err := s.backend.delete(); err != nil {
		return err
	}

	stores.removeStore(s.url)
	s.url = ""

	return nil
}

// Raw creates a raw item to be handled directly by the API caller.
// It returns a full URL to the item and the caller is responsible
// for handling the item through this URL.
func (s *Store) Raw(id string) (string, error) {
	span, _ := s.trace("Raw")
	defer span.Finish()

	s.Lock()
	defer s.Unlock()

	return s.backend.raw(id)
}

// ItemLock takes a lock on an item.
func (s *Store) ItemLock(item Item, exclusive bool) (string, error) {
	return s.backend.lock(item, exclusive)
}

// ItemUnlock unlocks an item.
func (s *Store) ItemUnlock(item Item, token string) error {
	return s.backend.unlock(item, token)
}
