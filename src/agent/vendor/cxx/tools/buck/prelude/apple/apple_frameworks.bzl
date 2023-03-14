# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

load("@prelude//:paths.bzl", "paths")
load(
    "@prelude//linking:link_info.bzl",
    "FrameworksLinkable",
    "LinkArgs",
    "LinkInfo",
    "LinkInfos",
    "LinkInfosTSet",
    "LinkableType",
    "get_link_args",
    "merge_framework_linkables",
)
load("@prelude//utils:utils.bzl", "expect")
load(":apple_framework_versions.bzl", "get_framework_linker_args")
load(":apple_toolchain_types.bzl", "AppleToolchainInfo")

_IMPLICIT_SDKROOT_FRAMEWORK_SEARCH_PATHS = [
    "$SDKROOT/Library/Frameworks",
    "$SDKROOT/System/Library/Frameworks",
]

def create_frameworks_linkable(ctx: "context") -> [FrameworksLinkable.type, None]:
    if not ctx.attrs.libraries and not ctx.attrs.frameworks:
        return None

    return FrameworksLinkable(
        library_names = [_library_name(x) for x in ctx.attrs.libraries],
        unresolved_framework_paths = _get_non_sdk_unresolved_framework_directories(ctx.attrs.frameworks),
        framework_names = [to_framework_name(x) for x in ctx.attrs.frameworks],
    )

def _get_apple_frameworks_linker_flags(ctx: "context", linkable: [FrameworksLinkable.type, None]) -> "cmd_args":
    if not linkable:
        return cmd_args()

    expanded_frameworks_paths = _expand_sdk_framework_paths(ctx, linkable.unresolved_framework_paths)
    flags = _get_framework_search_path_flags(expanded_frameworks_paths)
    flags.add(get_framework_linker_args(ctx, linkable.framework_names))

    for library_name in linkable.library_names:
        flags.add("-l" + library_name)

    return flags

def get_framework_search_path_flags(ctx: "context") -> "cmd_args":
    unresolved_framework_dirs = _get_non_sdk_unresolved_framework_directories(ctx.attrs.frameworks)
    expanded_framework_dirs = _expand_sdk_framework_paths(ctx, unresolved_framework_dirs)
    return _get_framework_search_path_flags(expanded_framework_dirs)

def _get_framework_search_path_flags(frameworks: ["cmd_args"]) -> "cmd_args":
    flags = cmd_args()
    for directory in frameworks:
        flags.add(["-F", directory])

    return flags

def _get_non_sdk_unresolved_framework_directories(frameworks: [""]) -> [""]:
    # We don't want to include SDK directories as those are already added via `isysroot` flag in toolchain definition.
    # Adding those directly via `-F` will break building Catalyst applications as frameworks from support directory
    # won't be found and those for macOS platform will be used.
    return dedupe(filter(None, [_non_sdk_unresolved_framework_directory(x) for x in frameworks]))

def to_framework_name(framework_path: str.type) -> str.type:
    name, ext = paths.split_extension(paths.basename(framework_path))
    expect(ext == ".framework", "framework `{}` missing `.framework` suffix", framework_path)
    return name

def _library_name(library: str.type) -> str.type:
    name = paths.basename(library)
    if not name.startswith("lib"):
        fail("unexpected library: {}".format(library))
    return paths.split_extension(name[3:])[0]

def _expand_sdk_framework_paths(ctx: "context", unresolved_framework_paths: [str.type]) -> ["cmd_args"]:
    return [_expand_sdk_framework_path(ctx, unresolved_framework_path) for unresolved_framework_path in unresolved_framework_paths]

def _expand_sdk_framework_path(ctx: "context", framework_path: str.type) -> "cmd_args":
    apple_toolchain_info = ctx.attrs._apple_toolchain[AppleToolchainInfo]
    path_expansion_map = {
        "$PLATFORM_DIR/": apple_toolchain_info.platform_path,
        "$SDKROOT/": apple_toolchain_info.sdk_path,
    }

    for (trailing_path_variable, path_value) in path_expansion_map.items():
        (before, separator, relative_path) = framework_path.partition(trailing_path_variable)
        if separator == trailing_path_variable:
            if len(before) > 0:
                fail("Framework symbolic path not anchored at the beginning, tried expanding `{}`".format(framework_path))
            if relative_path.count("$") > 0:
                fail("Framework path contains multiple symbolic paths, tried expanding `{}`".format(framework_path))
            if len(relative_path) == 0:
                fail("Framework symbolic path contains no relative path to expand, tried expanding `{}`, relative path: `{}`, before: `{}`, separator `{}`".format(framework_path, relative_path, before, separator))

            return cmd_args([path_value, relative_path], delimiter = "/")

    if framework_path.find("$") == 0:
        fail("Failed to expand framework path: {}".format(framework_path))

    return cmd_args(framework_path)

def _non_sdk_unresolved_framework_directory(framework_path: str.type) -> [str.type, None]:
    # We must only drop any framework paths that are part of the implicit
    # framework search paths in the linker + compiler, all other paths
    # must be expanded and included as part of the command.
    for implicit_search_path in _IMPLICIT_SDKROOT_FRAMEWORK_SEARCH_PATHS:
        if framework_path.find(implicit_search_path) == 0:
            return None
    return paths.dirname(framework_path)

def build_link_args_with_deduped_framework_flags(
        ctx: "context",
        info: "MergedLinkInfo",
        frameworks_linkable: ["FrameworksLinkable", None],
        link_style: "LinkStyle",
        prefer_stripped: bool.type = False) -> LinkArgs.type:
    frameworks_link_info = _link_info_from_frameworks_linkable(ctx, [info.frameworks[link_style], frameworks_linkable])
    if not frameworks_link_info:
        return get_link_args(info, link_style, prefer_stripped)

    return LinkArgs(
        tset = (ctx.actions.tset(
            LinkInfosTSet,
            value = LinkInfos(default = frameworks_link_info, stripped = frameworks_link_info),
            children = [info._infos[link_style]],
        ), prefer_stripped),
    )

def get_frameworks_link_info_by_deduping_link_infos(
        ctx: "context",
        infos: [[LinkInfo.type, None]],
        framework_linkable: [FrameworksLinkable.type, None]) -> [LinkInfo.type, None]:
    # When building a framework or executable, all frameworks used by the statically-linked
    # deps in the subtree need to be linked.
    #
    # Without deduping, we've seen the linking step fail because the argsfile
    # exceeds the acceptable size by the linker.
    framework_linkables = _extract_framework_linkables(infos)
    if framework_linkable:
        framework_linkables.append(framework_linkable)

    return _link_info_from_frameworks_linkable(ctx, framework_linkables)

def _extract_framework_linkables(link_infos: [[LinkInfo.type], None]) -> [FrameworksLinkable.type]:
    frameworks_type = LinkableType("frameworks")

    linkables = []
    for info in link_infos:
        for linkable in info.linkables:
            if linkable._type == frameworks_type:
                linkables.append(linkable)

    return linkables

def _link_info_from_frameworks_linkable(ctx: "context", framework_linkables: [[FrameworksLinkable.type, None]]) -> [LinkInfo.type, None]:
    framework_link_args = _get_apple_frameworks_linker_flags(ctx, merge_framework_linkables(framework_linkables))
    return LinkInfo(
        pre_flags = [framework_link_args],
    ) if framework_link_args else None
