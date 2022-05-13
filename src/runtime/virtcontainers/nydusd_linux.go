// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"bufio"
	"bytes"
	"context"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"io"
	"io/ioutil"
	"net"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"strings"
	"syscall"
	"time"

	"github.com/containernetworking/plugins/pkg/ns"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils/katatrace"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils/retry"
	"github.com/pkg/errors"
	"github.com/sirupsen/logrus"
)

const (
	shimNsPath = "/proc/self/ns/net"
)

func startInShimNS(cmd *exec.Cmd) error {
	// Create nydusd in shim netns as it needs to access host network
	return doNetNS(shimNsPath, func(_ ns.NetNS) error {
		return cmd.Start()
	})
}
