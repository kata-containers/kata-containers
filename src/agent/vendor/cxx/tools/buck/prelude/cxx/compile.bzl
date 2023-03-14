# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

load("@prelude//:paths.bzl", "paths")
load("@prelude//linking:lto.bzl", "LtoMode")
load(
    "@prelude//utils:utils.bzl",
    "flatten",
)
load(":attr_selection.bzl", "cxx_by_language_ext")
load(
    ":compiler.bzl",
    "get_flags_for_colorful_output",
    "get_flags_for_reproducible_build",
    "get_headers_dep_files_flags_factory",
    "get_output_flags",
    "get_pic_flags",
)
load(":cxx_context.bzl", "get_cxx_toolchain_info")
load(":debug.bzl", "SplitDebugMode")
load(
    ":headers.bzl",
    "CPrecompiledHeaderInfo",
)
load(":platform.bzl", "cxx_by_platform")
load(
    ":preprocessor.bzl",
    "CPreprocessor",  # @unused Used as a type
    "CPreprocessorInfo",  # @unused Used as a type
    "cxx_attr_preprocessor_flags",
    "cxx_merge_cpreprocessors",
)

# Supported Cxx file extensions
CxxExtension = enum(
    ".cpp",
    ".cc",
    ".cxx",
    ".c++",
    ".c",
    ".s",
    ".S",
    ".m",
    ".mm",
    ".cu",
    ".hip",
    ".asm",
    ".asmpp",
    ".h",
    ".hpp",
)

# Information on argsfiles created for Cxx compilation.
_CxxCompileArgsfile = record(
    # The generated argsfile
    file = field("artifact"),
    # This argfile as a command form that would use the argfile
    cmd_form = field("cmd_args"),
    # The args that was written to the argfile
    argfile_args = field("cmd_args"),
    # The args in their prisitine form without shell quoting
    args = field("cmd_args"),
    # Hidden args necessary for the argsfile to reference
    hidden_args = field([["artifacts", "cmd_args"]]),
)

_HeadersDepFiles = record(
    # An executable to wrap the actual command with for post-processing of dep
    # files into the format that Buck2 recognizes (i.e. one artifact per line).
    processor = field("cmd_args"),
    # The tag that was added to headers.
    tag = field("artifact_tag"),
    # A function that produces new cmd_args to append to the compile command to
    # get it to emit the dep file. This will receive the output dep file as an
    # input.
    mk_flags = field("function"),
)

# Information about how to compile a source file of particular extension.
_CxxCompileCommand = record(
    # The compiler and any args which are independent of the rule.
    base_compile_cmd = field("cmd_args"),
    # The argsfile of arguments from the rule and it's dependencies.
    argsfile = field(_CxxCompileArgsfile.type),
    headers_dep_files = field([_HeadersDepFiles.type, None]),
    compiler_type = field(str.type),
)

# Information about how to compile a source file.
CxxSrcCompileCommand = record(
    # Source file to compile.
    src = field("artifact"),
    # If we have multiple source entries with same files but different flags,
    # specify an index so we can differentiate them. Otherwise, use None.
    index = field(["int", None], None),
    # The CxxCompileCommand to use to compile this file.
    cxx_compile_cmd = field(_CxxCompileCommand.type),
    # Arguments specific to the source file.
    args = field(["_arg"]),
)

# Output of creating compile commands for Cxx source files.
CxxCompileCommandOutput = record(
    # List of compile commands for each source file
    src_compile_cmds = field([CxxSrcCompileCommand.type]),
    # Argsfiles to generate in order to compile these source files
    argsfiles_info = field(DefaultInfo.type),
    # Each argsfile by the file extension for which it is used
    argsfile_by_ext = field({str.type: "artifact"}),
)

# Output of creating compile commands for Cxx source files.
CxxCompileCommandOutputForCompDb = record(
    # Output of creating compile commands for Cxx source files.
    source_commands = field(CxxCompileCommandOutput.type),
    # this field is only to be used in CDB generation
    comp_db_commands = field(CxxCompileCommandOutput.type),
)

# An input to cxx compilation, consisting of a file to compile and optional
# file specific flags to compile with.
CxxSrcWithFlags = record(
    file = field("artifact"),
    flags = field(["resolved_macro"], []),
    # If we have multiple source entries with same files but different flags,
    # specify an index so we can differentiate them. Otherwise, use None.
    index = field(["int", None], None),
)

CxxCompileOutput = record(
    # The compiled `.o` file.
    object = field("artifact"),
    object_has_external_debug_info = field(bool.type, False),
    # Externally referenced debug info, which doesn't get linked with the
    # object (e.g. the above `.o` when using `-gsplit-dwarf=single` or the
    # the `.dwo` when using `-gsplit-dwarf=split`).
    external_debug_info = field(["artifact", None], None),
)

def create_compile_cmds(
        ctx: "context",
        impl_params: "CxxRuleConstructorParams",
        own_preprocessors: [CPreprocessor.type],
        inherited_preprocessor_infos: [CPreprocessorInfo.type]) -> CxxCompileCommandOutputForCompDb.type:
    """
    Forms the CxxSrcCompileCommand to use for each source file based on it's extension
    and optional source file flags. Returns CxxCompileCommandOutput containing an array
    of the generated compile commands and argsfile output.
    """

    srcs_with_flags = []
    for src in impl_params.srcs:
        srcs_with_flags.append(src)
    header_only = False
    if len(srcs_with_flags) == 0 and len(impl_params.additional.srcs) == 0:
        all_headers = flatten([x.headers for x in own_preprocessors])
        if len(all_headers) == 0:
            all_raw_headers = flatten([x.raw_headers for x in own_preprocessors])
            if len(all_raw_headers) != 0:
                header_only = True
                for header in all_raw_headers:
                    if header.extension in [".h", ".hpp"]:
                        srcs_with_flags.append(CxxSrcWithFlags(file = header))
            else:
                return CxxCompileCommandOutputForCompDb(
                    source_commands = CxxCompileCommandOutput(src_compile_cmds = [], argsfiles_info = DefaultInfo(), argsfile_by_ext = {}),
                    comp_db_commands = CxxCompileCommandOutput(src_compile_cmds = [], argsfiles_info = DefaultInfo(), argsfile_by_ext = {}),
                )
        else:
            header_only = True
            for header in all_headers:
                if header.artifact.extension in [".h", ".hpp"]:
                    srcs_with_flags.append(CxxSrcWithFlags(file = header.artifact))

    # TODO(T110378129): Buck v1 validates *all* headers used by a compilation
    # at compile time, but that doing that here/eagerly might be expensive (but
    # we should figure out something).
    _validate_target_headers(ctx, own_preprocessors)

    # Combine all preprocessor info and prepare it for compilations.
    pre = cxx_merge_cpreprocessors(
        ctx,
        filter(None, own_preprocessors + impl_params.extra_preprocessors),
        inherited_preprocessor_infos,
    )

    headers_tag = ctx.actions.artifact_tag()

    src_compile_cmds = []
    cxx_compile_cmd_by_ext = {}
    argsfile_by_ext = {}

    for src in srcs_with_flags:
        ext = CxxExtension(src.file.extension)

        # Deduplicate shared arguments to save memory. If we compile multiple files
        # of the same extension they will have some of the same flags. Save on
        # allocations by caching and reusing these objects.
        if not ext in cxx_compile_cmd_by_ext:
            toolchain = get_cxx_toolchain_info(ctx)
            compiler_info = _get_compiler_info(toolchain, ext)
            base_compile_cmd = _get_compile_base(compiler_info)

            headers_dep_files = None
            if _supports_dep_files(ext) and toolchain.use_dep_files:
                mk_dep_files_flags = get_headers_dep_files_flags_factory(compiler_info.compiler_type)
                if mk_dep_files_flags:
                    headers_dep_files = _HeadersDepFiles(
                        processor = cmd_args(compiler_info.dep_files_processor),
                        mk_flags = mk_dep_files_flags,
                        tag = headers_tag,
                    )

            argsfile_by_ext[ext.value] = _mk_argsfile(ctx, compiler_info, pre, ext, headers_tag)
            cxx_compile_cmd_by_ext[ext] = _CxxCompileCommand(
                base_compile_cmd = base_compile_cmd,
                argsfile = argsfile_by_ext[ext.value],
                headers_dep_files = headers_dep_files,
                compiler_type = compiler_info.compiler_type,
            )

        cxx_compile_cmd = cxx_compile_cmd_by_ext[ext]

        src_args = []
        src_args.extend(src.flags)
        src_args.extend(["-c", src.file])

        src_compile_command = CxxSrcCompileCommand(src = src.file, cxx_compile_cmd = cxx_compile_cmd, args = src_args, index = src.index)
        src_compile_cmds.append(src_compile_command)

    # Create an output file of all the argsfiles generated for compiling these source files.
    argsfiles = []
    argsfile_names = cmd_args()
    other_outputs = []
    argsfile_artifacts_by_ext = {}
    for ext, argsfile in argsfile_by_ext.items():
        argsfiles.append(argsfile.file)
        argsfile_names.add(cmd_args(argsfile.file).ignore_artifacts())
        other_outputs.extend(argsfile.hidden_args)
        argsfile_artifacts_by_ext[ext] = argsfile.file

    for argsfile in impl_params.additional.argsfiles:
        argsfiles.append(argsfile.file)
        argsfile_names.add(cmd_args(argsfile.file).ignore_artifacts())
        other_outputs.extend(argsfile.hidden_args)

    argsfiles_summary = ctx.actions.write("argsfiles", argsfile_names)

    # Create a provider that will output all the argsfiles necessary and generate those argsfiles.
    argsfiles = DefaultInfo(default_outputs = [argsfiles_summary] + argsfiles, other_outputs = other_outputs)

    if header_only:
        return CxxCompileCommandOutputForCompDb(
            source_commands = CxxCompileCommandOutput(src_compile_cmds = [], argsfiles_info = DefaultInfo(), argsfile_by_ext = {}),
            comp_db_commands = CxxCompileCommandOutput(src_compile_cmds = src_compile_cmds, argsfiles_info = argsfiles, argsfile_by_ext = argsfile_artifacts_by_ext),
        )
    else:
        return CxxCompileCommandOutputForCompDb(
            source_commands = CxxCompileCommandOutput(src_compile_cmds = src_compile_cmds, argsfiles_info = argsfiles, argsfile_by_ext = argsfile_artifacts_by_ext),
            comp_db_commands = CxxCompileCommandOutput(src_compile_cmds = src_compile_cmds, argsfiles_info = argsfiles, argsfile_by_ext = argsfile_artifacts_by_ext),
        )

def compile_cxx(
        ctx: "context",
        src_compile_cmds: [CxxSrcCompileCommand.type],
        pic: bool.type = False) -> [CxxCompileOutput.type]:
    """
    For a given list of src_compile_cmds, generate output artifacts.
    """
    toolchain = get_cxx_toolchain_info(ctx)
    linker_info = toolchain.linker_info

    objects = []
    for src_compile_cmd in src_compile_cmds:
        identifier = src_compile_cmd.src.short_path
        if src_compile_cmd.index != None:
            # Add a unique postfix if we have duplicate source files with different flags
            identifier = identifier + "_" + str(src_compile_cmd.index)

        filename_base = identifier + (".pic" if pic else "")
        object = ctx.actions.declare_output(
            paths.join("__objects__", "{}.{}".format(filename_base, linker_info.object_file_extension)),
        )

        cmd = cmd_args(src_compile_cmd.cxx_compile_cmd.base_compile_cmd)

        compiler_type = src_compile_cmd.cxx_compile_cmd.compiler_type
        cmd.add(get_output_flags(compiler_type, object))

        args = cmd_args()

        if pic:
            args.add(get_pic_flags(compiler_type))

        args.add(src_compile_cmd.cxx_compile_cmd.argsfile.cmd_form)
        args.add(src_compile_cmd.args)

        cmd.add(args)

        action_dep_files = {}

        headers_dep_files = src_compile_cmd.cxx_compile_cmd.headers_dep_files
        if headers_dep_files:
            intermediary_dep_file = ctx.actions.declare_output(
                paths.join("__dep_files_intermediaries__", filename_base),
            ).as_output()
            dep_file = ctx.actions.declare_output(
                paths.join("__dep_files__", filename_base),
            ).as_output()

            dep_file_flags = headers_dep_files.mk_flags(intermediary_dep_file)
            cmd.add(dep_file_flags)

            # API: First argument is the dep file source path, second is the
            # dep file destination path, other arguments are the actual compile
            # command.
            cmd = cmd_args([
                headers_dep_files.processor,
                intermediary_dep_file,
                headers_dep_files.tag.tag_artifacts(dep_file),
                cmd,
            ])

            action_dep_files["headers"] = headers_dep_files.tag

        if pic:
            identifier += " (pic)"
        ctx.actions.run(cmd, category = "cxx_compile", identifier = identifier, dep_files = action_dep_files)

        # If we're building with split debugging, where the debug info is in the
        # original object, then add the object as external debug info, *unless*
        # we're doing LTO, which generates debug info at link time (*except* for
        # fat LTO, which still generates native code and, therefore, debug info).
        object_has_external_debug_info = (
            toolchain.split_debug_mode == SplitDebugMode("single") and
            linker_info.lto_mode in (LtoMode("none"), LtoMode("fat"))
        )

        objects.append(CxxCompileOutput(
            object = object,
            object_has_external_debug_info = object_has_external_debug_info,
        ))

    return objects

def _validate_target_headers(ctx: "context", preprocessor: [CPreprocessor.type]):
    path_to_artifact = {}
    all_headers = flatten([x.headers for x in preprocessor])
    for header in all_headers:
        header_path = paths.join(header.namespace, header.name)
        artifact = path_to_artifact.get(header_path)
        if artifact != None:
            if artifact != header.artifact:
                fail("Conflicting headers {} and {} map to {} in target {}".format(artifact, header.artifact, header_path, ctx.label))
        else:
            path_to_artifact[header_path] = header.artifact

def _get_compiler_info(toolchain: "CxxToolchainInfo", ext: CxxExtension.type) -> "_compiler_info":
    if ext.value in (".cpp", ".cc", ".mm", ".cxx", ".c++", ".h", ".hpp"):
        return toolchain.cxx_compiler_info
    elif ext.value in (".c", ".m"):
        return toolchain.c_compiler_info
    elif ext.value in (".s", ".S"):
        return toolchain.as_compiler_info
    elif ext.value == ".cu":
        return toolchain.cuda_compiler_info
    elif ext.value == ".hip":
        return toolchain.hip_compiler_info
    elif ext.value in (".asm", ".asmpp"):
        return toolchain.asm_compiler_info
    else:
        # This should be unreachable as long as we handle all enum values
        fail("Unknown C++ extension: " + ext.value)

def _get_compile_base(compiler_info: "_compiler_info") -> "cmd_args":
    """
    Given a compiler info returned by _get_compiler_info, form the base compile args.
    """

    cmd = cmd_args(compiler_info.compiler)

    return cmd

def _supports_dep_files(ext: CxxExtension.type) -> bool.type:
    # Raw assembly doesn't make sense to capture dep files for.
    if ext.value in (".s", ".S", ".asm"):
        return False
    elif ext.value == ".hip":
        # TODO (T118797886): HipCompilerInfo doesn't have dep files processor.
        # Should it?
        return False
    return True

def _add_compiler_info_flags(compiler_info: "_compiler_info", ext: CxxExtension.type, cmd: "cmd_args"):
    cmd.add(compiler_info.preprocessor_flags or [])
    cmd.add(compiler_info.compiler_flags or [])
    cmd.add(get_flags_for_reproducible_build(compiler_info.compiler_type))

    if ext.value not in (".asm", ".asmpp"):
        # Clang's asm compiler doesn't support colorful output, so we skip this there.
        cmd.add(get_flags_for_colorful_output(compiler_info.compiler_type))

def _mk_argsfile(ctx: "context", compiler_info: "_compiler_info", preprocessor: CPreprocessorInfo.type, ext: CxxExtension.type, headers_tag: "artifact_tag") -> _CxxCompileArgsfile.type:
    """
    Generate and return an {ext}.argsfile artifact and command args that utilize the argsfile.
    """
    args = cmd_args()

    _add_compiler_info_flags(compiler_info, ext, args)

    args.add(headers_tag.tag_artifacts(preprocessor.set.project_as_args("args")))

    # Different preprocessors will contain whether to use modules,
    # and the modulemap to use, so we need to get the final outcome.
    if preprocessor.set.reduce("uses_modules"):
        args.add(headers_tag.tag_artifacts(preprocessor.set.project_as_args("modular_args")))

    args.add(cxx_attr_preprocessor_flags(ctx, ext.value))
    args.add(_attr_compiler_flags(ctx, ext.value))
    args.add(headers_tag.tag_artifacts(preprocessor.set.project_as_args("include_dirs")))

    # Workaround as that's not precompiled, but working just as prefix header.
    # Another thing is that it's clang specific, should be generalized.
    if ctx.attrs.precompiled_header != None:
        args.add(["-include", headers_tag.tag_artifacts(ctx.attrs.precompiled_header[CPrecompiledHeaderInfo].header)])
    if ctx.attrs.prefix_header != None:
        args.add(["-include", headers_tag.tag_artifacts(ctx.attrs.prefix_header)])

    shell_quoted_args = cmd_args(args, quote = "shell")
    argfile, _ = ctx.actions.write(ext.value + ".argsfile", shell_quoted_args, allow_args = True)

    hidden_args = [args]

    cmd_form = cmd_args(argfile, format = "@{}").hidden(hidden_args)

    return _CxxCompileArgsfile(file = argfile, cmd_form = cmd_form, argfile_args = shell_quoted_args, args = args, hidden_args = hidden_args)

def _attr_compiler_flags(ctx: "context", ext: str.type) -> [""]:
    return (
        ctx.attrs.compiler_flags +
        cxx_by_language_ext(ctx.attrs.lang_compiler_flags, ext) +
        flatten(cxx_by_platform(ctx, ctx.attrs.platform_compiler_flags)) +
        flatten(cxx_by_platform(ctx, cxx_by_language_ext(ctx.attrs.lang_platform_compiler_flags, ext)))
    )
