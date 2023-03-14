# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

load(
    ":manifest.bzl",
    "ManifestInfo",  # @unused Used as a type
)
load(":python.bzl", "PythonLibraryManifestsTSet")
load(":toolchain.bzl", "PythonToolchainInfo")

def create_source_db(
        ctx: "context",
        srcs: [ManifestInfo.type, None],
        python_deps: ["PythonLibraryInfo"]) -> DefaultInfo.type:
    output = ctx.actions.declare_output("db.json")
    artifacts = []

    python_toolchain = ctx.attrs._python_toolchain[PythonToolchainInfo]
    cmd = cmd_args(python_toolchain.make_source_db)
    cmd.add(cmd_args(output.as_output(), format = "--output={}"))

    # Pass manifests for rule's sources.
    if srcs != None:
        cmd.add(cmd_args(srcs.manifest, format = "--sources={}"))
        artifacts.extend(srcs.artifacts)

    # Pass manifests for transitive deps.
    dep_manifests = ctx.actions.tset(PythonLibraryManifestsTSet, children = [d.manifests for d in python_deps])

    dependencies = cmd_args(dep_manifests.project_as_args("source_type_manifests"), format = "--dependency={}")
    dependencies_file = ctx.actions.write("source_db_dependencies", dependencies)
    dependencies_file = cmd_args(dependencies_file, format = "@{}").hidden(dependencies)

    cmd.add(dependencies_file)
    artifacts.append(dep_manifests.project_as_args("source_type_artifacts"))

    ctx.actions.run(cmd, category = "py_source_db")

    return DefaultInfo(default_outputs = [output], other_outputs = artifacts)

def create_source_db_no_deps(
        ctx: "context",
        srcs: [{str.type: "artifact"}, None]) -> DefaultInfo.type:
    content = {} if srcs == None else srcs
    output = ctx.actions.write_json("db_no_deps.json", content)
    return DefaultInfo(default_outputs = [output], other_outputs = content.values())

def create_source_db_no_deps_from_manifest(
        ctx: "context",
        srcs: ManifestInfo.type) -> DefaultInfo.type:
    output = ctx.actions.declare_output("db_no_deps.json")
    cmd = cmd_args(ctx.attrs._python_toolchain[PythonToolchainInfo].make_source_db_no_deps)
    cmd.add(cmd_args(output.as_output(), format = "--output={}"))
    cmd.add(srcs.manifest)
    ctx.actions.run(cmd, category = "py_source_db")
    return DefaultInfo(default_outputs = [output], other_outputs = srcs.artifacts)
