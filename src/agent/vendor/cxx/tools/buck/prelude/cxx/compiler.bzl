# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

# TODO(T110378132): Added here for compat with v1, but this might make more
# sense on the toolchain definition.
def get_flags_for_reproducible_build(compiler_type: str.type) -> [str.type]:
    """
    Return flags needed to make compilations reproducible (e.g. avoiding
    embedding the working directory into debug info.
    """

    flags = []

    if compiler_type in ["clang_cl", "windows"]:
        flags.append("/Brepro")

    if compiler_type in ["clang", "clang_windows", "clang_cl"]:
        flags.extend(["-Xclang", "-fdebug-compilation-dir", "-Xclang", "."])

    if compiler_type == "clang_windows":
        flags.append("-mno-incremental-linker-compatible")

    return flags

def get_flags_for_colorful_output(compiler_type: str.type) -> [str.type]:
    """
    Return flags for enabling colorful diagnostic output.
    """
    flags = []
    if compiler_type in ["clang", "clang_windows", "clang_cl"]:
        # https://clang.llvm.org/docs/UsersManual.html
        flags.append("-fcolor-diagnostics")
    elif compiler_type == "gcc":
        # https://gcc.gnu.org/onlinedocs/gcc/Diagnostic-Message-Formatting-Options.html
        flags.append("-fdiagnostics-color=always")

    return flags

def cc_dep_files(output: "_arglike") -> cmd_args.type:
    return cmd_args(["-MD", "-MF", output])

def windows_cc_dep_files(_output: "_arglike") -> cmd_args.type:
    return cmd_args(["/showIncludes"])

def get_headers_dep_files_flags_factory(compiler_type: str.type) -> ["function", None]:
    if compiler_type in ["clang", "gcc", "clang_windows"]:
        return cc_dep_files

    if compiler_type in ["windows", "clang_cl"]:
        return windows_cc_dep_files

    return None

def get_pic_flags(compiler_type: str.type) -> [str.type]:
    if compiler_type in ["clang", "gcc"]:
        return ["-fPIC"]
    else:
        return []

def get_output_flags(compiler_type: str.type, output: "artifact") -> [""]:
    if compiler_type in ["windows", "clang_cl", "windows_ml64"]:
        return [cmd_args(output.as_output(), format = "/Fo{}")]
    else:
        return ["-o", output.as_output()]
