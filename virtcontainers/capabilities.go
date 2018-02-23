//
// Copyright (c) 2017 Intel Corporation
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//

package virtcontainers

const (
	blockDeviceSupport = 1 << iota
	blockDeviceHotplugSupport
)

type capabilities struct {
	flags uint
}

func (caps *capabilities) isBlockDeviceSupported() bool {
	if caps.flags&blockDeviceSupport != 0 {
		return true
	}
	return false
}

func (caps *capabilities) setBlockDeviceSupport() {
	caps.flags = caps.flags | blockDeviceSupport
}

func (caps *capabilities) isBlockDeviceHotplugSupported() bool {
	if caps.flags&blockDeviceHotplugSupport != 0 {
		return true
	}
	return false
}

func (caps *capabilities) setBlockDeviceHotplugSupport() {
	caps.flags |= blockDeviceHotplugSupport
}
