# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

load("@prelude//:http_file.bzl", "http_file_shared")
load("@prelude//utils:utils.bzl", "expect", "value_or")

_ROOT = "https://maven.thefacebook.com/nexus/content/groups/public"

def _from_mvn_url(url: str.type) -> str.type:
    """
    Convert `mvn:` style URIs to a URL.
    """

    mvn, group, id, typ, version = url.split(":")
    expect(mvn == "mvn")

    group = group.replace(".", "/")

    if typ == "src":
        ext = "-sources.jar"
    else:
        ext = "." + typ

    return "{root}/{group}/{id}/{version}/{id}-{version}{ext}".format(
        root = _ROOT,
        group = group,
        id = id,
        version = version,
        ext = ext,
    )

# Implementation of the `remote_file` build rule.
def remote_file_impl(ctx: "context") -> ["provider"]:
    url = ctx.attrs.url
    if url.startswith("mvn:"):
        url = _from_mvn_url(url)
    return http_file_shared(
        ctx.actions,
        name = value_or(ctx.attrs.out, ctx.label.name),
        url = url,
        is_executable = ctx.attrs.type == "executable",
        is_exploded_zip = ctx.attrs.type == "exploded_zip",
        unzip_tool = ctx.attrs._unzip_tool[RunInfo],
        sha1 = ctx.attrs.sha1,
        sha256 = ctx.attrs.sha256,
    )
