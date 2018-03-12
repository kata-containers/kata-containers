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

// This is a dummy proxy implementation of the proxy interface, only
// used for testing purpose.
type noopProxy struct{}

var noopProxyURL = "noopProxyURL"

// register is the proxy start implementation for testing purpose.
// It does nothing.
func (p *noopProxy) start(pod Pod, params proxyParams) (int, string, error) {
	return 0, noopProxyURL, nil
}

// stop is the proxy stop implementation for testing purpose.
// It does nothing.
func (p *noopProxy) stop(pod Pod, pid int) error {
	return nil
}
