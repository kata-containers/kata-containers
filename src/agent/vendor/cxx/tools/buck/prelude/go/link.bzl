# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

load("@prelude//cxx:cxx_library_utility.bzl", "cxx_inherited_link_info")
load(
    "@prelude//cxx:cxx_link_utility.bzl",
    "executable_shared_lib_arguments",
)
load(
    "@prelude//linking:link_info.bzl",
    "LinkStyle",
    "get_link_args",
    "unpack_link_args",
)
load(
    "@prelude//linking:shared_libraries.bzl",
    "SharedLibraryInfo",
    "merge_shared_libraries",
    "traverse_shared_library_info",
)
load(
    "@prelude//utils:utils.bzl",
    "map_idx",
)
load(":packages.bzl", "merge_pkgs")
load(":toolchain.bzl", "GoToolchainInfo", "get_toolchain_cmd_args")

# Provider wrapping packages used for linking.
GoPkgLinkInfo = provider(fields = [
    "pkgs",  # {str.type: "artifact"}
])

def get_inherited_link_pkgs(deps: ["dependency"]) -> {str.type: "artifact"}:
    return merge_pkgs([d[GoPkgLinkInfo].pkgs for d in deps if GoPkgLinkInfo in d])

def _process_shared_dependencies(ctx: "context", artifact: "artifact", deps: ["dependency"]):
    """
    Provides files and linker args needed to for binaries with shared library linkage.
    - the runtime files needed to run binary linked with shared libraries
    - linker arguments for shared libraries
    """
    if ctx.attrs.link_style != "shared":
        return ([], [])

    shlib_info = merge_shared_libraries(
        ctx.actions,
        deps = filter(None, map_idx(SharedLibraryInfo, deps)),
    )
    shared_libs = {}
    for name, shared_lib in traverse_shared_library_info(shlib_info).items():
        shared_libs[name] = shared_lib.lib

    extra_link_args, runtime_files, _ = executable_shared_lib_arguments(
        ctx.actions,
        ctx.attrs._go_toolchain[GoToolchainInfo].cxx_toolchain_for_linking,
        artifact,
        shared_libs,
    )

    return (runtime_files, extra_link_args)

def link(ctx: "context", main: "artifact", pkgs: {str.type: "artifact"} = {}, deps: ["dependency"] = [], link_mode = None):
    go_toolchain = ctx.attrs._go_toolchain[GoToolchainInfo]
    output = ctx.actions.declare_output(ctx.label.name)

    cmd = get_toolchain_cmd_args(go_toolchain)

    cmd.add(go_toolchain.linker)

    cmd.add("-o", output.as_output())
    cmd.add("-buildmode", "exe")  # TODO(agallagher): support other modes
    cmd.add("-buildid=")  # Setting to a static buildid helps make the binary reproducible.

    # Add inherited Go pkgs to library search path.
    all_pkgs = merge_pkgs([pkgs, get_inherited_link_pkgs(deps)])
    pkgs_dir = ctx.actions.symlinked_dir(
        "__link_pkgs__",
        {name + path.extension: path for name, path in all_pkgs.items()},
    )
    cmd.add("-L", pkgs_dir)

    link_style = ctx.attrs.link_style
    if link_style == None:
        link_style = "static"

    runtime_files, extra_link_args = _process_shared_dependencies(ctx, main, deps)

    # Gather external link args from deps.
    ext_links = get_link_args(
        cxx_inherited_link_info(ctx, deps),
        LinkStyle(link_style),
    )
    ext_link_args = cmd_args(unpack_link_args(ext_links))
    ext_link_args.add(cmd_args(extra_link_args, quote = "shell"))

    if not link_mode:
        link_mode = "external"
    cmd.add("-linkmode", link_mode)

    if link_mode == "external":
        # Delegate to C++ linker...
        # TODO: It feels a bit inefficient to generate a wrapper file for every
        # link.  Is there some way to etract the first arg of `RunInfo`?  Or maybe
        # we can generate te platform-specific stuff once and re-use?
        cxx_toolchain = go_toolchain.cxx_toolchain_for_linking
        cxx_link_cmd = cmd_args(
            [
                cxx_toolchain.linker_info.linker,
                cxx_toolchain.linker_info.linker_flags,
                go_toolchain.external_linker_flags,
                ext_link_args,
                "\"$@\"",
            ],
            delimiter = " ",
        )
        linker_wrapper, _ = ctx.actions.write(
            "__{}_cxx_link_wrapper__.sh".format(ctx.label.name),
            ["#!/bin/sh", cxx_link_cmd],
            allow_args = True,
            is_executable = True,
        )
        cmd.add("-extld", linker_wrapper).hidden(cxx_link_cmd)

    if ctx.attrs.linker_flags:
        cmd.add(ctx.attrs.linker_flags)

    cmd.add(main)

    ctx.actions.run(cmd, category = "go_link")

    return (output, runtime_files)
