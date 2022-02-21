// Copyright (c) 2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"os"
	"time"

	"github.com/containerd/containerd/events"
)

type forwarderType string

const (
	forwarderTypeLog        forwarderType = "log"
	forwarderTypeContainerd forwarderType = "containerd"

	// A time span used to wait for publish a containerd event,
	// once it costs a longer time than timeOut, it will be canceld.
	timeOut = 5 * time.Second

	// ttrpc address passed from container runtime.
	// For now containerd will pass the address, and CRI-O will not
	ttrpcAddressEnv = "TTRPC_ADDRESS"
)

type eventsForwarder interface {
	forward()
	forwarderType() forwarderType
}

type logForwarder struct {
	s *service
}

func (lf *logForwarder) forward() {
	for e := range lf.s.events {
		shimLog.WithField("topic", getTopic(e)).Infof("post event: %+v", e)
	}
}

func (lf *logForwarder) forwarderType() forwarderType {
	return forwarderTypeLog
}

type containerdForwarder struct {
	s         *service
	ctx       context.Context
	publisher events.Publisher
}

func (cf *containerdForwarder) forward() {
	for e := range cf.s.events {
		ctx, cancel := context.WithTimeout(cf.ctx, timeOut)
		err := cf.publisher.Publish(ctx, getTopic(e), e)
		cancel()
		if err != nil {
			shimLog.WithError(err).Error("post event")
		}
	}
}

func (cf *containerdForwarder) forwarderType() forwarderType {
	return forwarderTypeContainerd
}

func (s *service) newEventsForwarder(ctx context.Context, publisher events.Publisher) eventsForwarder {
	var forwarder eventsForwarder
	ttrpcAddress := os.Getenv(ttrpcAddressEnv)
	if ttrpcAddress == "" {
		// non containerd will use log forwarder to write events to log
		forwarder = &logForwarder{
			s: s,
		}
	} else {
		forwarder = &containerdForwarder{
			s:         s,
			ctx:       ctx,
			publisher: publisher,
		}
	}

	return forwarder
}
