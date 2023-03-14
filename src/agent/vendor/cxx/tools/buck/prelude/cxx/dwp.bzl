# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

load("@prelude//:local_only.bzl", "link_cxx_binary_locally")
load(":cxx_context.bzl", "get_cxx_toolchain_info")

def dwp_available(ctx: "context"):
    dwp = get_cxx_toolchain_info(ctx).binary_utilities_info.dwp
    return dwp != None

def run_dwp_action(
        ctx: "context",
        obj: "artifact",
        identifier: [str.type, None],
        category_suffix: [str.type, None],
        referenced_objects: ["_arglike", ["artifact"]],
        dwp_output: "artifact",
        local_only: bool.type,
        allow_huge_dwp: bool.type = False):
    args = cmd_args()
    dwp = get_cxx_toolchain_info(ctx).binary_utilities_info.dwp
    args.add("/bin/sh", "-c", '"$1" {}-o "$2" -e "$3" && touch "$2"'.format("--continue-on-cu-index-overflow " if allow_huge_dwp else ""), "")
    args.add(dwp, dwp_output.as_output(), obj)

    # All object/dwo files referenced in the library/executable are implicitly
    # processed by dwp.
    args.hidden(referenced_objects)

    category = "dwp"
    if category_suffix != None:
        category += "_" + category_suffix

    ctx.actions.run(
        args,
        category = category,
        identifier = identifier,
        local_only = local_only,
    )

def dwp(
        ctx: "context",
        # Executable/library to extra dwo paths from.
        obj: "artifact",
        # An identifier that will uniquely name this link action in the context of a category. Useful for
        # differentiating multiple link actions in the same rule.
        identifier: [str.type, None],
        # A category suffix that will be added to the category of the link action that is generated.
        category_suffix: [str.type, None],
        # All `.o`/`.dwo` paths referenced in `obj`.
        # TODO(T110378122): Ideally, referenced objects are a list of artifacts,
        # but currently we don't track them properly.  So, we just pass in the full
        # link line and extract all inputs from that, which is a bit of an
        # overspecification.
        referenced_objects: ["_arglike", ["artifact"]],
        # whether to enable dangerous option to allow huge dwp file. DWARF specs says dwp file
        # should be less than 4GB to ensure a valid .debug_cu_index, llvm-dwp errors out on huge
        # dwp file. allow_huge_dwp will toggle option to turn error to warning.
        allow_huge_dwp: bool.type = False) -> "artifact":
    # gdb/lldb expect to find a file named $file.dwp next to $file.
    output = ctx.actions.declare_output(obj.short_path + ".dwp")
    run_dwp_action(
        ctx,
        obj,
        identifier,
        category_suffix,
        referenced_objects,
        output,
        # dwp produces ELF files on the same size scale as the corresponding @obj.
        # The files are a concatentation of input DWARF debug info.
        # Caching dwp has the same issues as caching binaries, so use the same local_only policy.
        local_only = link_cxx_binary_locally(ctx),
        allow_huge_dwp = allow_huge_dwp,
    )
    return output
