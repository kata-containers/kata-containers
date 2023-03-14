# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

def get_re_executor_from_props(re_props: [{str.type: str.type}, None]) -> ["command_executor_config_builder", None]:
    """
    Convert the `remote_execution` properties param into a `CommandExecutorConfig`
    to use with test providers.
    """

    if re_props == None:
        return None

    return CommandExecutorConfig(
        local_enabled = False,
        remote_enabled = True,
        remote_execution_properties = re_props,
        remote_execution_use_case = "tpx-default",
    )
