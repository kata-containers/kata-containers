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

import (
	"fmt"
	"path/filepath"

	"github.com/mitchellh/mapstructure"
	"github.com/sirupsen/logrus"
)

// ProxyConfig is a structure storing information needed from any
// proxy in order to be properly initialized.
type ProxyConfig struct {
	Path  string
	Debug bool
}

// proxyParams is the structure providing specific parameters needed
// for the execution of the proxy binary.
type proxyParams struct {
	agentURL string
	logger   *logrus.Entry
}

// ProxyType describes a proxy type.
type ProxyType string

const (
	// NoopProxyType is the noopProxy.
	NoopProxyType ProxyType = "noopProxy"

	// NoProxyType is the noProxy.
	NoProxyType ProxyType = "noProxy"

	// CCProxyType is the ccProxy.
	CCProxyType ProxyType = "ccProxy"

	// KataProxyType is the kataProxy.
	KataProxyType ProxyType = "kataProxy"

	// KataBuiltInProxyType is the kataBuiltInProxy.
	KataBuiltInProxyType ProxyType = "kataBuiltInProxy"
)

const (
	// Number of seconds to wait for the proxy to respond to a connection
	// request.
	waitForProxyTimeoutSecs = 5.0
)

const (
	// unix socket type of console
	consoleProtoUnix = "unix"

	// pty type of console. Used mostly by kvmtools.
	consoleProtoPty = "pty"
)

// Set sets a proxy type based on the input string.
func (pType *ProxyType) Set(value string) error {
	switch value {
	case "noopProxy":
		*pType = NoopProxyType
		return nil
	case "noProxy":
		*pType = NoProxyType
		return nil
	case "ccProxy":
		*pType = CCProxyType
		return nil
	case "kataProxy":
		*pType = KataProxyType
		return nil
	case "kataBuiltInProxy":
		*pType = KataBuiltInProxyType
		return nil
	default:
		return fmt.Errorf("Unknown proxy type %s", value)
	}
}

// String converts a proxy type to a string.
func (pType *ProxyType) String() string {
	switch *pType {
	case NoopProxyType:
		return string(NoopProxyType)
	case NoProxyType:
		return string(NoProxyType)
	case CCProxyType:
		return string(CCProxyType)
	case KataProxyType:
		return string(KataProxyType)
	case KataBuiltInProxyType:
		return string(KataBuiltInProxyType)
	default:
		return ""
	}
}

// newProxy returns a proxy from a proxy type.
func newProxy(pType ProxyType) (proxy, error) {
	switch pType {
	case NoopProxyType:
		return &noopProxy{}, nil
	case NoProxyType:
		return &noProxy{}, nil
	case CCProxyType:
		return &ccProxy{}, nil
	case KataProxyType:
		return &kataProxy{}, nil
	case KataBuiltInProxyType:
		return &kataBuiltInProxy{}, nil
	default:
		return &noopProxy{}, nil
	}
}

// newProxyConfig returns a proxy config from a generic PodConfig handler,
// after it properly checked the configuration was valid.
func newProxyConfig(podConfig *PodConfig) (ProxyConfig, error) {
	if podConfig == nil {
		return ProxyConfig{}, fmt.Errorf("Pod config cannot be nil")
	}

	var config ProxyConfig
	switch podConfig.ProxyType {
	case KataProxyType:
		fallthrough
	case CCProxyType:
		if err := mapstructure.Decode(podConfig.ProxyConfig, &config); err != nil {
			return ProxyConfig{}, err
		}
	}

	if config.Path == "" {
		return ProxyConfig{}, fmt.Errorf("Proxy path cannot be empty")
	}

	return config, nil
}

func defaultProxyURL(pod Pod, socketType string) (string, error) {
	switch socketType {
	case SocketTypeUNIX:
		socketPath := filepath.Join(runStoragePath, pod.id, "proxy.sock")
		return fmt.Sprintf("unix://%s", socketPath), nil
	case SocketTypeVSOCK:
		// TODO Build the VSOCK default URL
		return "", nil
	default:
		return "", fmt.Errorf("Unknown socket type: %s", socketType)
	}
}

func isProxyBuiltIn(pType ProxyType) bool {
	return pType == KataBuiltInProxyType
}

// proxy is the virtcontainers proxy interface.
type proxy interface {
	// start launches a proxy instance for the specified pod, returning
	// the PID of the process and the URL used to connect to it.
	start(pod Pod, params proxyParams) (int, string, error)

	// stop terminates a proxy instance after all communications with the
	// agent inside the VM have been properly stopped.
	stop(pod Pod, pid int) error
}
