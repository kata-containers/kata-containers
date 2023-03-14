# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

load("@prelude//:paths.bzl", "paths")
load(
    "@prelude//cxx:compile.bzl",
    "CxxSrcWithFlags",  # @unused Used as a type
)
load("@prelude//cxx:cxx_library.bzl", "cxx_compile_srcs")
load("@prelude//cxx:cxx_toolchain_types.bzl", "CxxToolchainInfo")
load(
    "@prelude//cxx:cxx_types.bzl",
    "CxxRuleConstructorParams",  # @unused Used as a type
)
load("@prelude//cxx:headers.bzl", "cxx_get_regular_cxx_headers_layout", "prepare_headers")
load(
    "@prelude//cxx:preprocessor.bzl",
    "CPreprocessor",
    "cxx_inherited_preprocessor_infos",
    "cxx_merge_cpreprocessors",
    "cxx_private_preprocessor_info",
)
load(
    "@prelude//linking:link_info.bzl",
    "Linkage",
    "MergedLinkInfo",
    "merge_link_infos",
)
load(
    "@prelude//linking:shared_libraries.bzl",
    "SharedLibraryInfo",
    "merge_shared_libraries",
)
load(
    "@prelude//utils:utils.bzl",
    "expect",
    "map_idx",
)
load(":compile.bzl", "GoPkgCompileInfo", "compile", "get_filtered_srcs", "get_inherited_compile_pkgs")
load(":link.bzl", "GoPkgLinkInfo", "get_inherited_link_pkgs")
load(":packages.bzl", "go_attr_pkg_name", "merge_pkgs")
load(":toolchain.bzl", "GoToolchainInfo", "get_toolchain_cmd_args")

def _cgo(
        ctx: "context",
        srcs: ["artifact"],
        own_pre: [CPreprocessor.type],
        inherited_pre: ["CPreprocessorInfo"]) -> (["artifact"], ["artifact"], ["artifact"]):
    """
    Run `cgo` on `.go` sources to genreate Go, C, and C-Header sources.
    """

    pre = cxx_merge_cpreprocessors(ctx, own_pre, inherited_pre)
    pre_args = pre.set.project_as_args("args")

    gen_dir = "cgo_gen"

    go_srcs = []
    c_headers = []
    c_srcs = []
    go_srcs.append(ctx.actions.declare_output(paths.join(gen_dir, "_cgo_gotypes.go")))
    c_srcs.append(ctx.actions.declare_output(paths.join(gen_dir, "_cgo_export.c")))
    c_headers.append(ctx.actions.declare_output(paths.join(gen_dir, "_cgo_export.h")))
    for src in srcs:
        go_srcs.append(ctx.actions.declare_output(paths.join(gen_dir, paths.replace_extension(src.basename, ".cgo1.go"))))
        c_srcs.append(ctx.actions.declare_output(paths.join(gen_dir, paths.replace_extension(src.basename, ".cgo2.c"))))

    # Return a `cmd_args` to use as the generated sources.
    go_toolchain = ctx.attrs._go_toolchain[GoToolchainInfo]
    expect(go_toolchain.cgo != None)
    expect(CxxToolchainInfo in ctx.attrs._cxx_toolchain)
    cxx_toolchain = ctx.attrs._cxx_toolchain[CxxToolchainInfo]

    cmd = get_toolchain_cmd_args(go_toolchain, go_root = False)
    cmd.add(go_toolchain.cgo_wrapper[RunInfo])
    cmd.add(cmd_args(go_toolchain.cgo, format = "--cgo={}"))

    # TODO(agallagher): cgo outputs a dir with generated sources, but I'm not
    # sure how to pass in an output dir *and* enumerate the sources we know will
    # generated w/o v2 complaining that the output dir conflicts with the nested
    # artifacts.
    cmd.add(cmd_args(go_srcs[0].as_output(), format = "--output={}/.."))
    cmd.add(cmd_args(cxx_toolchain.c_compiler_info.preprocessor, format = "--cpp={}"))
    cmd.add(cmd_args(pre_args, format = "--cpp={}"))
    cmd.add(srcs)
    for src in go_srcs + c_headers + c_srcs:
        cmd.hidden(src.as_output())
    ctx.actions.run(cmd, category = "cgo")

    return go_srcs, c_headers, c_srcs

def cgo_library_impl(ctx: "context") -> ["provider"]:
    pkg_name = go_attr_pkg_name(ctx)

    # Gather preprocessor inputs.
    (own_pre, _) = cxx_private_preprocessor_info(
        ctx,
        cxx_get_regular_cxx_headers_layout(ctx),
    )
    inherited_pre = cxx_inherited_preprocessor_infos(ctx.attrs.deps)

    # Seprate sources into C++ and CGO sources.
    cgo_srcs = []
    cxx_srcs = []
    for src in ctx.attrs.srcs:
        if src.extension == ".go":
            cgo_srcs.append(src)
        elif src.extension in (".c", ".cpp"):
            cxx_srcs.append(src)
        else:
            fail("unexpected extension: {}".format(src))

    # Generate CGO and C sources.
    go_srcs, c_headers, c_srcs = _cgo(ctx, cgo_srcs, [own_pre], inherited_pre)
    cxx_srcs.extend(c_srcs)

    # Wrap the generated CGO C headers in a CPreprocessor object for compiling.
    cgo_headers_pre = CPreprocessor(args = [
        "-I",
        prepare_headers(
            ctx,
            {h.basename: h for h in c_headers},
            "cgo-private-headers",
        ).include_path,
    ])

    link_style = ctx.attrs.link_style
    if link_style == None:
        link_style = "static"

    # Copmile C++ sources into object files.
    c_compile_cmds = cxx_compile_srcs(
        ctx,
        CxxRuleConstructorParams(
            rule_type = "cgo_library",
            headers_layout = cxx_get_regular_cxx_headers_layout(ctx),
            srcs = [CxxSrcWithFlags(file = src) for src in cxx_srcs],
        ),
        # Create private header tree and propagate via args.
        [own_pre, cgo_headers_pre],
        inherited_pre,
        [],
        Linkage(link_style),
    )

    compiled_objects = c_compile_cmds.objects
    if link_style != "static":
        compiled_objects = c_compile_cmds.pic_objects

    # Merge all sources together to pass to the Go compile step.
    all_srcs = cmd_args(go_srcs + compiled_objects)
    if ctx.attrs.go_srcs:
        all_srcs.add(get_filtered_srcs(ctx, ctx.attrs.go_srcs))

    # Build Go library.
    lib = compile(
        ctx,
        pkg_name,
        all_srcs,
        deps = ctx.attrs.deps + ctx.attrs.exported_deps,
    )

    pkgs = {pkg_name: lib}
    return [
        DefaultInfo(default_outputs = [lib]),
        GoPkgCompileInfo(pkgs = merge_pkgs([
            pkgs,
            get_inherited_compile_pkgs(ctx.attrs.exported_deps),
        ])),
        GoPkgLinkInfo(pkgs = merge_pkgs([
            pkgs,
            get_inherited_link_pkgs(ctx.attrs.deps + ctx.attrs.exported_deps),
        ])),
        merge_link_infos(ctx, filter(None, [d.get(MergedLinkInfo) for d in ctx.attrs.deps])),
        merge_shared_libraries(
            ctx.actions,
            deps = filter(None, map_idx(SharedLibraryInfo, ctx.attrs.deps)),
        ),
    ]
