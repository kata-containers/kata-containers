// Copyright (c) NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0
//
// kata-nvidia-cdi-list prints CDI mount/link lines for a staged driver root using
// the NVIDIA Container Toolkit (nvcdi) with an in-process go-nvml dgxa100 mock,
// so no GPU or host NVML is required. Output is consumed by nvidia_rootfs.sh.

package main

import (
	"flag"
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"sort"
	"strings"

	"github.com/NVIDIA/go-nvml/pkg/nvml"
	"github.com/NVIDIA/go-nvml/pkg/nvml/mock/dgxa100"

	"github.com/NVIDIA/nvidia-container-toolkit/pkg/nvcdi"
)

func main() {
	driverRoot := flag.String("driver-root", "", "absolute path to staged driver root (e.g. stage_one tarball contents)")
	hookPath := flag.String("hook-path", "", "path for nvidia-cdi-hook in generated edits (default: <driver-root>/usr/bin/nvidia-cdi-hook if it exists, else search PATH via toolkit default)")
	flag.Parse()

	if *driverRoot == "" {
		fmt.Fprintln(os.Stderr, "kata-nvidia-cdi-list: -driver-root is required")
		os.Exit(1)
	}
	absRoot, err := filepath.Abs(*driverRoot)
	if err != nil {
		fmt.Fprintf(os.Stderr, "kata-nvidia-cdi-list: bad driver root: %v\n", err)
		os.Exit(1)
	}
	if st, err := os.Stat(absRoot); err != nil || !st.IsDir() {
		fmt.Fprintf(os.Stderr, "kata-nvidia-cdi-list: driver root is not a directory: %s\n", absRoot)
		os.Exit(1)
	}

	ver, err := inferDriverVersion(absRoot)
	if err != nil {
		fmt.Fprintf(os.Stderr, "kata-nvidia-cdi-list: %v\n", err)
		os.Exit(1)
	}

	mock := newMockNVML(ver)

	resolvedHook := *hookPath
	if resolvedHook == "" {
		candidate := filepath.Join(absRoot, "usr", "bin", "nvidia-cdi-hook")
		if st, err := os.Stat(candidate); err == nil && !st.IsDir() {
			resolvedHook = candidate
		}
	}

	opts := []nvcdi.Option{
		nvcdi.WithDriverRoot(absRoot),
		nvcdi.WithDevRoot(absRoot),
		nvcdi.WithMode(nvcdi.ModeNvml),
		nvcdi.WithNvmlLib(mock),
		nvcdi.WithFeatureFlags(nvcdi.FeatureDisableNvsandboxUtils, nvcdi.FeatureEnableExplicitDriverLibraries),
	}
	if resolvedHook != "" {
		opts = append(opts, nvcdi.WithNVIDIACDIHookPath(resolvedHook))
	}

	lib, err := nvcdi.New(opts...)
	if err != nil {
		fmt.Fprintf(os.Stderr, "kata-nvidia-cdi-list: nvcdi.New: %v\n", err)
		os.Exit(1)
	}

	edits, err := lib.GetCommonEdits()
	if err != nil {
		fmt.Fprintf(os.Stderr, "kata-nvidia-cdi-list: GetCommonEdits: %v\n", err)
		os.Exit(1)
	}
	if edits == nil || edits.ContainerEdits == nil {
		fmt.Fprintln(os.Stderr, "kata-nvidia-cdi-list: empty common edits")
		os.Exit(1)
	}
	ce := edits.ContainerEdits

	for _, m := range ce.Mounts {
		if m == nil || m.HostPath == "" || m.ContainerPath == "" {
			continue
		}
		fmt.Printf("mount\t%s\t%s\n", m.HostPath, m.ContainerPath)
	}
	for _, d := range ce.DeviceNodes {
		if d == nil {
			continue
		}
		host := d.HostPath
		if host == "" {
			host = d.Path
		}
		if host == "" {
			continue
		}
		path := d.Path
		if path == "" {
			path = host
		}
		fmt.Printf("device\t%s\t%s\n", host, path)
	}
	for _, h := range ce.Hooks {
		if h == nil {
			continue
		}
		args := h.Args
		for i := 0; i < len(args)-1; i++ {
			if args[i] == "--link" {
				// Third empty column so rootfs shell can always `read -r kind a b`.
				fmt.Printf("link\t%s\t\n", args[i+1])
			}
		}
	}
}

// fullDriverVersion matches dotted NVIDIA driver versions (e.g. 550.163.01),
// excluding bare sonames like libcuda.so.1.
var fullDriverVersion = regexp.MustCompile(`^\d+\.\d+(\.\d+)+$`)

func inferDriverVersion(driverRoot string) (string, error) {
	globs := []string{
		filepath.Join(driverRoot, "usr", "lib", "x86_64-linux-gnu", "libnvidia-ml.so.*"),
		filepath.Join(driverRoot, "usr", "lib", "aarch64-linux-gnu", "libnvidia-ml.so.*"),
		filepath.Join(driverRoot, "usr", "lib64", "libnvidia-ml.so.*"),
		filepath.Join(driverRoot, "lib", "x86_64-linux-gnu", "libnvidia-ml.so.*"),
		filepath.Join(driverRoot, "lib", "aarch64-linux-gnu", "libnvidia-ml.so.*"),
		filepath.Join(driverRoot, "usr", "lib", "x86_64-linux-gnu", "libcuda.so.*"),
		filepath.Join(driverRoot, "usr", "lib", "aarch64-linux-gnu", "libcuda.so.*"),
		filepath.Join(driverRoot, "lib", "x86_64-linux-gnu", "libcuda.so.*"),
		filepath.Join(driverRoot, "lib", "aarch64-linux-gnu", "libcuda.so.*"),
	}
	const mlPrefix = "libnvidia-ml.so."
	const cudaPrefix = "libcuda.so."

	var candidates []string
	for _, g := range globs {
		matches, err := filepath.Glob(g)
		if err != nil {
			return "", err
		}
		for _, m := range matches {
			base := filepath.Base(m)
			switch {
			case strings.HasPrefix(base, mlPrefix):
				candidates = append(candidates, strings.TrimPrefix(base, mlPrefix))
			case strings.HasPrefix(base, cudaPrefix):
				candidates = append(candidates, strings.TrimPrefix(base, cudaPrefix))
			}
		}
	}
	if len(candidates) == 0 {
		return "", fmt.Errorf("could not infer driver version under %q (need libnvidia-ml.so.* or libcuda.so.*)", driverRoot)
	}
	var dotted []string
	for _, v := range candidates {
		if fullDriverVersion.MatchString(v) {
			dotted = append(dotted, v)
		}
	}
	if len(dotted) == 0 {
		return "", fmt.Errorf("no dotted driver version under %q (found %v); avoid unversioned .so.1-only trees", driverRoot, candidates)
	}
	sort.Strings(dotted)
	return dotted[len(dotted)-1], nil
}

func newMockNVML(driverVersion string) *dgxa100.Server {
	s := dgxa100.New()
	s.SystemGetDriverVersionFunc = func() (string, nvml.Return) {
		return driverVersion, nvml.SUCCESS
	}
	// Device count is unused for GetCommonEdits-only generation but keeps NVML
	// consistent if future code paths query devices.
	s.DeviceGetCountFunc = func() (int, nvml.Return) {
		return len(s.Devices), nvml.SUCCESS
	}
	for _, d := range s.Devices {
		dev, ok := d.(*dgxa100.Device)
		if !ok {
			continue
		}
		dev.GetMaxMigDeviceCountFunc = func() (int, nvml.Return) {
			return 0, nvml.SUCCESS
		}
	}
	return s
}
