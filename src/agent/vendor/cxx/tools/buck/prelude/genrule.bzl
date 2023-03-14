# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

# Implementation of the `genrule` build rule.

load("@prelude//:cache_mode.bzl", "CacheModeInfo")
load("@prelude//:genrule_local_labels.bzl", "genrule_labels_require_local")
load("@prelude//utils:utils.bzl", "value_or")

# Currently, some rules require running from the project root, so provide an
# opt-in list for those here.  Longer-term, these should be ported to actual
# rule implementations in v2, rather then using `genrule`s.
_BUILD_ROOT_LABELS = {label: True for label in [
    # The buck2 test suite
    "buck2_test_build_root",
    "antlir_macros",
    "rust_bindgen",
    "haskell_hsc",
    "cql_cxx_genrule",
    "clang-module",
    "cuda_build_root",
    "bundle_pch_genrule",  # Compiles C++, and so need to run from build root
    "lpm_package",
    "haskell_dll",
]}

# In Buck1 the SRCS environment variable is only set if the substring SRCS is on the command line.
# That's a horrible heuristic, and doesn't account for users accessing $SRCS from a shell script.
# But in some cases, $SRCS is so large it breaks the process limit, so have a label to opt in to
# that behavior.
_NO_SRCS_ENVIRONMENT_LABEL = "no_srcs_environment"

def _requires_build_root(ctx: "context") -> bool.type:
    for label in ctx.attrs.labels:
        if label in _BUILD_ROOT_LABELS:
            return True
    return False

def _requires_local(ctx: "context") -> bool.type:
    return genrule_labels_require_local(ctx.attrs.labels)

def _ignore_artifacts(ctx: "context") -> bool.type:
    return "buck2_ignore_artifacts" in ctx.attrs.labels

def _requires_no_srcs_environment(ctx: "context") -> bool.type:
    return _NO_SRCS_ENVIRONMENT_LABEL in ctx.attrs.labels

# There is a special use case of `default_outs` which is pretty frequent:
# ```
# default_outs = ["."],
# ```
# which makes the whole $OUT directory a default output.
# To handle it in a v1 compatible way create an auxiliary symlinked directory with all output artifacts
# and return it as a single output.
def _should_handle_special_case_whole_out_dir_is_output(ctx: "context", outs_attr: dict.type):
    for (_, item_outputs) in outs_attr.items():
        # Situation when `"."` is both in `outs` and `default_outs` is handled by default
        if "." in item_outputs:
            return False
    default_outs = ctx.attrs.default_outs
    if default_outs and default_outs[0] == ".":
        if len(default_outs) != 1:
            fail("When present, `.` should be a single element in `default_outs`.")
        return True
    return False

# We don't want to use cache mode in open source because the config keys that drive it aren't wired up
# @oss-disable: _USE_CACHE_MODE = True 
_USE_CACHE_MODE = False # @oss-enable

# Extra attributes required by every genrule based on genrule_impl
def genrule_attributes() -> {str.type: "attribute"}:
    if _USE_CACHE_MODE:
        # FIXME: prelude// should be standalone (not refer to fbsource//)
        return {"_cache_mode": attrs.dep(default = "fbsource//xplat/buck2/platform/cache_mode:cache_mode")}
    else:
        return {}

def _get_cache_mode(ctx: "context") -> CacheModeInfo.type:
    if _USE_CACHE_MODE:
        return ctx.attrs._cache_mode[CacheModeInfo]
    else:
        return CacheModeInfo(allow_cache_uploads = False, cache_bust_genrules = False)

def genrule_impl(ctx: "context") -> ["provider"]:
    # Directories:
    #   sh - sh file
    #   src - sources files
    #   out - where outputs go
    # `src` is the current directory
    # Buck1 uses `.` as output, but that won't work since
    # Buck2 clears the output directory before execution, and thus src/sh too.
    return process_genrule(ctx, ctx.attrs.out, ctx.attrs.outs)

def _declare_output(ctx: "context", path: str.type) -> "artifact":
    if path == ".":
        return ctx.actions.declare_output("out")
    elif path.endswith("/"):
        return ctx.actions.declare_output("out", path[:-1])
    else:
        return ctx.actions.declare_output("out", path)

def process_genrule(
        ctx: "context",
        out_attr: [str.type, None],
        outs_attr: [dict.type, None],
        extra_env_vars: dict.type = {},
        identifier: [str.type, None] = None) -> ["provider"]:
    if (out_attr != None) and (outs_attr != None):
        fail("Only one of `out` and `outs` should be set. Got out=`%s`, outs=`%s`" % (repr(out_attr), repr(outs_attr)))

    local_only = _requires_local(ctx)

    # NOTE: Eventually we shouldn't require local_only here, since we should be
    # fine with caching local fallbacks if necessary (or maybe that should be
    # disallowed as a matter of policy), but for now let's be safe.
    cacheable = value_or(ctx.attrs.cacheable, True) and local_only

    handle_whole_out_dir_is_output = False
    default_out_map = {}

    # TODO(cjhopman): verify output paths are ".", "./", or forward-relative.
    if out_attr != None:
        out_env = out_attr
        out_artifact = _declare_output(ctx, out_attr)
        default_outputs = [out_artifact]
        all_outputs = default_outputs
        named_outputs = {}
    elif outs_attr != None:
        out_env = ""

        default_outputs = []
        all_outputs = []
        named_outputs = {}
        default_out_paths = ctx.attrs.default_outs or []

        handle_whole_out_dir_is_output = _should_handle_special_case_whole_out_dir_is_output(ctx, outs_attr)

        for (name, this_outputs) in outs_attr.items():
            output_artifacts = []
            for path in this_outputs:
                artifact = _declare_output(ctx, path)
                if path in default_out_paths:
                    default_outputs.append(artifact)
                output_artifacts.append(artifact)
                default_out_map[path] = artifact
            named_outputs[name] = output_artifacts
            all_outputs.extend(output_artifacts)

        if handle_whole_out_dir_is_output:
            # handle it later when artifacts are bound
            pass
        elif len(default_outputs) != len(default_out_paths):
            # TODO(akozhevnikov) handle arbitrary `default_out`, currently fallback to all outputs to support
            # cases when `default_out` points to directory containing all files from `outs`
            warning("Could not properly handle `default_outs` and `outs` parameters of `{}` rule, default outputs for the rule are defaulted to all artifacts from `outs` parameter.".format(ctx.label))
            default_outputs = all_outputs
        elif len(default_outputs) == 0:
            # We want building to force something to be built, so make sure it contains at least one artifact
            default_outputs = all_outputs
    else:
        fail("One of `out` or `outs` should be set. Got `%s`" % repr(ctx.attrs))

    # Some custom rules use `process_genrule` but doesn't set this attrbiute.
    is_windows = hasattr(ctx.attrs, "_target_os_type") and ctx.attrs._target_os_type == "windows"
    if is_windows:
        path_sep = "\\"
        cmd = ctx.attrs.cmd_exe if ctx.attrs.cmd_exe != None else ctx.attrs.cmd
        if cmd == None:
            fail("One of `cmd` or `cmd_exe` should be set.")
    else:
        path_sep = "/"
        cmd = ctx.attrs.bash if ctx.attrs.bash != None else ctx.attrs.cmd
        if cmd == None:
            fail("One of `cmd` or `bash` should be set.")
    cmd = cmd_args(cmd)

    # For backwards compatibility with Buck1.
    if is_windows:
        # Replace $OUT and ${OUT}
        cmd.replace_regex("\\$(OUT\\b|\\{OUT\\})", "%OUT%")
        cmd.replace_regex("\\$(SRCDIR\\b|\\{SRCDIR\\})", "%SRCDIR%")
        cmd.replace_regex("\\$(SRCS\\b|\\{SRCS\\})", "%SRCS%")
        cmd.replace_regex("\\$(TMP\\b|\\{TMP\\})", "%TMP%")

    if _ignore_artifacts(ctx):
        cmd = cmd.ignore_artifacts()

    if type(ctx.attrs.srcs) == type([]):
        # FIXME: We should always use the short_path, but currently that is sometimes blank.
        # See fbcode//buck2/tests/targets/rules/genrule:genrule-dot-input for a test that exposes it.
        symlinks = {src.short_path: src for src in ctx.attrs.srcs}

        if len(symlinks) != len(ctx.attrs.srcs):
            for src in ctx.attrs.srcs:
                name = src.short_path
                if symlinks[name] != src:
                    msg = "genrule srcs include duplicative name: `{}`. ".format(name)
                    msg += "`{}` conflicts with `{}`".format(symlinks[name].owner, src.owner)
                    fail(msg)
    else:
        symlinks = ctx.attrs.srcs
    srcs_artifact = ctx.actions.symlinked_dir("srcs" if not identifier else "{}-srcs".format(identifier), symlinks)

    # Setup environment variables.
    srcs = cmd_args()
    for symlink in symlinks:
        srcs.add(cmd_args(srcs_artifact, format = path_sep.join([".", "{}", symlink])))
    env_vars = {
        "ASAN_OPTIONS": cmd_args("detect_leaks=0,detect_odr_violation=0"),
        "GEN_DIR": cmd_args("GEN_DIR_DEPRECATED"),  # ctx.relpath(ctx.output_root_dir(), srcs_path)
        "OUT": cmd_args(srcs_artifact, format = path_sep.join([".", "{}", "..", "out", out_env])),
        "SRCDIR": cmd_args(srcs_artifact, format = path_sep.join([".", "{}"])),
        "SRCS": srcs,
    } | {k: cmd_args(v) for k, v in getattr(ctx.attrs, "env", {}).items()}

    # RE will cache successful actions that don't produce the desired outptuts,
    # so if that happens and _then_ we add a local-only label, we'll get a
    # cache hit on the action that didn't produce the outputs and get the error
    # again (thus making the label useless). So, when a local-only label is
    # set, we make the action *different*.
    if local_only:
        env_vars["__BUCK2_LOCAL_ONLY_CACHE_BUSTER"] = cmd_args("")

    # For now, when uploads are enabled, be safe and avoid sharing cache hits.
    cache_bust = _get_cache_mode(ctx).cache_bust_genrules

    if cacheable and cache_bust:
        env_vars["__BUCK2_ALLOW_CACHE_UPLOADS_CACHE_BUSTER"] = cmd_args("")

    if _requires_no_srcs_environment(ctx):
        env_vars.pop("SRCS")

    for key, value in extra_env_vars.items():
        env_vars[key] = value

    # Create required directories.
    if is_windows:
        script = [
            cmd_args(srcs_artifact, format = "if not exist .\\{}\\..\\out mkdir .\\{}\\..\\out"),
        ]
        script_extension = "bat"
    else:
        script = [
            # Use a somewhat unique exit code so this can get retried on RE (T99656531).
            cmd_args(srcs_artifact, format = "mkdir -p ./{}/../out || exit 99"),
            cmd_args("export TMP=${TMPDIR:-/tmp}"),
        ]
        script_extension = "sh"

    # Actually define the operation, relative to where we changed to
    script.append(cmd)

    # Some rules need to run from the build root, but for everything else, `cd`
    # into the sandboxed source dir and relative all paths to that.
    if not _requires_build_root(ctx):
        script = (
            # Change to the directory that genrules expect.
            [cmd_args(srcs_artifact, format = "cd {}")] +
            # Relative all paths in the command to the sandbox dir.
            [cmd.relative_to(srcs_artifact) for cmd in script]
        )

        # Relative all paths in the env to the sandbox dir.
        env_vars = {key: val.relative_to(srcs_artifact) for key, val in env_vars.items()}

    if is_windows:
        # Should be in the beginning.
        script = [cmd_args("@echo off")] + script

    sh_script, _ = ctx.actions.write(
        "sh/genrule.{}".format(script_extension) if not identifier else "sh/{}-genrule.{}".format(identifier, script_extension),
        script,
        is_executable = True,
        allow_args = True,
    )
    if is_windows:
        script_args = ["cmd.exe", "/c", sh_script]
    else:
        script_args = ["/bin/bash", "-e", sh_script]

    category = "genrule"
    if ctx.attrs.type != None:
        # As of 09/2021, all genrule types were legal snake case if their dashes and periods were replaced with underscores.
        category += "_" + ctx.attrs.type.replace("-", "_").replace(".", "_")
    ctx.actions.run(
        cmd_args(script_args).hidden([cmd, srcs_artifact] + [a.as_output() for a in all_outputs]),
        env = env_vars,
        local_only = local_only,
        allow_cache_upload = cacheable,
        category = category,
        identifier = identifier,
    )

    if handle_whole_out_dir_is_output:
        default_outputs = [ctx.actions.symlinked_dir("out_dir" if not identifier else "{}-outdir", default_out_map)]

    providers = [DefaultInfo(
        default_outputs = default_outputs,
        sub_targets = {k: [DefaultInfo(default_outputs = v)] for (k, v) in named_outputs.items()},
    )]

    # The cxx_genrule also forwards here, and that doesn't have .executable, so use getattr
    if getattr(ctx.attrs, "executable", False):
        if out_attr == None:
            providers.append(RunInfo(args = cmd_args(default_outputs)))
        else:
            providers.append(RunInfo(args = cmd_args(all_outputs)))
    return providers
