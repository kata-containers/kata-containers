# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

load("@prelude//cxx:cxx_toolchain_types.bzl", "CxxPlatformInfo")
load("@prelude//linking:shared_libraries.bzl", "traverse_shared_library_info")
load("@prelude//utils:utils.bzl", "flatten")
load(":interface.bzl", "PythonLibraryInterface", "PythonLibraryManifestsInterface")
load(":manifest.bzl", "ManifestInfo")
load(":toolchain.bzl", "PythonPlatformInfo", "get_platform_attr")

PythonLibraryManifests = record(
    label = field("label"),
    srcs = field([ManifestInfo.type, None]),
    src_types = field([ManifestInfo.type, None], None),
    resources = field([(ManifestInfo.type, ["_arglike"]), None]),
    bytecode = field([ManifestInfo.type, None]),
    # A map of module name to to source artifact for Python extensions.
    extensions = field([{str.type: "_a"}, None]),
)

def _bytecode_artifacts(value: PythonLibraryManifests.type):
    if value.bytecode == None:
        return []
    return value.bytecode.artifacts

def _bytecode_manifests(value: PythonLibraryManifests.type):
    if value.bytecode == None:
        return []
    return value.bytecode.manifest

def _hidden_resources(value: PythonLibraryManifests.type):
    if value.resources == None:
        return []
    return value.resources[1]

def _has_hidden_resources(children: [bool.type], value: [PythonLibraryManifests.type, None]):
    if value:
        if value.resources and len(value.resources[1]) > 0:
            return True
    return any(children)

def _resource_manifests(value: PythonLibraryManifests.type):
    if value.resources == None:
        return []
    return value.resources[0].manifest

def _resource_artifacts(value: PythonLibraryManifests.type):
    if value.resources == None:
        return []
    return value.resources[0].artifacts

def _source_manifests(value: PythonLibraryManifests.type):
    if value.srcs == None:
        return []
    return value.srcs.manifest

def _source_artifacts(value: PythonLibraryManifests.type):
    if value.srcs == None:
        return []
    return value.srcs.artifacts

def _source_type_manifests(value: PythonLibraryManifests.type):
    if value.src_types == None:
        return []
    return value.src_types.manifest

def _source_type_artifacts(value: PythonLibraryManifests.type):
    if value.src_types == None:
        return []
    return value.src_types.artifacts

PythonLibraryManifestsTSet = transitive_set(
    args_projections = {
        "bytecode_artifacts": _bytecode_artifacts,
        "bytecode_manifests": _bytecode_manifests,
        "hidden_resources": _hidden_resources,
        "resource_artifacts": _resource_artifacts,
        "resource_manifests": _resource_manifests,
        "source_artifacts": _source_artifacts,
        "source_manifests": _source_manifests,
        "source_type_artifacts": _source_type_artifacts,
        "source_type_manifests": _source_type_manifests,
    },
    reductions = {
        "has_hidden_resources": _has_hidden_resources,
    },
)

# Information about a python library and its dependencies.
# TODO(nmj): Resources in general, and mapping of resources to new paths too.
PythonLibraryInfo = provider(fields = [
    "manifests",  # PythonLibraryManifestsTSet
    "shared_libraries",  # "SharedLibraryInfo"
])

def info_to_interface(info: PythonLibraryInfo.type) -> PythonLibraryInterface.type:
    return PythonLibraryInterface(
        shared_libraries = lambda: traverse_shared_library_info(info.shared_libraries),
        iter_manifests = lambda: info.manifests.traverse(),
        manifests = lambda: manifests_to_interface(info.manifests),
        has_hidden_resources = lambda: info.manifests.reduce("has_hidden_resources"),
        hidden_resources = lambda: [info.manifests.project_as_args("hidden_resources")],
    )

def manifests_to_interface(manifests: PythonLibraryManifestsTSet.type) -> PythonLibraryManifestsInterface.type:
    return PythonLibraryManifestsInterface(
        src_manifests = lambda: [manifests.project_as_args("source_manifests")],
        src_artifacts = lambda: [manifests.project_as_args("source_artifacts")],
        src_type_manifests = lambda: [manifests.project_as_args("source_manifests")],
        src_type_artifacts = lambda: [manifests.project_as_args("source_artifacts")],
        bytecode_manifests = lambda: [manifests.project_as_args("bytecode_manifests")],
        bytecode_artifacts = lambda: [manifests.project_as_args("bytecode_artifacts")],
        resource_manifests = lambda: [manifests.project_as_args("resource_manifests")],
        resource_artifacts = lambda: [manifests.project_as_args("resource_artifacts")],
    )

def get_python_deps(ctx: "context"):
    python_platform = ctx.attrs._python_toolchain[PythonPlatformInfo]
    cxx_platform = ctx.attrs._cxx_toolchain[CxxPlatformInfo]
    return flatten(
        [ctx.attrs.deps] +
        get_platform_attr(python_platform, cxx_platform, ctx.attrs.platform_deps),
    )
