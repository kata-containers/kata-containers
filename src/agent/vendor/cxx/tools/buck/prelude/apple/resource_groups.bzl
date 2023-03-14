# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

load(
    "@prelude//cxx:groups.bzl",
    "MATCH_ALL_LABEL",
)
load(
    "@prelude//utils:graph_utils.bzl",
    "breadth_first_traversal_by",
)
load(":apple_asset_catalog_types.bzl", "AppleAssetCatalogSpec")
load(":apple_core_data_types.bzl", "AppleCoreDataSpec")
load(":apple_resource_types.bzl", "AppleResourceSpec")

ResourceGroupInfo = provider(fields = [
    "groups",  # [Group.type]
    "groups_hash",  # str.type
    "mappings",  # {"label": str.type}
])

ResourceGraphNode = record(
    label = field("label"),
    # Attribute labels on the target.
    labels = field([str.type], []),
    # Deps of this target which might have resources transitively.
    deps = field(["label"], []),
    # Exported deps of this target which might have resources transitively.
    exported_deps = field(["label"], []),
    # Actual resource data, present when node corresponds to `apple_resource` target.
    resource_spec = field([AppleResourceSpec.type, None], None),
    # Actual asset catalog data, present when node corresponds to `apple_asset_catalog` target.
    asset_catalog_spec = field([AppleAssetCatalogSpec.type, None], None),
    # Actual core data, present when node corresponds to `core_data_model` target
    core_data_spec = field([AppleCoreDataSpec.type, None], None),
)

ResourceGraphTSet = transitive_set()

ResourceGraph = provider(fields = [
    "label",  # "label"
    "nodes",  # "ResourceGraphTSet"
])

def create_resource_graph(
        ctx: "context",
        labels: [str.type],
        deps: ["dependency"],
        exported_deps: ["dependency"],
        resource_spec: [AppleResourceSpec.type, None] = None,
        asset_catalog_spec: [AppleAssetCatalogSpec.type, None] = None,
        core_data_spec: [AppleCoreDataSpec.type, None] = None) -> ResourceGraph.type:
    node = ResourceGraphNode(
        label = ctx.label,
        labels = labels,
        deps = _with_resources_deps(deps),
        exported_deps = _with_resources_deps(exported_deps),
        resource_spec = resource_spec,
        asset_catalog_spec = asset_catalog_spec,
        core_data_spec = core_data_spec,
    )
    all_deps = deps + exported_deps
    child_nodes = filter(None, [d.get(ResourceGraph) for d in all_deps])
    return ResourceGraph(
        label = ctx.label,
        nodes = ctx.actions.tset(ResourceGraphTSet, value = node, children = [child_node.nodes for child_node in child_nodes]),
    )

def get_resource_graph_node_map_func(graph: ResourceGraph.type):
    def get_resource_graph_node_map() -> {"label": ResourceGraphNode.type}:
        nodes = graph.nodes.traverse()
        return {node.label: node for node in filter(None, nodes)}

    return get_resource_graph_node_map

def _with_resources_deps(deps: ["dependency"]) -> ["label"]:
    """
    Filters dependencies and returns only those which are relevant
    to working with resources i.e. those which contains resource graph provider.
    """
    graphs = filter(None, [d.get(ResourceGraph) for d in deps])
    return [g.label for g in graphs]

def get_resource_group_info(ctx: "context") -> [ResourceGroupInfo.type, None]:
    """
    Parses the currently analyzed context for any resource group definitions
    and returns a list of all resource groups with their mappings.
    """
    resource_group_map = ctx.attrs.resource_group_map

    if not resource_group_map:
        return None

    if type(resource_group_map) == "dependency":
        return resource_group_map[ResourceGroupInfo]

    fail("Resource group maps must be provided as a resource_group_map rule dependency.")

def get_filtered_resources(
        root: "label",
        resource_graph_node_map_func,
        resource_group: [str.type, None],
        resource_group_mappings: [{"label": str.type}, None]) -> ([AppleResourceSpec.type], [AppleAssetCatalogSpec.type], [AppleCoreDataSpec.type]):
    """
    Walks the provided DAG and collects resources matching resource groups definition.
    """

    resource_graph_node_map = resource_graph_node_map_func()

    def get_traversed_deps(target: "label") -> ["label"]:
        node = resource_graph_node_map[target]  # buildifier: disable=uninitialized
        return node.exported_deps + node.deps

    targets = breadth_first_traversal_by(
        resource_graph_node_map,
        get_traversed_deps(root),
        get_traversed_deps,
    )

    resource_specs = []
    asset_catalog_specs = []
    core_data_specs = []

    for target in targets:
        target_resource_group = resource_group_mappings.get(target)

        # Ungrouped targets belong to the unlabeled bundle
        if ((not target_resource_group and not resource_group) or
            # Does it match special "MATCH_ALL" mapping?
            target_resource_group == MATCH_ALL_LABEL or
            # Does it match currently evaluated group?
            target_resource_group == resource_group):
            node = resource_graph_node_map[target]
            resource_spec = node.resource_spec
            if resource_spec:
                resource_specs.append(resource_spec)
            asset_catalog_spec = node.asset_catalog_spec
            if asset_catalog_spec:
                asset_catalog_specs.append(asset_catalog_spec)
            core_data_spec = node.core_data_spec
            if core_data_spec:
                core_data_specs.append(core_data_spec)

    return resource_specs, asset_catalog_specs, core_data_specs
