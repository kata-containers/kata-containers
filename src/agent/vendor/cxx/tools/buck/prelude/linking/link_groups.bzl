# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

load(
    "@prelude//utils:dicts.bzl",
    "flatten_x",
)
load(
    ":link_info.bzl",
    "LinkInfos",
    "LinkedObject",
)

# Information about a linkable node which explicitly sets `link_group`.
LinkGroupLib = record(
    # The label of the owning target (if any).
    label = field(["label", None], None),
    # The shared libs to package for this link group.
    shared_libs = field({str.type: LinkedObject.type}),
    # The link info to link against this link group.
    shared_link_infos = field(LinkInfos.type),
)

# Provider propagating info about transitive link group libs.
LinkGroupLibInfo = provider(fields = [
    # A map of link group names to their shared libraries.
    "libs",  # {str.type: LinkGroupLib.type}
])

def gather_link_group_libs(
        libs: [{str.type: LinkGroupLib.type}] = [],
        children: [LinkGroupLibInfo.type] = [],
        deps: ["dependency"] = []) -> {str.type: LinkGroupLib.type}:
    """
    Return all link groups libs deps and top-level libs.
    """
    return flatten_x(
        (libs +
         [c.libs for c in children] +
         [d[LinkGroupLibInfo].libs for d in deps if LinkGroupLibInfo in d]),
        fmt = "conflicting link group roots for \"{0}\": {1} != {2}",
    )

def merge_link_group_lib_info(
        label: ["label", None] = None,
        name: [str.type, None] = None,
        shared_libs: [{str.type: LinkedObject.type}, None] = None,
        shared_link_infos: [LinkInfos.type, None] = None,
        deps: ["dependency"] = []) -> LinkGroupLibInfo.type:
    """
    Merge and return link group info libs from deps and the current rule wrapped
    in a provider.
    """
    libs = {}
    if name != None:
        libs[name] = LinkGroupLib(
            label = label,
            shared_libs = shared_libs,
            shared_link_infos = shared_link_infos,
        )
    return LinkGroupLibInfo(
        libs = gather_link_group_libs(
            libs = [libs],
            deps = deps,
        ),
    )
