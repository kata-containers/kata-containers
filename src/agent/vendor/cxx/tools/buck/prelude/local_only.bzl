# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

load("@prelude//cxx:cxx_context.bzl", "get_cxx_toolchain_info")
load("@prelude//utils:utils.bzl", "value_or")

def _is_core_tool(ctx: "context"):
    return "is_core_tool" in ctx.attrs.labels

def link_cxx_binary_locally(ctx: "context", cxx_toolchain: ["CxxToolchainInfo", None] = None) -> bool.type:
    # core tools are linked on RE because they are a) small enough to do so and
    # b) don't get build stamping so they do cache correctly.
    if _is_core_tool(ctx):
        return False
    if not cxx_toolchain:
        cxx_toolchain = get_cxx_toolchain_info(ctx)
    link_locally = cxx_toolchain.linker_info.link_binaries_locally
    if hasattr(ctx.attrs, "_link_binaries_locally_override"):
        return value_or(ctx.attrs._link_binaries_locally_override, link_locally)
    return link_locally

def package_python_locally(ctx: "context", python_toolchain: "PythonToolchainInfo") -> bool.type:
    if _is_core_tool(ctx) or getattr(ctx.attrs, "_package_remotely", False):
        return False

    return python_toolchain.build_standalone_binaries_locally
