# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

load("@prelude//utils:utils.bzl", "expect", "value_or")

def http_file_shared(
        actions: "actions",
        name: str.type,
        url: str.type,
        is_executable: bool.type,
        is_exploded_zip: bool.type,
        unzip_tool: [RunInfo.type, None],
        sha1: [None, str.type],
        sha256 = [None, str.type]) -> ["provider"]:
    output = actions.declare_output(name)
    downloaded_output = actions.declare_output("exploded_zip") if is_exploded_zip else output
    actions.download_file(
        downloaded_output,
        url,
        is_executable = is_executable,
        sha1 = sha1,
        sha256 = sha256,
        is_deferrable = True,
    )

    if is_exploded_zip:
        actions.run(
            cmd_args([
                unzip_tool,
                "--src",
                downloaded_output,
                "--dst",
                output.as_output(),
            ]),
            category = "exploded_zip_unzip",
            local_only = sha1 == None,
        )

    providers = [DefaultInfo(default_outputs = [output])]
    if is_executable:
        providers.append(RunInfo(args = [output]))
    return providers

def http_file_impl(ctx: "context") -> ["provider"]:
    expect(len(ctx.attrs.urls) == 1, "multiple `urls` not supported: {}", ctx.attrs.urls)
    return http_file_shared(
        ctx.actions,
        name = value_or(ctx.attrs.out, ctx.label.name),
        url = ctx.attrs.urls[0],
        sha1 = ctx.attrs.sha1,
        sha256 = ctx.attrs.sha256,
        is_executable = ctx.attrs.executable or False,
        is_exploded_zip = False,
        unzip_tool = None,
    )
