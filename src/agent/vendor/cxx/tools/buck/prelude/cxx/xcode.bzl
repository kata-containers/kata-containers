# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

load(
    "@prelude//cxx:compile.bzl",
    "CxxSrcWithFlags",  # @unused Used as a type
)

def cxx_populate_xcode_attributes(
        ctx,
        srcs: [CxxSrcWithFlags.type],
        argsfiles_by_ext: {str.type: "artifact"},
        product_name: str.type) -> {str.type: ""}:
    converted_srcs = {}
    for src in srcs:
        file_properties = _get_artifact_owner(src.file)
        if src.flags:
            # List of resolved_macros will encode as:
            # [['\"-some-flag\"'], ['\"-another-flag\"']]
            #
            # Convert it to a string and rip-out the quotes
            # so it appears as ["-some-flag", "-another-flag"]
            file_properties["flags"] = [str(flag).replace('\"', "") for flag in src.flags]
        converted_srcs[src.file] = file_properties

    data = {
        "argsfiles_by_ext": {
            # Enum types cannot be encoded by our JSON API.
            # Use the str representation.
            repr(ext).replace('\"', ""): artifact
            for ext, artifact in argsfiles_by_ext.items()
        },
        "headers": _get_artifacts_with_owners(ctx.attrs.headers),
        "product_name": product_name,
        "srcs": converted_srcs,
    }

    if hasattr(ctx.attrs, "exported_headers"):
        data["exported_headers"] = _get_artifacts_with_owners(ctx.attrs.exported_headers)

    return data

def _get_artifacts_with_owners(files: "") -> {"artifact": {str.type: "label"}}:
    if type(files) == "dict":
        return {artifact: _get_artifact_owner(artifact) for _, artifact in files.items()}
    else:
        return {file: _get_artifact_owner(file) for file in files}

def _get_artifact_owner(file: "artifact") -> {str.type: "label"}:
    if file.owner:
        return {"target": file.owner}
    else:
        return {}
