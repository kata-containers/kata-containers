# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

def postprocess(ctx, input, postprocessor):
    output = ctx.actions.declare_output("postprocessed/{}".format(input.short_path))
    cmd = cmd_args()
    cmd.add(postprocessor)
    cmd.add(["--input", input])
    cmd.add(["--output", output.as_output()])
    ctx.actions.run(cmd, category = "link_postprocess", identifier = input.short_path)
    return output
