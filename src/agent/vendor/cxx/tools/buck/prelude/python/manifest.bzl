# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

# Manifests are files containing information how to map sources into a package.
# The files are JSON lists with an entry per source, where each source is 3-tuple
# of relative destination path, artifact path, and a short description of the
# origin of this source (used for error messages in tooling that uses these).
ManifestInfo = record(
    # The actual manifest file (in the form of a JSON file).
    manifest = field("artifact"),
    # All artifacts that are referenced in the manifest.
    artifacts = field(["artifact"]),
)

def _write_manifest(
        ctx: "context",
        name: str.type,
        entries: [(str.type, "artifact", str.type)]) -> "artifact":
    """
    Serialize the given source manifest entries to a JSON file.
    """
    return ctx.actions.write_json(name + ".manifest", entries)

def create_manifest_for_entries(
        ctx: "context",
        name: str.type,
        entries: [(str.type, "artifact", str.type)]) -> ManifestInfo.type:
    """
    Generate a source manifest for the given list of sources.
    """
    return ManifestInfo(
        manifest = _write_manifest(ctx, name, entries),
        artifacts = [a for _, a, _ in entries],
    )

def create_manifest_for_source_map(
        ctx: "context",
        param: str.type,
        srcs: {str.type: "artifact"}) -> ManifestInfo.type:
    """
    Generate a source manifest for the given map of sources from the given rule.
    """
    origin = "{} {}".format(ctx.label.raw_target(), param)
    return create_manifest_for_entries(
        ctx,
        param,
        [(dest, artifact, origin) for dest, artifact in srcs.items()],
    )

def create_manifest_for_source_dir(
        ctx: "context",
        param: str.type,
        extracted: "artifact") -> ManifestInfo.type:
    """
    Generate a source manifest for the given directory of sources from the given
    rule.
    """
    manifest = ctx.actions.declare_output(param + ".manifest")
    cmd = cmd_args(ctx.attrs._create_manifest_for_source_dir[RunInfo])
    cmd.add("--origin={}".format(ctx.label.raw_target()))
    cmd.add(cmd_args(manifest.as_output(), format = "--output={}"))
    cmd.add(extracted)
    ctx.actions.run(cmd, category = "py_source_manifest", identifier = param)
    return ManifestInfo(manifest = manifest, artifacts = [extracted])

def create_manifest_for_extensions(
        ctx: "context",
        extensions: {str.type: ("_a", "label")},
        # Whether to include DWP files.
        dwp: bool.type = False) -> ManifestInfo.type:
    entries = []
    for dest, (lib, label) in extensions.items():
        entries.append((dest, lib.output, str(label.raw_target())))
        if dwp and lib.dwp != None:
            entries.append((dest + ".dwp", lib.dwp, str(label.raw_target()) + ".dwp"))
    manifest = create_manifest_for_entries(ctx, "extensions", entries)

    # Include external debug paths, even though they're not explicitly listed
    # in the manifest, as python packaging may also consume debug paths which
    # were referenced in native code.
    for (lib, _) in extensions.values():
        manifest.artifacts.extend(lib.external_debug_info)

    return manifest
