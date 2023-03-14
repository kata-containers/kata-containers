# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

"""
Rule for the inplace pex builder, and some utility methods for generic pex builder
execution
"""

load("@prelude//:local_only.bzl", "package_python_locally")
load(
    "@prelude//linking:link_info.bzl",
    "LinkedObject",  # @unused Used as a type
)
load("@prelude//utils:utils.bzl", "flatten")
load(":interface.bzl", "PythonLibraryManifestsInterface")
load(":manifest.bzl", "ManifestInfo")  # @unused Used as a type
load(":toolchain.bzl", "PackageStyle", "PythonToolchainInfo")

# This represents the input to the creation of a Pex. Manifests provide source
# files, extensions are native extensions, and compile indicates whether we
# should also include bytecode from manifests.
PexModules = record(
    manifests = field(PythonLibraryManifestsInterface.type),
    extensions = field([ManifestInfo.type, None], None),
    extra_manifests = field([ManifestInfo.type, None], None),
    compile = field(bool.type, False),
)

# The output of pex creation. It's everything needed to make the DefaultInfo and RunInfo
# providers.
PexProviders = record(
    default_outputs = ["artifact"],
    other_outputs = ["artifact", "_arglike"],
    sub_targets = {str.type: ["provider"]},
    run_cmd = cmd_args.type,
)

def _srcs(srcs: [""], format = "{}") -> "cmd_args":
    args = cmd_args()
    for src in srcs:
        args.add(cmd_args(src, format = format))
    return args

# TODO(nmj): Resources
# TODO(nmj): Figure out how to harmonize these flags w/ existing make_xar
#                 invocations. It might be perfectly reasonable to just have a wrapper
#                 script that invokes make_xar in a slightly different way.
def make_pex(
        ctx: "context",
        python_toolchain: "PythonToolchainInfo",
        bundled_runtime: bool.type,
        package_style: PackageStyle.type,
        build_args: ["_arglike"],
        pex_modules: PexModules.type,
        shared_libraries: {str.type: (LinkedObject.type, bool.type)},
        main_module: str.type,
        hidden_resources: [None, "_arglike"]) -> PexProviders.type:
    """
    Passes a standardized set of flags to a `make_pex` binary to create a python
    "executable".

    Arguments:
        - python_toolchain: Used to locate the PEX binaries.
        - package_style: How to package this binary. Might be controlled by the
          toolchain, but also by the rule.
        - build_args: Extra arguments to pass to the PEX binary.
        - pex_modules: Manifests for sources to package.
        - shared_libraries: Shared libraries to link in. Mapping of soname to
          artifact and whether they should be preloaded.
        - main_module: the name of the module to execute when running the
          resulting binary.
        - hidden_resources: extra resources the binary depends on.
    """

    output = ctx.actions.declare_output("{}{}".format(ctx.attrs.name, python_toolchain.pex_extension))

    standalone = package_style == PackageStyle("standalone")

    runtime_files = []
    if standalone and hidden_resources != None:
        error_msg = _hidden_resources_error_message(ctx.label, hidden_resources)
        fail(error_msg)
    if hidden_resources != None:
        runtime_files.extend(hidden_resources)

    # HACK: runtime bundling is only implemented in make_par (via
    # python_toolchain.make_pex_standalone), so pretend we're doing a standalone build
    if bundled_runtime:
        standalone = True

    if not (standalone or
            package_style == PackageStyle("inplace") or
            package_style == PackageStyle("inplace_lite")):
        fail("unsupported package style: {}".format(package_style))

    symlink_tree_path = None
    if not standalone:
        symlink_tree_path = ctx.actions.declare_output("{}#link-tree".format(ctx.attrs.name))

    modules_args, hidden = _pex_modules_args(ctx, pex_modules, {name: lib for name, (lib, _) in shared_libraries.items()}, symlink_tree_path)

    bootstrap_args = _pex_bootstrap_args(
        ctx,
        python_toolchain.interpreter,
        None,
        python_toolchain.host_interpreter,
        main_module,
        output,
        shared_libraries,
        symlink_tree_path,
        package_style,
    )
    bootstrap_args.add(build_args)

    if standalone:
        if python_toolchain.make_pex_standalone == None:
            fail("Python toolchain does not provide make_pex_standalone")

        # We support building _standalone_ packages locally to e.g. support fbcode's
        # current style of build info stamping (e.g. T10696178).
        prefer_local = package_python_locally(ctx, python_toolchain)

        cmd = cmd_args(python_toolchain.make_pex_standalone)
        cmd.add(modules_args)
        cmd.add(bootstrap_args)
        ctx.actions.run(cmd, prefer_local = prefer_local, category = "par", identifier = "standalone")

    else:
        hidden.append(symlink_tree_path)
        modules = cmd_args(python_toolchain.make_pex_modules)
        modules.add(modules_args)
        ctx.actions.run(modules, category = "par", identifier = "modules")

        bootstrap = cmd_args(python_toolchain.make_pex_inplace)
        bootstrap.add(bootstrap_args)
        ctx.actions.run(bootstrap, category = "par", identifier = "bootstrap")

    runtime_files.extend(hidden)

    run_args = []

    # Windows can't run PAR directly.
    if ctx.attrs._target_os_type == "windows":
        run_args.append(ctx.attrs._python_toolchain[PythonToolchainInfo].interpreter)
    run_args.append(output)

    return PexProviders(
        default_outputs = [output],
        other_outputs = runtime_files,
        sub_targets = {},
        run_cmd = cmd_args(run_args).hidden(runtime_files),
    )

def _pex_bootstrap_args(
        ctx: "context",
        python_interpreter: "_arglike",
        python_interpreter_flags: [None, str.type],
        python_host_interpreter: "_arglike",
        main_module: str.type,
        output: "artifact",
        shared_libraries: {str.type: (LinkedObject.type, bool.type)},
        symlink_tree_path: [None, "artifact"],
        package_style: PackageStyle.type) -> "cmd_args":
    preload_libraries_path = ctx.actions.write(
        "__preload_libraries.txt",
        cmd_args([
            "--preload={}".format(name)
            for name, (_, preload) in shared_libraries.items()
            if preload
        ]),
    )

    cmd = cmd_args()
    cmd.add(cmd_args(preload_libraries_path, format = "@{}"))
    cmd.add([
        "--python",
        python_interpreter,
        "--host-python",
        python_host_interpreter,
        "--entry-point",
        main_module,
    ])
    if python_interpreter_flags:
        cmd.add("--python-interpreter-flags", python_interpreter_flags)
    if symlink_tree_path != None:
        cmd.add(cmd_args(["--modules-dir", symlink_tree_path]).ignore_artifacts())

    # Package style `inplace_lite` cannot be used with shared libraries
    if package_style == PackageStyle("inplace_lite") and not shared_libraries:
        cmd.add("--use-lite")
    cmd.add(output.as_output())

    return cmd

def _pex_modules_args(
        ctx: "context",
        pex_modules: PexModules.type,
        shared_libraries: {str.type: LinkedObject.type},
        symlink_tree_path: [None, "artifact"]) -> ("cmd_args", ["_arglike"]):
    """
    Produces args to deal with a PEX's modules. Returns args to pass to the
    modules builder, and artifacts the resulting modules would require at
    runtime (this might be empty for e.g. a standalone pex).
    """

    srcs = []
    src_artifacts = []

    srcs.extend(pex_modules.manifests.src_manifests())
    src_artifacts.extend(pex_modules.manifests.src_artifacts())

    if pex_modules.extensions:
        srcs.append(pex_modules.extensions.manifest)
        src_artifacts.extend(pex_modules.extensions.artifacts)

    if pex_modules.compile:
        srcs.extend(pex_modules.manifests.bytecode_manifests())
        src_artifacts.extend(pex_modules.manifests.bytecode_artifacts())

    if pex_modules.extra_manifests:
        srcs.append(pex_modules.extra_manifests.manifest)
        src_artifacts.extend(pex_modules.extra_manifests.artifacts)

    resources = pex_modules.manifests.resource_manifests()
    resource_artifacts = pex_modules.manifests.resource_artifacts()

    src_manifests_path = ctx.actions.write(
        "__src_manifests.txt",
        _srcs(srcs, format = "--module-manifest={}"),
    )
    resource_manifests_path = ctx.actions.write(
        "__resource_manifests.txt",
        _srcs(resources, format = "--resource-manifest={}"),
    )

    native_libraries = [s.output for s in shared_libraries.values()]
    native_library_srcs_path = ctx.actions.write(
        "__native_libraries___srcs.txt",
        _srcs(native_libraries, format = "--native-library-src={}"),
    )
    native_library_dests_path = ctx.actions.write(
        "__native_libraries___dests.txt",
        ["--native-library-dest={}".format(lib) for lib in shared_libraries],
    )

    src_manifest_args = cmd_args(src_manifests_path).hidden(srcs)
    resource_manifest_args = cmd_args(resource_manifests_path).hidden(resources)
    native_library_srcs_args = cmd_args(native_library_srcs_path)

    cmd = cmd_args()
    cmd.add(cmd_args(src_manifest_args, format = "@{}"))
    cmd.add(cmd_args(resource_manifest_args, format = "@{}"))
    cmd.add(cmd_args(native_library_srcs_args, format = "@{}"))
    cmd.add(cmd_args(native_library_dests_path, format = "@{}"))

    dwp = []
    if ctx.attrs.package_split_dwarf_dwp:
        dwp = [s.dwp for s in shared_libraries.values() if s.dwp != None]
        dwp_srcs_path = ctx.actions.write(
            "__dwp___srcs.txt",
            _srcs(dwp, format = "--dwp-src={}"),
        )
        dwp_dests_path = ctx.actions.write(
            "__dwp___dests.txt",
            ["--dwp-dest={}.dwp".format(lib) for lib, s in shared_libraries.items() if s.dwp != None],
        )
        dwp_srcs_args = cmd_args(dwp_srcs_path)
        cmd.add(cmd_args(dwp_srcs_args, format = "@{}"))
        cmd.add(cmd_args(dwp_dests_path, format = "@{}"))

    if symlink_tree_path != None:
        cmd.add(["--modules-dir", symlink_tree_path.as_output()])

    # Accumulate all the artifacts we depend on. Only add them to the command
    # if we are not going to create symlinks.
    hidden = (
        src_artifacts +
        resource_artifacts +
        native_libraries +
        dwp +
        flatten([lib.external_debug_info for lib in shared_libraries.values()])
    )
    if symlink_tree_path == None:
        cmd.hidden(hidden)
        hidden = []

    return (cmd, hidden)

def _hidden_resources_error_message(current_target: "label", hidden_resources) -> str.type:
    """
    Friendlier error message about putting non-python resources into standalone bins
    """
    owner_to_artifacts = {}

    for resource_set in hidden_resources:
        for resources in resource_set.traverse():
            for r in resources:
                if r.is_source:
                    # Source files; do a string repr so that we get the
                    # package path in there too
                    owner_to_artifacts.setdefault("", []).append(str(r))
                else:
                    owner_to_artifacts.setdefault(r.owner, []).append(r.short_path)

    msg = (
        "Cannot package hidden srcs/resources in a standalone python_binary. " +
        'Eliminate resources in non-Python dependencies of this python binary, use `package_style = "inplace"`, ' +
        'use `strip_mode="full"` or turn off Split DWARF `-c fbcode.split-dwarf=false` on C++ binary resources.\n'
    )

    for (rule, resources) in owner_to_artifacts.items():
        if rule != "":
            msg += "Hidden srcs/resources for {}\n".format(rule)
        else:
            msg += "Source files:\n"
            msg += "Find the reason this file was included with `buck2 cquery 'allpaths({}, owner(%s))' <file paths>`\n".format(current_target.raw_target())
        for resource in sorted(resources):
            msg += "  {}\n".format(resource)
    return msg
