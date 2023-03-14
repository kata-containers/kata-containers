# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

load("@prelude//cxx:cxx_toolchain_types.bzl", "CxxToolchainInfo")
load(
    "@prelude//linking:link_info.bzl",
    "LinkedObject",  # @unused Used as a type
)
load("@prelude//linking:strip.bzl", "strip_shared_library")
load(
    "@prelude//utils:types.bzl",
    "unchecked",  # @unused Used as a type
)

SharedLibrary = record(
    lib = field(LinkedObject.type),
    stripped_lib = field(["artifact", None]),
    can_be_asset = field(bool.type),
    for_primary_apk = field(bool.type),
    label = field("label"),
)

SharedLibraries = record(
    # A mapping of shared library SONAME (e.g. `libfoo.so.2`) to the artifact.
    # Since the SONAME is what the dynamic loader uses to uniquely identify
    # libraries, using this as the key allows easily detecting conflicts from
    # dependencies.
    libraries = field({str.type: SharedLibrary.type}),
)

# T-set of SharedLibraries
SharedLibrariesTSet = transitive_set()

# Shared libraries required by top-level packaging rules (e.g. shared libs
# for Python binary, symlink trees of shared libs for C++ binaries)
SharedLibraryInfo = provider(fields = [
    "set",  # [SharedLibrariesTSet.type, None]
])

def create_shared_libraries(
        ctx: "context",
        libraries: {str.type: LinkedObject.type}) -> SharedLibraries.type:
    """
    Take a mapping of dest -> src and turn it into a mapping that will be
    passed around in providers. Used for both srcs, and resources.
    """
    cxx_toolchain = getattr(ctx.attrs, "_cxx_toolchain", None)
    return SharedLibraries(
        libraries = {name: SharedLibrary(
            lib = shlib,
            stripped_lib = strip_shared_library(
                ctx,
                cxx_toolchain[CxxToolchainInfo],
                shlib.output,
                cmd_args(["--strip-unneeded"]),
            ) if cxx_toolchain != None else None,
            can_be_asset = getattr(ctx.attrs, "can_be_asset", False) or False,
            for_primary_apk = getattr(ctx.attrs, "used_by_wrap_script", False),
            label = ctx.label,
        ) for (name, shlib) in libraries.items()},
    )

# We do a lot of merging library maps, so don't use O(n) type annotations
def _merge_lib_map(
        dest_mapping: unchecked({str.type: SharedLibrary.type}),
        mapping_to_merge: unchecked({str.type: SharedLibrary.type})) -> None:
    """
    Merges a mapping_to_merge into `dest_mapping`. Fails if different libraries
    map to the same name.
    """
    for (name, src) in mapping_to_merge.items():
        existing = dest_mapping.get(name)
        if existing != None and existing.lib != src.lib:
            error = (
                "Duplicate library {}! Conflicting mappings:\n" +
                "{} from {}\n" +
                "{} from {}"
            )
            fail(
                error.format(
                    name,
                    existing.lib,
                    existing.label,
                    src.lib,
                    src.label,
                ),
            )
        dest_mapping[name] = src

# Merge multiple SharedLibraryInfo. The value in `node` represents a set of
# SharedLibraries that is provided by the target being analyzed. It's optional
# because that might not always exist, e.g. a Python library can pass through
# SharedLibraryInfo but it cannot produce any. The value in `deps` represents
# all the inherited shared libraries for this target.
def merge_shared_libraries(
        actions: "actions",
        node: ["SharedLibraries", None] = None,
        deps: ["SharedLibraryInfo"] = []) -> "SharedLibraryInfo":
    kwargs = {}

    children = filter(None, [dep.set for dep in deps])
    if children:
        kwargs["children"] = children
    if node:
        kwargs["value"] = node

    set = actions.tset(SharedLibrariesTSet, **kwargs) if kwargs else None
    return SharedLibraryInfo(set = set)

def traverse_shared_library_info(
        info: "SharedLibraryInfo"):  # -> {str.type: SharedLibrary.type}:
    libraries = {}
    if info.set:
        for libs in info.set.traverse():
            _merge_lib_map(libraries, libs.libraries)
    return libraries
