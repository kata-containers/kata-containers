# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

# Implementation of the Rust build rules.

load("@prelude//cxx:cxx_toolchain_types.bzl", "CxxPlatformInfo")
load(
    "@prelude//linking:link_info.bzl",
    "LinkStyle",
    "MergedLinkInfo",
    "merge_link_infos",
)
load(
    "@prelude//linking:shared_libraries.bzl",
    "SharedLibraryInfo",
)
load("@prelude//utils:platform_flavors_util.bzl", "by_platform")
load("@prelude//utils:utils.bzl", "flatten")

# Override dylib crates to static_pic, so that Rust code is always
# statically linked.
# In v1 we always linked Rust deps statically, even for "shared" link style
# That shouldn't be necessary, but fully shared needs some more debugging,
# so default to v1 behaviour. (Should be controlled with the `rust.force_rlib` option)
FORCE_RLIB = True

# Output of a Rust compilation
RustLinkInfo = provider(fields = [
    # crate - crate name
    "crate",
    # styles - information about each LinkStyle as RustLinkStyleInfo
    # {LinkStyle: RustLinkStyleInfo}
    "styles",
    # Propagate non-rust native linkable dependencies through rust libraries.
    "non_rust_exported_link_deps",
    # Propagate non-rust native linkable info through rust libraries.
    "non_rust_link_info",
    # Propagate non-rust shared libraries through rust libraries.
    "non_rust_shared_libs",
])

# Information which is keyed on link_style
RustLinkStyleInfo = record(
    # Path to library or binary
    rlib = field("artifact"),
    # Transitive dependencies which are relevant to consumer
    # This is a dict from artifact to None (we don't have sets)
    transitive_deps = field({"artifact": None}),

    # Path for library metadata (used for check or pipelining)
    rmeta = field("artifact"),
    # Transitive rmeta deps
    transitive_rmeta_deps = field({"artifact": None}),
)

def style_info(info: RustLinkInfo.type, link_style: LinkStyle.type) -> RustLinkStyleInfo.type:
    if FORCE_RLIB and link_style == LinkStyle("shared"):
        link_style = LinkStyle("static_pic")

    return info.styles[link_style]

def cxx_by_platform(ctx: "context", xs: [(str.type, "_a")]) -> "_a":
    platform = ctx.attrs._cxx_toolchain[CxxPlatformInfo].name
    return flatten(by_platform([platform], xs))

# A Rust dependency
RustDependency = record(
    # The actual dependency
    dep = field("dependency"),
    # The local name, if any (for `named_deps`)
    name = field([None, str.type]),
    # Any flags for the dependency (`flagged_deps`), which are passed on to rustc.
    flags = field([str.type]),
)

# Returns all first-order dependencies, resolving the ones from "platform_deps"
def resolve_deps(ctx: "context") -> [RustDependency.type]:
    return [
        RustDependency(name = name, dep = dep, flags = flags)
        # The `getattr`s are needed for when we're operating on
        # `prebuilt_rust_library` rules, which don't have those attrs.
        for name, dep, flags in [(None, dep, []) for dep in ctx.attrs.deps + cxx_by_platform(ctx, ctx.attrs.platform_deps)] +
                                [(name, dep, []) for name, dep in getattr(ctx.attrs, "named_deps", {}).items()] +
                                [(None, dep, flags) for dep, flags in getattr(ctx.attrs, "flagged_deps", []) +
                                                                      cxx_by_platform(ctx, getattr(ctx.attrs, "platform_flagged_deps", []))]
    ]

# Returns native link dependencies.
def _non_rust_link_deps(ctx: "context") -> ["dependency"]:
    """
    Return all first-order native linkable dependencies of all transitive Rust
    libraries.

    This emulates v1's graph walk, where it traverses through Rust libraries
    looking for non-Rust native link infos (and terminating the search there).
    """
    first_order_deps = [dep.dep for dep in resolve_deps(ctx)]
    return [
        d
        for d in first_order_deps
        if RustLinkInfo not in d and MergedLinkInfo in d
    ]

# Returns native link dependencies.
def _non_rust_link_infos(ctx: "context") -> ["MergedLinkInfo"]:
    """
    Return all first-order native link infos of all transitive Rust libraries.

    This emulates v1's graph walk, where it traverses through Rust libraries
    looking for non-Rust native link infos (and terminating the search there).
    MergedLinkInfo is a mapping from link style to all the transitive deps
    rolled up in a tset.
    """
    return [d[MergedLinkInfo] for d in _non_rust_link_deps(ctx)]

# Returns native link dependencies.
def _non_rust_shared_lib_infos(ctx: "context") -> ["SharedLibraryInfo"]:
    """
    Return all transitive shared libraries for non-Rust native linkabes.

    This emulates v1's graph walk, where it traverses through -- and ignores --
    Rust libraries to collect all transitive shared libraries.
    """
    first_order_deps = [dep.dep for dep in resolve_deps(ctx)]
    return [
        d[SharedLibraryInfo]
        for d in first_order_deps
        if RustLinkInfo not in d and SharedLibraryInfo in d
    ]

# Returns native link dependencies.
def _rust_link_infos(ctx: "context") -> ["RustLinkInfo"]:
    first_order_deps = resolve_deps(ctx)
    return filter(None, [d.dep.get(RustLinkInfo) for d in first_order_deps])

def normalize_crate(label: str.type) -> str.type:
    return label.replace("-", "_")

def inherited_non_rust_exported_link_deps(ctx: "context") -> ["dependency"]:
    deps = {}
    for dep in _non_rust_link_deps(ctx):
        deps[dep.label] = dep
    for info in _rust_link_infos(ctx):
        for dep in info.non_rust_exported_link_deps:
            deps[dep.label] = dep
    return deps.values()

def inherited_non_rust_link_info(ctx: "context") -> "MergedLinkInfo":
    infos = []
    infos.extend(_non_rust_link_infos(ctx))
    infos.extend([d.non_rust_link_info for d in _rust_link_infos(ctx)])
    return merge_link_infos(ctx, infos)

def inherited_non_rust_shared_libs(ctx: "context") -> ["SharedLibraryInfo"]:
    infos = []
    infos.extend(_non_rust_shared_lib_infos(ctx))
    infos.extend([d.non_rust_shared_libs for d in _rust_link_infos(ctx)])
    return infos

def attr_crate(ctx: "context") -> str.type:
    return ctx.attrs.crate or normalize_crate(ctx.label.name)
