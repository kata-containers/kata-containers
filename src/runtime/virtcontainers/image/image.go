// Copyright (c) 2021 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//
package image

import "context"

type PullImageReq struct {
	// Name of the image. e.g. docker.io/library/busybox:latest
	Image string
}

type PullImageResp struct {
	// Reference to the image in use. For most runtimes, this should be an
	// image ID or digest.
	ImageRef string
}

type ImageService interface {
	// pull image in guest
	PullImage(ctx context.Context, req *PullImageReq) (*PullImageResp, error)
}
