# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

load("@prelude//:paths.bzl", "paths")
load("@prelude//apple:apple_toolchain_types.bzl", "AppleToolsInfo")
load(
    "@prelude//cxx:compile.bzl",
    "CxxSrcWithFlags",  # @unused Used as a type
)
load("@prelude//cxx:cxx_types.bzl", "CxxAdditionalArgsfileParams")
load("@prelude//cxx:headers.bzl", "CHeader")
load(
    "@prelude//cxx:preprocessor.bzl",
    "CPreprocessor",
    "cxx_inherited_preprocessor_infos",
    "cxx_merge_cpreprocessors",
)
load(":apple_sdk_modules_utility.bzl", "get_sdk_deps_tset", "is_sdk_modules_provided")
load(":apple_toolchain_types.bzl", "AppleToolchainInfo")
load(":apple_utility.bzl", "get_disable_pch_validation_flags", "get_module_name", "get_versioned_target_triple")
load(":modulemap.bzl", "preprocessor_info_for_modulemap")
load(":swift_module_map.bzl", "write_swift_module_map_with_swift_deps")
load(":swift_pcm_compilation.bzl", "compile_swift_pcm", "get_pcm_deps_tset")

def _add_swiftmodule_search_path(swiftmodule_path: "artifact"):
    # Value will contain a path to the artifact,
    # while we need only the folder which contains the artifact.
    return ["-I", cmd_args(swiftmodule_path).parent()]

def _hidden_projection(swiftmodule_path: "artifact"):
    return swiftmodule_path

def _linker_args_projection(swiftmodule_path: "artifact"):
    return cmd_args(swiftmodule_path, format = "-Wl,-add_ast_path,{}")

SwiftmodulePathsTSet = transitive_set(args_projections = {
    "hidden": _hidden_projection,
    "linker_args": _linker_args_projection,
    "module_search_path": _add_swiftmodule_search_path,
})

ExportedHeadersTSet = transitive_set()

SwiftDependencyInfo = provider(fields = [
    "exported_headers",  # ExportedHeadersTSet of {"module_name": [exported_headers]}
    "exported_swiftmodule_paths",  # SwiftmodulePathsTSet of artifact that includes only paths through exported_deps, used for compilation
    "transitive_swiftmodule_paths",  # SwiftmodulePathsTSet of artifact that includes all transitive paths, used for linking
])

SwiftCompilationOutput = record(
    # The object files output from compilation.
    object_files = field(["artifact"]),
    # The swiftmodule file output from compilation.
    swiftmodule = field("artifact"),
    # The dependency info provider that provides the swiftmodule
    # search paths required for compilation.
    providers = field([["SwiftPCMCompilationInfo", "SwiftDependencyInfo"]]),
    # Preprocessor info required for ObjC compilation of this library.
    pre = field(CPreprocessor.type),
    # Exported preprocessor info required for ObjC compilation of rdeps.
    exported_pre = field(CPreprocessor.type),
    # Argsfile to compile an object file which is used by some subtargets.
    swift_argsfile = field("CxxAdditionalArgsfileParams"),
)

_REQUIRED_SDK_MODULES = ["Swift", "SwiftOnoneSupport", "Darwin", "_Concurrency"]

def compile_swift(
        ctx: "context",
        srcs: [CxxSrcWithFlags.type],
        exported_headers: [CHeader.type],
        objc_modulemap_pp_info: ["CPreprocessor", None],
        extra_search_paths_flags: ["_arglike"] = []) -> ["SwiftCompilationOutput", None]:
    if not srcs:
        return None

    toolchain = ctx.attrs._apple_toolchain[AppleToolchainInfo].swift_toolchain_info

    module_name = get_module_name(ctx)
    output_header = ctx.actions.declare_output(module_name + "-Swift.h")
    output_object = ctx.actions.declare_output(module_name + ".o")
    output_swiftmodule = ctx.actions.declare_output(module_name + ".swiftmodule")

    shared_flags = _get_shared_flags(
        ctx,
        module_name,
        exported_headers,
        objc_modulemap_pp_info,
        extra_search_paths_flags,
    )

    if toolchain.can_toolchain_emit_obj_c_header_textually:
        _compile_swiftmodule(ctx, toolchain, shared_flags, srcs, output_swiftmodule, output_header)
    else:
        unprocessed_header = ctx.actions.declare_output(module_name + "-SwiftUnprocessed.h")
        _compile_swiftmodule(ctx, toolchain, shared_flags, srcs, output_swiftmodule, unprocessed_header)
        _perform_swift_postprocessing(ctx, module_name, unprocessed_header, output_header)

    swift_argsfile = _compile_object(ctx, toolchain, shared_flags, srcs, output_object)

    # Swift libraries extend the ObjC modulemaps to include the -Swift.h header
    modulemap_pp_info = preprocessor_info_for_modulemap(ctx, "swift-extended", exported_headers, output_header)
    exported_swift_header = CHeader(
        artifact = output_header,
        name = output_header.basename,
        namespace = module_name,
        named = False,
    )
    exported_pp_info = CPreprocessor(
        headers = [exported_swift_header],
        modular_args = modulemap_pp_info.modular_args,
        args = modulemap_pp_info.args,
        modulemap_path = modulemap_pp_info.modulemap_path,
    )

    # We also need to include the unprefixed -Swift.h header in this libraries preprocessor info
    swift_header = CHeader(
        artifact = output_header,
        name = output_header.basename,
        namespace = "",
        named = False,
    )
    pre = CPreprocessor(headers = [swift_header])

    # Pass up the swiftmodule paths for this module and its exported_deps
    return SwiftCompilationOutput(
        object_files = [output_object],
        swiftmodule = output_swiftmodule,
        providers = [get_swift_dependency_info(ctx, exported_pp_info, output_swiftmodule)],
        pre = pre,
        exported_pre = exported_pp_info,
        swift_argsfile = swift_argsfile,
    )

# Swift headers are postprocessed to make them compatible with Objective-C
# compilation that does not use -fmodules. This is a workaround for the bad
# performance of -fmodules without Explicit Modules, once Explicit Modules is
# supported, this postprocessing should be removed.
def _perform_swift_postprocessing(
        ctx: "context",
        module_name: "string",
        unprocessed_header: "artifact",
        output_header: "artifact"):
    transitive_exported_headers = {
        module: module_exported_headers
        for exported_headers_map in _get_exported_headers_tset(ctx).traverse()
        if exported_headers_map
        for module, module_exported_headers in exported_headers_map.items()
    }
    deps_json = ctx.actions.write_json(module_name + "-Deps.json", transitive_exported_headers)
    postprocess_cmd = cmd_args(ctx.attrs._apple_tools[AppleToolsInfo].swift_objc_header_postprocess)
    postprocess_cmd.add([
        unprocessed_header,
        deps_json,
        output_header.as_output(),
    ])
    ctx.actions.run(postprocess_cmd, category = "swift_objc_header_postprocess")

# We use separate actions for swiftmodule and object file output. This
# improves build parallelism at the cost of duplicated work, but by disabling
# type checking in function bodies the swiftmodule compilation can be done much
# faster than object file output.
def _compile_swiftmodule(
        ctx: "context",
        toolchain: "SwiftToolchainInfo",
        shared_flags: "cmd_args",
        srcs: [CxxSrcWithFlags.type],
        output_swiftmodule: "artifact",
        output_header: "artifact") -> "CxxAdditionalArgsfileParams":
    argfile_cmd = cmd_args(shared_flags)
    argfile_cmd.add([
        "-Xfrontend",
        "-experimental-skip-non-inlinable-function-bodies-without-types",
        "-emit-module",
        "-emit-objc-header",
    ])
    cmd = cmd_args([
        "-emit-module-path",
        output_swiftmodule.as_output(),
        "-emit-objc-header-path",
        output_header.as_output(),
    ])
    return _compile_with_argsfile(ctx, "swiftmodule_compile", argfile_cmd, srcs, cmd, toolchain)

def _compile_object(
        ctx: "context",
        toolchain: "SwiftToolchainInfo",
        shared_flags: "cmd_args",
        srcs: [CxxSrcWithFlags.type],
        output_object: "artifact") -> "CxxAdditionalArgsfileParams":
    cmd = cmd_args([
        "-emit-object",
        "-o",
        output_object.as_output(),
    ])
    return _compile_with_argsfile(ctx, "swift_compile", shared_flags, srcs, cmd, toolchain)

def _compile_with_argsfile(
        ctx: "context",
        name: str.type,
        shared_flags: "cmd_args",
        srcs: [CxxSrcWithFlags.type],
        additional_flags: "cmd_args",
        toolchain: "SwiftToolchainInfo") -> "CxxAdditionalArgsfileParams":
    shell_quoted_args = cmd_args(shared_flags, quote = "shell")
    argfile, _ = ctx.actions.write(name + ".argsfile", shell_quoted_args, allow_args = True)

    cmd = cmd_args(toolchain.compiler)
    cmd.add(additional_flags)
    cmd.add(cmd_args(["@", argfile], delimiter = ""))

    cmd.add([s.file for s in srcs])

    # Swift compilation on RE without explicit modules is impractically expensive
    # because there's no shared module cache across different libraries.
    prefer_local = not _uses_explicit_modules(ctx)

    # Argsfile should also depend on all artifacts in it, otherwise they won't be materialised.
    cmd.hidden([shell_quoted_args])

    # If we prefer to execute locally (e.g., for perf reasons), ensure we upload to the cache,
    # so that CI builds populate caches used by developer machines.
    ctx.actions.run(cmd, category = name, prefer_local = prefer_local, allow_cache_upload = prefer_local)

    hidden_args = [shared_flags]
    return CxxAdditionalArgsfileParams(file = argfile, hidden_args = hidden_args, extension = ".swift")

def _get_shared_flags(
        ctx: "context",
        module_name: str.type,
        objc_headers: [CHeader.type],
        objc_modulemap_pp_info: ["CPreprocessor", None],
        extra_search_paths_flags: ["_arglike"] = []) -> "cmd_args":
    toolchain = ctx.attrs._apple_toolchain[AppleToolchainInfo].swift_toolchain_info
    cmd = cmd_args()
    cmd.add([
        # This allows us to use a relative path for the compiler resource directory.
        "-working-directory",
        ".",
        "-sdk",
        toolchain.sdk_path,
        "-target",
        get_versioned_target_triple(ctx),
        "-wmo",
        "-module-name",
        module_name,
        "-parse-as-library",
        # Disable Clang module breadcrumbs in the DWARF info. These will not be
        # debug prefix mapped and are not shareable across machines.
        "-Xfrontend",
        "-no-clang-module-breadcrumbs",
    ])

    if _uses_explicit_modules(ctx):
        cmd.add(get_disable_pch_validation_flags())

    if toolchain.resource_dir:
        cmd.add([
            "-resource-dir",
            toolchain.resource_dir,
        ])

    if ctx.attrs.swift_version:
        cmd.add(["-swift-version", ctx.attrs.swift_version])

    if ctx.attrs.enable_cxx_interop:
        cmd.add(["-enable-experimental-cxx-interop"])

    serialize_debugging_options = False
    if ctx.attrs.serialize_debugging_options:
        if objc_headers:
            # TODO(T99100029): We cannot use VFS overlays with Buck2, so we have to disable
            # serializing debugging options for mixed libraries to debug successfully
            warning("Mixed libraries cannot serialize debugging options, disabling for module `{}` in rule `{}`".format(module_name, ctx.label))
        elif not toolchain.prefix_serialized_debugging_options:
            warning("The current toolchain does not support prefixing serialized debugging options, disabling for module `{}` in rule `{}`".format(module_name, ctx.label))
        else:
            # Apply the debug prefix map to Swift serialized debugging info.
            # This will allow for debugging remotely built swiftmodule files.
            serialize_debugging_options = True

    if serialize_debugging_options:
        cmd.add([
            "-Xfrontend",
            "-serialize-debugging-options",
            "-Xfrontend",
            "-prefix-serialized-debugging-options",
        ])
    else:
        cmd.add([
            "-Xfrontend",
            "-no-serialize-debugging-options",
        ])

    if toolchain.can_toolchain_emit_obj_c_header_textually:
        cmd.add([
            "-Xfrontend",
            "-emit-objc-header-textually",
        ])

    # Add flags required to import ObjC module dependencies
    _add_clang_deps_flags(ctx, cmd)
    _add_swift_deps_flags(ctx, cmd)

    # Add flags for importing the ObjC part of this library
    _add_mixed_library_flags_to_cmd(cmd, objc_headers, objc_modulemap_pp_info)

    # Add toolchain and target flags last to allow for overriding defaults
    cmd.add(toolchain.compiler_flags)
    cmd.add(ctx.attrs.swift_compiler_flags)
    cmd.add(extra_search_paths_flags)

    return cmd

def _add_swift_deps_flags(ctx: "context", cmd: "cmd_args"):
    # If Explicit Modules are enabled, a few things must be provided to a compilation job:
    # 1. Direct and transitive SDK deps from `sdk_modules` attribute.
    # 2. Direct and transitive user-defined deps.
    # 3. Transitive SDK deps of user-defined deps.
    # (This is the case, when a user-defined dep exports a type from SDK module,
    # thus such SDK module should be implicitly visible to consumers of that custom dep)
    if _uses_explicit_modules(ctx):
        toolchain = ctx.attrs._apple_toolchain[AppleToolchainInfo].swift_toolchain_info
        module_name = get_module_name(ctx)
        sdk_deps_tset = get_sdk_deps_tset(
            ctx,
            module_name,
            ctx.attrs.deps + ctx.attrs.exported_deps,
            _REQUIRED_SDK_MODULES,
            toolchain,
        )
        swift_deps_tset = ctx.actions.tset(
            SwiftmodulePathsTSet,
            children = _get_swift_paths_tsets(ctx.attrs.deps + ctx.attrs.exported_deps),
        )
        swift_module_map_artifact = write_swift_module_map_with_swift_deps(
            ctx,
            module_name,
            list(sdk_deps_tset.traverse()),
            list(swift_deps_tset.traverse()),
        )
        cmd.add([
            "-Xfrontend",
            "-disable-implicit-swift-modules",
            "-Xfrontend",
            "-explicit-swift-module-map-file",
            "-Xfrontend",
            swift_module_map_artifact,
        ])

        # Add Clang sdk modules which do not go to swift modulemap
        cmd.add(sdk_deps_tset.project_as_args("clang_deps"))

        # Swift compilation should depend on transitive Swift modules from swift-module-map.
        cmd.hidden(sdk_deps_tset.project_as_args("hidden"))
        cmd.hidden(swift_deps_tset.project_as_args("hidden"))
    else:
        depset = ctx.actions.tset(SwiftmodulePathsTSet, children = _get_swift_paths_tsets(ctx.attrs.deps + ctx.attrs.exported_deps))
        cmd.add(depset.project_as_args("module_search_path"))

def _add_clang_deps_flags(ctx: "context", cmd: "cmd_args") -> None:
    # If a module uses Explicit Modules, all direct and
    # transitive Clang deps have to be explicitly added.
    if _uses_explicit_modules(ctx):
        pcm_deps_tset = get_pcm_deps_tset(ctx, ctx.attrs.deps + ctx.attrs.exported_deps)
        cmd.add(pcm_deps_tset.project_as_args("clang_deps"))
    else:
        inherited_preprocessor_infos = cxx_inherited_preprocessor_infos(ctx.attrs.deps + ctx.attrs.exported_deps)
        preprocessors = cxx_merge_cpreprocessors(ctx, [], inherited_preprocessor_infos)
        cmd.add(cmd_args(preprocessors.set.project_as_args("args"), prepend = "-Xcc"))
        cmd.add(cmd_args(preprocessors.set.project_as_args("modular_args"), prepend = "-Xcc"))
        cmd.add(cmd_args(preprocessors.set.project_as_args("include_dirs"), prepend = "-Xcc"))

def _add_mixed_library_flags_to_cmd(
        cmd: "cmd_args",
        objc_headers: [CHeader.type],
        objc_modulemap_pp_info: ["CPreprocessor", None]) -> None:
    if not objc_headers:
        return

    # TODO(T99100029): We cannot use VFS overlays to mask this import from
    # the debugger as they require absolute paths. Instead we will enforce
    # that mixed libraries do not have serialized debugging info and rely on
    # rdeps to serialize the correct paths.
    for arg in objc_modulemap_pp_info.args:
        cmd.add("-Xcc")
        cmd.add(arg)

    for arg in objc_modulemap_pp_info.modular_args:
        cmd.add("-Xcc")
        cmd.add(arg)

    cmd.add("-import-underlying-module")

def _get_swift_paths_tsets(deps: ["dependency"]) -> ["SwiftmodulePathsTSet"]:
    return [
        d[SwiftDependencyInfo].exported_swiftmodule_paths
        for d in deps
        if SwiftDependencyInfo in d
    ]

def _get_transitive_swift_paths_tsets(deps: ["dependency"]) -> ["SwiftmodulePathsTSet"]:
    return [
        d[SwiftDependencyInfo].transitive_swiftmodule_paths
        for d in deps
        if SwiftDependencyInfo in d
    ]

def _get_exported_headers_tset(ctx: "context", exported_headers: [["string"], None] = None) -> "ExportedHeadersTSet":
    return ctx.actions.tset(
        ExportedHeadersTSet,
        value = {get_module_name(ctx): exported_headers} if exported_headers else None,
        children = [
            dep.exported_headers
            for dep in [x.get(SwiftDependencyInfo) for x in ctx.attrs.exported_deps]
            if dep and dep.exported_headers
        ],
    )

def get_swift_pcm_compile_info(
        ctx: "context",
        propagated_exported_preprocessor_info: ["CPreprocessorInfo", None],
        exported_pre: ["CPreprocessor", None]) -> ["SwiftPCMCompilationInfo", None]:
    swift_toolchain = ctx.attrs._apple_toolchain[AppleToolchainInfo].swift_toolchain_info

    # If a toolchain supports explicit modules, exported PP exists and a target is modular,
    # let's precompile a modulemap in order to enable consumptions by Swift.
    if is_sdk_modules_provided(swift_toolchain) and exported_pre and exported_pre.modulemap_path and ctx.attrs.modular:
        return compile_swift_pcm(
            ctx,
            exported_pre,
            propagated_exported_preprocessor_info,
        )
    return None

def get_swift_dependency_info(
        ctx: "context",
        exported_pre: ["CPreprocessor", None],
        output_module: ["artifact", None]) -> "SwiftDependencyInfo":
    all_deps = ctx.attrs.exported_deps + ctx.attrs.deps
    if ctx.attrs.reexport_all_header_dependencies:
        exported_deps = all_deps
    else:
        exported_deps = ctx.attrs.exported_deps

    exported_headers = [_header_basename(header) for header in ctx.attrs.exported_headers]
    exported_headers += [header.name for header in exported_pre.headers] if exported_pre else []

    if output_module:
        exported_swiftmodules = ctx.actions.tset(SwiftmodulePathsTSet, value = output_module, children = _get_swift_paths_tsets(exported_deps))
        transitive_swiftmodules = ctx.actions.tset(SwiftmodulePathsTSet, value = output_module, children = _get_transitive_swift_paths_tsets(all_deps))
    else:
        exported_swiftmodules = ctx.actions.tset(SwiftmodulePathsTSet, children = _get_swift_paths_tsets(exported_deps))
        transitive_swiftmodules = ctx.actions.tset(SwiftmodulePathsTSet, children = _get_transitive_swift_paths_tsets(all_deps))

    return SwiftDependencyInfo(
        exported_headers = _get_exported_headers_tset(ctx, exported_headers),
        exported_swiftmodule_paths = exported_swiftmodules,
        transitive_swiftmodule_paths = transitive_swiftmodules,
    )

def _header_basename(header: ["artifact", "string"]) -> "string":
    if type(header) == type(""):
        return paths.basename(header)
    else:
        return header.basename

def _uses_explicit_modules(ctx: "context") -> bool.type:
    swift_toolchain = ctx.attrs._apple_toolchain[AppleToolchainInfo].swift_toolchain_info
    return ctx.attrs.uses_explicit_modules and is_sdk_modules_provided(swift_toolchain)
