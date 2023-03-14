# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

# Provider flagging that result of the rule contains Apple bundle.
# It might be copied into main bundle to appropriate place if rule
# with this provider is a dependency of `apple_bundle`.
AppleBundleInfo = provider(fields = [
    # Result bundle; `artifact`
    "bundle",
    # The name of the executable within the bundle.
    # `str.type`
    "binary_name",
    # If the bundle was built for watchOS Apple platform, this affects packaging.
    # Might be omitted for certain types of bundle (e.g. frameworks) when packaging doesn't depend on it.
    # [None, `bool.type`]
    "is_watchos",
])

# Provider which helps to propagate minimum deployment version up the target graph.
AppleMinDeploymentVersionInfo = provider(fields = [
    # `str.type`
    "version",
])

AppleBundleResourceInfo = provider(fields = [
    "resource_output",  # AppleBundleResourcePartListOutput.type
])
