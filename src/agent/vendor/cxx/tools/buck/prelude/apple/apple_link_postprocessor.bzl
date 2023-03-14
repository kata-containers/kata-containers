# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

def get_apple_link_postprocessor(ctx):
    if ctx.attrs.link_postprocessor:
        return cmd_args(ctx.attrs.link_postprocessor[RunInfo])
    return None
