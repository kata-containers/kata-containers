# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

load("@prelude//:paths.bzl", "paths")
load(
    "@prelude//linking:link_info.bzl",
    "LinkInfo",
    "LinkInfos",
    "LinkStyle",
    "ObjectsLinkable",
)
load(
    "@prelude//linking:linkables.bzl",
    "LinkableProviders",  # @unused Used as type
)
load("@prelude//linking:shared_libraries.bzl", "SharedLibrariesTSet")
load("@prelude//linking:strip.bzl", "strip_debug_info")

LinkableProvidersTSet = transitive_set()

# Info required to link cxx_python_extensions into native python binaries
CxxExtensionLinkInfo = provider(
    fields = [
        "linkable_providers",  # LinkableProvidersTSet.type
        "shared_libraries",  # SharedLibrariesTSet.type
        "artifacts",  # {str.type: "_a"}
        "python_module_names",  # {str.type: str.type}
    ],
)

def merge_cxx_extension_info(
        actions: "actions",
        deps: ["dependency"],
        linkable_providers: [LinkableProviders.type, None] = None,
        shared_libraries: [SharedLibrariesTSet.type] = [],
        artifacts: {str.type: "_a"} = {},
        python_module_names: {str.type: str.type} = {}) -> CxxExtensionLinkInfo.type:
    linkable_provider_children = []
    shared_libraries = list(shared_libraries)
    artifacts = dict(artifacts)
    python_module_names = dict(python_module_names)
    for dep in deps:
        cxx_extension_info = dep.get(CxxExtensionLinkInfo)
        if cxx_extension_info == None:
            continue
        linkable_provider_children.append(cxx_extension_info.linkable_providers)
        shared_libraries.append(cxx_extension_info.shared_libraries)
        artifacts.update(cxx_extension_info.artifacts)
        python_module_names.update(cxx_extension_info.python_module_names)
    linkable_providers_kwargs = {}
    if linkable_providers != None:
        linkable_providers_kwargs["value"] = linkable_providers
    linkable_providers_kwargs["children"] = linkable_provider_children
    return CxxExtensionLinkInfo(
        linkable_providers = actions.tset(LinkableProvidersTSet, **linkable_providers_kwargs),
        shared_libraries = actions.tset(SharedLibrariesTSet, children = shared_libraries),
        artifacts = artifacts,
        python_module_names = python_module_names,
    )

def rewrite_static_symbols(
        ctx: "context",
        suffix: str.type,
        pic_objects: ["artifact"],
        non_pic_objects: ["artifact"],
        libraries: {LinkStyle.type: LinkInfos.type},
        cxx_toolchain: "CxxToolchainInfo") -> {LinkStyle.type: LinkInfos.type}:
    symbols_file = write_syms_file(ctx, pic_objects + non_pic_objects, suffix, cxx_toolchain)
    static_objects, stripped_static_objects = suffix_symbols(ctx, suffix, non_pic_objects, symbols_file, cxx_toolchain)
    static_pic_objects, stripped_static_pic_objects = suffix_symbols(ctx, suffix, pic_objects, symbols_file, cxx_toolchain)

    static_info = libraries[LinkStyle("static")].default
    updated_static_info = LinkInfo(
        name = static_info.name,
        pre_flags = static_info.pre_flags,
        post_flags = static_info.post_flags,
        linkables = [static_objects],
        use_link_groups = static_info.use_link_groups,
    )
    updated_stripped_static_info = LinkInfo(
        name = static_info.name,
        pre_flags = static_info.pre_flags,
        post_flags = static_info.post_flags,
        linkables = [stripped_static_objects],
        use_link_groups = static_info.use_link_groups,
    )

    static_pic_info = libraries[LinkStyle("static")].default
    updated_static_pic_info = LinkInfo(
        name = static_pic_info.name,
        pre_flags = static_pic_info.pre_flags,
        post_flags = static_pic_info.post_flags,
        linkables = [static_pic_objects],
        use_link_groups = static_pic_info.use_link_groups,
    )
    updated_stripped_static_pic_info = LinkInfo(
        name = static_pic_info.name,
        pre_flags = static_pic_info.pre_flags,
        post_flags = static_pic_info.post_flags,
        linkables = [stripped_static_pic_objects],
        use_link_groups = static_pic_info.use_link_groups,
    )
    updated_libraries = {
        LinkStyle("static"): LinkInfos(default = updated_static_info, stripped = updated_stripped_static_info),
        LinkStyle("static_pic"): LinkInfos(default = updated_static_pic_info, stripped = updated_stripped_static_pic_info),
    }
    return updated_libraries

def write_syms_file(
        ctx: "context",
        objects: ["artifact"],
        suffix: str.type,
        cxx_toolchain: "CxxToolchainInfo") -> "artifact":
    """
    Take a list of objects and append a suffix to all  defined symbols.
    """
    nm = cxx_toolchain.binary_utilities_info.nm
    symbols_file = ctx.actions.declare_output(ctx.label.name + "_renamed_syms")
    objects_args = cmd_args()
    for obj in objects:
        objects_args.add(cmd_args(obj, format = "{}"))

    script_env = {
        "NM": nm,
        "OBJECTS": objects_args,
        "SYMSFILE": symbols_file.as_output(),
    }

    # Compile symbols defined by all object files into a de-duplicated list of symbols to rename
    # --no-sort tells nm not to sort the output because we are sorting it to dedupe anyway
    # --defined-only prints only the symbols defined by this extension this way we won't rename symbols defined externally e.g. PyList_GetItem, etc...
    # -j print only the symbol name
    # sort -u sorts the combined list of symbols and removes any duplicate entries
    # using awk we format the symbol names 'PyInit_hello' followed by the symbol name with the suffix appended to create the input file for objcopy
    # objcopy uses a list of symbol name followed by updated name e.g. 'PyInit_hello PyInit_hello_package_module'
    script = (
        "set -euo pipefail; " +  # fail if any command in the script fails
        '"$NM" --no-sort --defined-only -j $OBJECTS | sort -u |' +
        ' awk \'{{print $1" "$1"_{suffix}"}}\' > '.format(suffix = suffix) +
        '"$SYMSFILE";'
    )
    ctx.actions.run(
        [
            "/bin/bash",
            "-c",
            script,
        ],
        env = script_env,
        category = "write_syms_file",
        identifier = "{}_write_syms_file".format(symbols_file.basename),
    )
    return symbols_file

def suffix_symbols(
        ctx: "context",
        suffix: str.type,
        objects: ["artifact"],
        symbols_file: "artifact",
        cxx_toolchain: "CxxToolchainInfo") -> (ObjectsLinkable.type, ObjectsLinkable.type):
    """
    Take a list of objects and append a suffix to all  defined symbols.
    """
    objcopy = cxx_toolchain.binary_utilities_info.objcopy

    artifacts = []
    stripped_artifacts = []
    for obj in objects:
        base, name = paths.split_extension(obj.short_path)
        updated_name = "_".join([base, suffix, name])
        artifact = ctx.actions.declare_output(updated_name)

        script_env = {
            "OBJCOPY": objcopy,
            "ORIGINAL": obj,
            "OUT": artifact.as_output(),
            "SYMSFILE": symbols_file,
        }

        script = (
            "set -euo pipefail; " +  # fail if any command in the script fails
            '"$OBJCOPY" --redefine-syms="$SYMSFILE" "$ORIGINAL" "$OUT"'  # using objcopy we pass in the symbols file to re-write the original symbol name to the now suffixed version
        )

        # Usage: objcopy [option(s)] in-file [out-file]
        ctx.actions.run(
            [
                "/bin/bash",
                "-c",
                script,
            ],
            env = script_env,
            category = "suffix_symbols",
            identifier = updated_name,
        )

        artifacts.append(artifact)
        updated_base, _ = paths.split_extension(artifact.short_path)
        stripped_artifacts.append(strip_debug_info(ctx, updated_base + ".stripped.o", artifact))

    default = ObjectsLinkable(
        objects = artifacts,
        linker_type = cxx_toolchain.linker_info.type,
    )
    stripped = ObjectsLinkable(
        objects = stripped_artifacts,
        linker_type = cxx_toolchain.linker_info.type,
    )
    return default, stripped
