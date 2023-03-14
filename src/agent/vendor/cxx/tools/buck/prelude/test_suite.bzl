# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

def test_suite_impl(_ctx: "context") -> ["provider"]:
    # There is nothing to implement here: test_suite exists as a mechanism to "group" tests using
    # the `tests` attribute, and the `tests` attribute is supported for all rules.
    return [DefaultInfo()]
