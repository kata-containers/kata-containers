# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

load("@prelude//:genrule.bzl", "process_genrule")
load("@prelude//android:android_providers.bzl", "AndroidApkInfo", "AndroidApkUnderTestInfo")
load("@prelude//utils:utils.bzl", "expect")

def apk_genrule_impl(ctx: "context") -> ["provider"]:
    # TODO(T104150125) The underlying APK should not have exopackage enabled
    input_android_apk_info = ctx.attrs.apk[AndroidApkInfo]
    expect(input_android_apk_info != None, "'apk' attribute must be an Android APK!")
    input_android_apk_under_test_info = ctx.attrs.apk[AndroidApkUnderTestInfo]

    env_vars = {
        "APK": cmd_args(input_android_apk_info.apk),
    }

    # Like buck1, we ignore the 'out' attribute and construct the output path ourselves.
    output_apk_name = "{}.apk".format(ctx.label.name)

    genrule_providers = process_genrule(ctx, output_apk_name, None, env_vars)

    expect(len(genrule_providers) == 1 and type(genrule_providers[0]) == DefaultInfo.type, "Expecting just a single DefaultInfo, but got {}".format(genrule_providers))
    output_apk = genrule_providers[0].default_outputs[0]

    return genrule_providers + [
        AndroidApkInfo(
            apk = output_apk,
            manifest = input_android_apk_info.manifest,
        ),
    ] + filter(None, [input_android_apk_under_test_info])
