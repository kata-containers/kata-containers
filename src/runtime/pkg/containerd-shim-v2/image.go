// Copyright (c) 2021 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"time"

	"github.com/containerd/containerd/plugin"
	"github.com/containerd/containerd/runtime/v2/shim"
	"github.com/containerd/containerd/runtime/v2/task"
	"github.com/containerd/ttrpc"
	"github.com/pkg/errors"
	"github.com/sirupsen/logrus"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils/katatrace"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/image"
)

func init() {
	plugin.Register(&plugin.Registration{
		Type:     plugin.TTRPCPlugin,
		ID:       "image",
		Requires: []plugin.Type{"*"},
		InitFn:   initImageService,
	})
}

type ImageService struct {
	s *service
}

func initImageService(ic *plugin.InitContext) (interface{}, error) {
	i, err := ic.GetByID(plugin.TTRPCPlugin, "task")
	if err != nil {
		return nil, errors.Errorf("get task plugin error. %v", err)
	}
	task := i.(*shim.TaskService)
	s := task.Local.(*service)
	is := &ImageService{s: s}
	return is, nil
}

func (is *ImageService) RegisterTTRPC(server *ttrpc.Server) error {
	task.RegisterImageService(server, is)
	return nil
}

// Pull image and unbundle ready for container creation
func (is *ImageService) PullImage(ctx context.Context, req *task.PullImageRequest) (_ *task.PullImageResponse, err error) {
	shimLog.WithField("image", req.Image).Debug("PullImage() start")
	defer shimLog.WithField("image", req.Image).Debug("PullImage() end")
	span, spanCtx := katatrace.Trace(is.s.rootCtx, shimLog, "PullImage", shimTracingTags)
	defer span.End()

	start := time.Now()
	defer func() {
		err = toGRPC(err)
		rpcDurationsHistogram.WithLabelValues("pullimage").Observe(float64(time.Since(start).Nanoseconds() / int64(time.Millisecond)))
	}()

	is.s.mu.Lock()
	defer is.s.mu.Unlock()

	shimLog.WithFields(logrus.Fields{
		"image": req.Image,
	}).Debug("Making image pull request")

	r := &image.PullImageReq{
		Image: req.Image,
	}

	resp, err := is.s.sandbox.PullImage(spanCtx, r)
	if err != nil {
		shimLog.Errorf("kata runtime PullImage err. %v", err)
		return nil, err
	}
	return &task.PullImageResponse{
		ImageRef: resp.ImageRef,
	}, err
}
