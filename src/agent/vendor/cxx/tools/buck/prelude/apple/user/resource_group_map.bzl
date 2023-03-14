# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

load("@prelude//:attributes.bzl", "Traversal")
load(
    "@prelude//apple:resource_groups.bzl",
    "ResourceGroupInfo",
    "create_resource_graph",
    "get_resource_graph_node_map_func",
)
load(
    "@prelude//cxx:groups.bzl",
    "compute_mappings",
    "parse_groups_definitions",
)
load("@prelude//user:rule_spec.bzl", "RuleRegistrationSpec")

def v1_attrs():
    return attrs.list(attrs.tuple(attrs.string(), attrs.list(attrs.tuple(attrs.dep(), attrs.enum(Traversal), attrs.option(attrs.string())))))

def resource_group_map_attr():
    v2_attrs = attrs.dep(providers = [ResourceGroupInfo])
    return attrs.option(attrs.one_of(v2_attrs, v1_attrs()), default = None)

def _impl(ctx: "context") -> ["provider"]:
    resource_groups = parse_groups_definitions(ctx.attrs.map)
    resource_groups_deps = [mapping.root.node for group in resource_groups for mapping in group.mappings]
    resource_graph = create_resource_graph(
        ctx = ctx,
        labels = [],
        deps = resource_groups_deps,
        exported_deps = [],
    )
    resource_graph_node_map = get_resource_graph_node_map_func(resource_graph)()
    mappings = compute_mappings(groups = resource_groups, graph_map = resource_graph_node_map)
    return [
        DefaultInfo(),
        ResourceGroupInfo(groups = resource_groups, groups_hash = hash(str(resource_groups)), mappings = mappings),
    ]

registration_spec = RuleRegistrationSpec(
    name = "resource_group_map",
    impl = _impl,
    attrs = {
        "map": v1_attrs(),
    },
)
