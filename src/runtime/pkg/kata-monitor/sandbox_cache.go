// Copyright (c) 2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

package katamonitor

import (
	"context"
	"fmt"
	"regexp"
	"sync"

	"github.com/containerd/containerd"
	"github.com/sirupsen/logrus"

	"encoding/json"

	eventstypes "github.com/containerd/containerd/api/events"
	"github.com/containerd/containerd/events"
	"github.com/containerd/typeurl"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/types"

	// Register grpc event types
	_ "github.com/containerd/containerd/api/events"
)

type sandboxCache struct {
	*sync.Mutex
	sandboxes map[string]string
}

func (sc *sandboxCache) getAllSandboxes() map[string]string {
	sc.Lock()
	defer sc.Unlock()
	return sc.sandboxes
}

func (sc *sandboxCache) getSandboxNamespace(sandbox string) (string, error) {
	sc.Lock()
	defer sc.Unlock()

	if val, found := sc.sandboxes[sandbox]; found {
		return val, nil
	}

	return "", fmt.Errorf("sandbox %s not in cache", sandbox)
}

func (sc *sandboxCache) deleteIfExists(id string) (string, bool) {
	sc.Lock()
	defer sc.Unlock()

	if val, found := sc.sandboxes[id]; found {
		delete(sc.sandboxes, id)
		return val, true
	}

	// not in sandbox cache
	return "", false
}

func (sc *sandboxCache) putIfNotExists(id, value string) bool {
	sc.Lock()
	defer sc.Unlock()

	if _, found := sc.sandboxes[id]; !found {
		sc.sandboxes[id] = value
		return true
	}

	// already in sandbox cache
	return false
}

func (sc *sandboxCache) init(sandboxes map[string]string) {
	sc.Lock()
	defer sc.Unlock()
	sc.sandboxes = sandboxes
}

// startEventsListener will boot a thread to listen container events to manage sandbox cache
func (sc *sandboxCache) startEventsListener(addr string) error {
	client, err := containerd.New(addr)
	if err != nil {
		return err
	}
	defer client.Close()

	ctx := context.Background()

	eventsClient := client.EventService()
	containerClient := client.ContainerService()

	// only need create/delete events.
	eventFilters := []string{
		`topic=="/containers/create"`,
		`topic=="/containers/delete"`,
	}

	runtimeNameRegexp, err := regexp.Compile(types.KataRuntimeNameRegexp)
	if err != nil {
		return err
	}

	eventsCh, errCh := eventsClient.Subscribe(ctx, eventFilters...)
	for {
		var e *events.Envelope
		select {
		case e = <-eventsCh:
		case err = <-errCh:
			monitorLog.WithError(err).Warn("get error from error chan")
			return err
		}

		if e != nil {
			var eventBody []byte
			if e.Event != nil {
				v, err := typeurl.UnmarshalAny(e.Event)
				if err != nil {
					monitorLog.WithError(err).Warn("cannot unmarshal an event from Any")
					continue
				}
				eventBody, err = json.Marshal(v)
				if err != nil {
					monitorLog.WithError(err).Warn("cannot marshal Any into JSON")
					continue
				}
			}

			if e.Topic == "/containers/create" {
				// Namespace: k8s.io
				// Topic: /containers/create
				// Event: {
				//          "id":"6a2e22e6fffaf1dec63ddabf587ed56069b1809ba67a0d7872fc470528364e66",
				//          "image":"k8s.gcr.io/pause:3.1",
				//          "runtime":{"name":"io.containerd.kata.v2"}
				//        }
				cc := eventstypes.ContainerCreate{}
				err := json.Unmarshal(eventBody, &cc)
				if err != nil {
					monitorLog.WithError(err).WithField("body", string(eventBody)).Warn("unmarshal ContainerCreate failed")
					continue
				}

				// skip non-kata contaienrs
				if !runtimeNameRegexp.MatchString(cc.Runtime.Name) {
					continue
				}

				c, err := getContainer(containerClient, e.Namespace, cc.ID)
				if err != nil {
					monitorLog.WithError(err).WithField("container", cc.ID).Warn("failed to get container")
					continue
				}

				// if the container is a sandbox container,
				// means the VM is started, and can start to collect metrics from the VM.
				if isSandboxContainer(&c) {
					// we can simply put the contaienrid in sandboxes list if the container is a sandbox container
					sc.putIfNotExists(cc.ID, e.Namespace)
					monitorLog.WithField("container", cc.ID).Info("add sandbox to cache")
				}
			} else if e.Topic == "/containers/delete" {
				// Namespace: k8s.io
				// Topic: /containers/delete
				// Event: {
				//          "id":"73ec10d2e38070f930310687ab46bbaa532c79d5680fd7f18fff99f759d9385e"
				//        }
				cd := &eventstypes.ContainerDelete{}
				err := json.Unmarshal(eventBody, &cd)
				if err != nil {
					monitorLog.WithError(err).WithField("body", string(eventBody)).Warn("unmarshal ContainerDelete failed")
				}

				// if container in sandboxes list, it must be the pause container in the sandbox,
				// so the contaienr id is the sandbox id
				// we can simply delete the contaienr from sandboxes list
				// the last container in a sandbox is deleted, means the VM will stop.
				_, deleted := sc.deleteIfExists(cd.ID)
				monitorLog.WithFields(logrus.Fields{"container": cd.ID, "result": deleted}).Info("delete sandbox from cache")
			} else {
				monitorLog.WithFields(logrus.Fields{"Namespace": e.Namespace, "Topic": e.Topic, "Event": string(eventBody)}).Error("other events")
			}

		}
	}
}
