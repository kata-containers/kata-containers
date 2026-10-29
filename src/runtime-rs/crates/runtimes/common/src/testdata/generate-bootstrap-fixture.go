// Copyright 2026 Kata Contributors
// SPDX-License-Identifier: Apache-2.0
// Run from a containerd v2.3.3 checkout: go run <this-file> <output.pb>
// Calls the actual shim.Command path; does not execute the returned command.

package main

import (
	"context"
	"encoding/hex"
	"fmt"
	"io"
	"os"

	runtimeoptions "github.com/containerd/containerd/api/types/runtimeoptions/v1"
	"github.com/containerd/containerd/v2/pkg/namespaces"
	"github.com/containerd/containerd/v2/pkg/shim"
	"github.com/containerd/typeurl/v2"
)

func main() {
	envelope, err := typeurl.MarshalAny(&runtimeoptions.Options{ConfigPath: "/opt/kata/custom/configuration-qemu.toml"})
	if err != nil {
		panic(err)
	}
	command, err := shim.Command(namespaces.WithNamespace(context.Background(), "k8s.io"), &shim.CommandConfig{
		ID: "bootstrap-fixture-sandbox", RuntimePath: "/opt/kata/runtime-rs/bin/containerd-shim-kata-v2",
		GRPCAddress: "/run/containerd/containerd.sock", TTRPCAddress: "/run/containerd/containerd.sock.ttrpc",
		WorkDir: "/fixture", Opts: typeurl.MarshalProto(envelope), Action: "start",
	})
	if err != nil {
		panic(err)
	}
	data, err := io.ReadAll(command.Stdin)
	if err != nil {
		panic(err)
	}
	if err := os.WriteFile(os.Args[1], data, 0600); err != nil {
		panic(err)
	}
	fmt.Println(hex.EncodeToString(data))
}
