# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

load("@prelude//:paths.bzl", "paths")
load(":packages.bzl", "merge_pkgs")
load(":toolchain.bzl", "GoToolchainInfo", "get_toolchain_cmd_args")

# Provider wrapping packages used for compiling.
GoPkgCompileInfo = provider(fields = [
    "pkgs",  # {str.type: "artifact"}
])

# Provider for test targets that test a library. Contains information for
# compiling the test and library code together as expected by go.
GoTestInfo = provider(fields = [
    "deps",  # ["dependency"]
    "srcs",  # ["source"]
    "pkg_name",  # str.type
])

def get_inherited_compile_pkgs(deps: ["dependency"]) -> {str.type: "artifact"}:
    return merge_pkgs([d[GoPkgCompileInfo].pkgs for d in deps if GoPkgCompileInfo in d])

def get_filtered_srcs(ctx: "context", srcs: ["artifact"], tests: bool.type = False) -> "cmd_args":
    """
    Filter the input sources based on build pragma
    """

    go_toolchain = ctx.attrs._go_toolchain[GoToolchainInfo]

    # Delegate to `go list` to filter out srcs with incompatible `// +build`
    # pragmas.
    filtered_srcs = ctx.actions.declare_output("__filtered_srcs__.txt")
    srcs_dir = ctx.actions.symlinked_dir(
        "__srcs__",
        {src.short_path: src for src in srcs},
    )
    filter_cmd = get_toolchain_cmd_args(go_toolchain, go_root = False)
    filter_cmd.add(go_toolchain.filter_srcs[RunInfo])
    filter_cmd.add(cmd_args(go_toolchain.go, format = "--go={}"))
    if tests:
        filter_cmd.add("--tests")
    filter_cmd.add(cmd_args(",".join(go_toolchain.tags), format = "--tags={}"))
    filter_cmd.add(cmd_args(filtered_srcs.as_output(), format = "--output={}"))
    filter_cmd.add(srcs_dir)
    ctx.actions.run(filter_cmd, category = "go_filter_srcs")

    # Add filtered srcs to compile command.
    return cmd_args(filtered_srcs, format = "@{}").hidden(srcs)

def _get_import_map(pkgs: [str.type]) -> {str.type: str.type}:
    """
    Return the import remappings for vendor paths.
    """

    vendor_prefixes = []
    vendor_prefixes.append("third-party-source/go")

    # TODO: add in implicit vendor prefixes inferred from project name.
    vendor_prefixes = reversed(sorted(vendor_prefixes))

    mappings = {}
    for pkg in pkgs:
        for prefix in vendor_prefixes:
            if paths.starts_with(pkg, prefix):
                mappings[paths.relativize(pkg, prefix)] = pkg
                break

    return mappings

def _assemble_cmd(
        ctx: "context",
        flags: [str.type] = []) -> "cmd_args":
    go_toolchain = ctx.attrs._go_toolchain[GoToolchainInfo]
    cmd = cmd_args()
    cmd.add(go_toolchain.assembler)
    cmd.add(flags)
    return cmd

def _compile_cmd(
        ctx: "context",
        pkg_name: str.type,
        pkgs: {str.type: "artifact"} = {},
        deps: ["dependency"] = [],
        flags: [str.type] = []) -> "cmd_args":
    go_toolchain = ctx.attrs._go_toolchain[GoToolchainInfo]

    cmd = cmd_args()
    cmd.add(go_toolchain.compiler)
    cmd.add("-p", pkg_name)
    cmd.add("-pack")
    cmd.add("-nolocalimports")
    cmd.add(flags)
    cmd.add("-buildid=")

    # Add Go pkgs inherited from deps to compiler search path.
    all_pkgs = merge_pkgs([pkgs, get_inherited_compile_pkgs(deps)])
    if all_pkgs:
        pkg_dir = ctx.actions.symlinked_dir(
            "__{}_compile_pkgs__".format(paths.basename(pkg_name)),
            {name + path.extension: path for name, path in all_pkgs.items()},
        )
        cmd.add("-I", pkg_dir)

    for mapping in _get_import_map(all_pkgs.keys()).items():
        cmd.add("-importmap", "{}={}".format(*mapping))

    return cmd

def compile(
        ctx: "context",
        pkg_name: str.type,
        srcs: "cmd_args",
        pkgs: {str.type: "artifact"} = {},
        deps: ["dependency"] = [],
        compile_flags: [str.type] = [],
        assemble_flags: [str.type] = []) -> "artifact":
    go_toolchain = ctx.attrs._go_toolchain[GoToolchainInfo]
    output = ctx.actions.declare_output(paths.basename(pkg_name) + ".a")

    cmd = get_toolchain_cmd_args(go_toolchain)
    cmd.add(go_toolchain.compile_wrapper[RunInfo])
    cmd.add(cmd_args(output.as_output(), format = "--output={}"))
    cmd.add(cmd_args(_compile_cmd(ctx, pkg_name, pkgs, deps, compile_flags), format = "--compiler={}"))
    cmd.add(cmd_args(_assemble_cmd(ctx, assemble_flags), format = "--assembler={}"))
    cmd.add(cmd_args(go_toolchain.packer, format = "--packer={}"))
    cmd.add(srcs)
    ctx.actions.run(cmd, category = "go_compile", identifier = paths.basename(pkg_name))

    return output
