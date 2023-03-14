# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

load("@prelude//:attributes.bzl", "Linkage", "Traversal")
load(
    "@prelude//cxx:groups.bzl",
    "compute_mappings",
)
load(
    "@prelude//cxx:link_groups.bzl",
    "LinkGroupInfo",
    "parse_link_group_definitions",
)
load(
    "@prelude//linking:link_groups.bzl",
    "LinkGroupLibInfo",
)
load(
    "@prelude//linking:link_info.bzl",
    "MergedLinkInfo",
)
load(
    "@prelude//linking:linkable_graph.bzl",
    "LinkableGraph",
    "create_linkable_graph",
    "get_linkable_graph_node_map_func",
)
load(
    "@prelude//linking:shared_libraries.bzl",
    "SharedLibraryInfo",
)
load("@prelude//user:rule_spec.bzl", "RuleRegistrationSpec")

def _v1_attrs(optional_root: bool.type = False):
    attrs_root = attrs.dep(providers = [
        LinkGroupLibInfo,
        LinkableGraph,
        MergedLinkInfo,
        SharedLibraryInfo,
    ])
    if optional_root:
        attrs_root = attrs.option(attrs_root)
    return attrs.list(
        attrs.tuple(
            attrs.string(),
            attrs.list(
                attrs.tuple(
                    attrs_root,
                    attrs.enum(Traversal),
                    attrs.option(attrs.string()),
                    attrs.option(attrs.enum(Linkage)),
                ),
            ),
        ),
    )

def link_group_map_attr():
    v2_attrs = attrs.dep(providers = [LinkGroupInfo])
    return attrs.option(attrs.one_of(v2_attrs, _v1_attrs(optional_root = True)), default = None)

def _impl(ctx: "context") -> ["provider"]:
    link_groups = parse_link_group_definitions(ctx.attrs.map)
    linkable_graph = create_linkable_graph(
        ctx,
        children = [
            mapping.root.node.linkable_graph
            for group in link_groups
            for mapping in group.mappings
        ],
    )
    linkable_graph_node_map = get_linkable_graph_node_map_func(linkable_graph)()
    mappings = compute_mappings(groups = link_groups, graph_map = linkable_graph_node_map)
    return [
        DefaultInfo(),
        LinkGroupInfo(groups = link_groups, groups_hash = hash(str(link_groups)), mappings = mappings),
    ]

registration_spec = RuleRegistrationSpec(
    name = "link_group_map",
    impl = _impl,
    attrs = {
        "map": _v1_attrs(),
    },
)
