// Copyright (c) 2019 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//
// template implements base vm factory with vm templating.

package template

// templateDeviceStateSize denotes device state size when
// mount tmpfs.
// when bypass-shared-memory is not support like arm64,
// creating template will occupy more space. That's why we
// put it here.
const templateDeviceStateSize = 8
