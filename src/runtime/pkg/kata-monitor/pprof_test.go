// Copyright (c) 2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

package katamonitor

import (
	"fmt"
	"io/ioutil"
	"net/http"
	"os"
	"path/filepath"
	"sync"
	"testing"
	"time"

	"github.com/stretchr/testify/assert"
)

func TestComposeSocketAddress(t *testing.T) {
	assert := assert.New(t)
	path := fmt.Sprintf("/tmp/TestComposeSocketAddress-%d", time.Now().Nanosecond())
	statePath := filepath.Join(path, "io.containerd.runtime.v2.task")

	sandboxes := map[string]string{"foo": "ns-foo", "bar": "ns-bar"}
	defer func() {
		os.RemoveAll(path)
	}()

	for sandbox, ns := range sandboxes {
		err := os.MkdirAll(filepath.Join(statePath, ns, sandbox), 0755)
		assert.Nil(err)
		f := filepath.Join(statePath, ns, sandbox, "monitor_address")
		err = ioutil.WriteFile(f, []byte(sandbox), 0644)
		assert.Nil(err)
	}

	km := &KataMonitor{
		containerdStatePath: path,
		sandboxCache: &sandboxCache{
			Mutex:     &sync.Mutex{},
			sandboxes: sandboxes,
		},
	}

	testCases := []struct {
		url  string
		err  bool
		addr string
	}{
		{
			url:  "http://localhost:6060/debug/vars",
			err:  true,
			addr: "",
		},
		{
			url:  "http://localhost:6060/debug/vars?sandbox=abc",
			err:  true,
			addr: "",
		},
		{
			url:  "http://localhost:6060/debug/vars?sandbox=foo",
			err:  false,
			addr: "foo",
		},
		{
			url:  "http://localhost:6060/debug/vars?sandbox=bar",
			err:  false,
			addr: "bar",
		},
	}

	for _, tc := range testCases {
		r, err := http.NewRequest("GET", tc.url, nil)
		assert.Nil(err)

		addr, err := km.composeSocketAddress(r)

		assert.Equal(tc.err, err != nil)
		assert.Equal(tc.addr, addr)
	}
}
