//
// Copyright (c) 2016 Intel Corporation
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

type mockHypervisor struct {
}

func (m *mockHypervisor) init(pod *Pod) error {
	valid, err := pod.config.HypervisorConfig.valid()
	if valid == false || err != nil {
		return err
	}

	return nil
}

func (m *mockHypervisor) capabilities() capabilities {
	return capabilities{}
}

func (m *mockHypervisor) createPod(podConfig PodConfig) error {
	return nil
}

func (m *mockHypervisor) startPod() error {
	return nil
}

func (m *mockHypervisor) waitPod(timeout int) error {
	return nil
}

func (m *mockHypervisor) stopPod() error {
	return nil
}

func (m *mockHypervisor) pausePod() error {
	return nil
}

func (m *mockHypervisor) resumePod() error {
	return nil
}

func (m *mockHypervisor) addDevice(devInfo interface{}, devType deviceType) error {
	return nil
}

func (m *mockHypervisor) hotplugAddDevice(devInfo interface{}, devType deviceType) error {
	return nil
}

func (m *mockHypervisor) hotplugRemoveDevice(devInfo interface{}, devType deviceType) error {
	return nil
}

func (m *mockHypervisor) getPodConsole(podID string) string {
	return ""
}
