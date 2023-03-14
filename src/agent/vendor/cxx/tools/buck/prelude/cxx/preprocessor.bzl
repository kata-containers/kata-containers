# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

load("@prelude//:paths.bzl", "paths")
load(
    "@prelude//utils:utils.bzl",
    "flatten",
    "value_or",
)
load(":attr_selection.bzl", "cxx_by_language_ext")
load(":cxx_context.bzl", "get_cxx_toolchain_info")
load(
    ":headers.bzl",
    "CHeader",  # @unused Used as a type
    "CxxHeadersLayout",  # @unused Used as a type
    "CxxHeadersNaming",
    "HeaderStyle",
    "HeadersAsRawHeadersMode",
    "as_raw_headers",
    "cxx_attr_exported_header_style",
    "cxx_attr_exported_headers",
    "cxx_attr_headers",
    "prepare_headers",
)
load(":platform.bzl", "cxx_by_platform")

CPreprocessor = record(
    # The arguments, [arglike things]
    args = field([""], []),
    # Header specs
    headers = field([CHeader.type], []),
    # Those should be mutually exclusive with normal headers as per documentation
    raw_headers = field(["artifact"], []),
    # Directories to be included via -I, [arglike things]
    include_dirs = field(["label_relative_path"], []),
    # Directories to be included via -isystem, [arglike things]
    system_include_dirs = field(["label_relative_path"], []),
    # Whether to compile with modules support
    uses_modules = field(bool.type, False),
    # Modular args to set when modules are in use, [arglike things]
    modular_args = field([""], []),
    modulemap_path = field("", None),
)

# Methods for transitive_sets must be declared prior to their use.

def _cpreprocessor_args(pres: [CPreprocessor.type]):
    args = cmd_args()
    for pre in pres:
        args.add(pre.args)
    return args

def _cpreprocessor_modular_args(pres: [CPreprocessor.type]):
    args = cmd_args()
    for pre in pres:
        args.add(pre.modular_args)
    return args

def _cpreprocessor_include_dirs(pres: [CPreprocessor.type]):
    args = cmd_args()
    for pre in pres:
        for d in pre.include_dirs:
            args.add("-I")
            args.add(d)
        for d in pre.system_include_dirs:
            args.add("-isystem")
            args.add(d)
    return args

def _cpreprocessor_uses_modules(children: [bool.type], pres: [[CPreprocessor.type], None]):
    if pres:
        for pre in pres:
            if pre.uses_modules:
                return True
    return any(children)

# Set of [CPreprocessor.type]. Most nodes have just a single value, but we
# allow > 1 for cxx compilation commands where it we do want > 1 (one for
# exported pp info and one for not-exported).
CPreprocessorTSet = transitive_set(
    args_projections = {
        "args": _cpreprocessor_args,
        "include_dirs": _cpreprocessor_include_dirs,
        "modular_args": _cpreprocessor_modular_args,
    },
    reductions = {
        "uses_modules": _cpreprocessor_uses_modules,
    },
)

CPreprocessorInfo = provider(fields = [
    "set",  # "CPreprocessorTSet"
])

# Defines the provider exposed by libraries to test targets,
# so that tests can have access to the private headers of
# the first order deps (for testing purposes).
CPreprocessorForTestsInfo = provider(fields = [
    # [str.type] - list of targets in "tests"
    "test_names",  #
    # CPreprocessor.type - the private preprocessor
    # for the target which is _only_ exposed to any
    # test targets defined in `test_names`
    "own_non_exported_preprocessor",
])

# Preprocessor flags
def cxx_attr_preprocessor_flags(ctx: "context", ext: str.type) -> [""]:
    return (
        ctx.attrs.preprocessor_flags +
        cxx_by_language_ext(ctx.attrs.lang_preprocessor_flags, ext) +
        flatten(cxx_by_platform(ctx, ctx.attrs.platform_preprocessor_flags)) +
        flatten(cxx_by_platform(ctx, cxx_by_language_ext(ctx.attrs.lang_platform_preprocessor_flags, ext)))
    )

def cxx_attr_exported_preprocessor_flags(ctx: "context") -> [""]:
    return (
        ctx.attrs.exported_preprocessor_flags +
        _by_language_cxx(ctx.attrs.exported_lang_preprocessor_flags) +
        flatten(cxx_by_platform(ctx, ctx.attrs.exported_platform_preprocessor_flags)) +
        flatten(cxx_by_platform(ctx, _by_language_cxx(ctx.attrs.exported_lang_platform_preprocessor_flags)))
    )

def cxx_inherited_preprocessor_infos(first_order_deps: ["dependency"]) -> [CPreprocessorInfo.type]:
    # We filter out nones because some non-cxx rule without such providers could be a dependency, for example
    # cxx_binary "fbcode//one_world/cli/util/process_wrapper:process_wrapper" depends on
    # python_library "fbcode//third-party-buck/$platform/build/glibc:__project__"
    return filter(None, [x.get(CPreprocessorInfo) for x in first_order_deps])

def cxx_merge_cpreprocessors(ctx: "context", own: [CPreprocessor.type], xs: [CPreprocessorInfo.type]) -> "CPreprocessorInfo":
    kwargs = {"children": [x.set for x in xs]}
    if own:
        kwargs["value"] = own
    return CPreprocessorInfo(
        set = ctx.actions.tset(CPreprocessorTSet, **kwargs),
    )

def cxx_exported_preprocessor_info(ctx: "context", headers_layout: CxxHeadersLayout.type, extra_preprocessors: [CPreprocessor.type] = []) -> CPreprocessor.type:
    """
    This rule's preprocessor info which is both applied to the compilation of
    its source and propagated to the compilation of dependent's sources.
    """

    # Modular libraries will provide their exported headers via a symlink tree
    # using extra_preprocessors, so should not be put into a header map.
    if getattr(ctx.attrs, "modular", False):
        exported_headers = []
    else:
        exported_headers = cxx_attr_exported_headers(ctx, headers_layout)

        # Add any headers passed in via constructor params
        for pre in extra_preprocessors:
            exported_headers += pre.headers

    exported_header_map = {
        paths.join(h.namespace, h.name): h.artifact
        for h in exported_headers
    }
    raw_headers = []
    include_dirs = []
    system_include_dirs = []

    style = cxx_attr_exported_header_style(ctx)

    # If headers-as-raw-headers is enabled, convert exported headers to raw
    # headers, with the appropriate include directories.
    raw_headers_mode = _attr_headers_as_raw_headers_mode(ctx)
    inferred_inc_dirs = as_raw_headers(ctx, exported_header_map, raw_headers_mode)
    if inferred_inc_dirs != None:
        raw_headers.extend(exported_header_map.values())
        if style == HeaderStyle("local"):
            include_dirs.extend(inferred_inc_dirs)
        else:
            system_include_dirs.extend(inferred_inc_dirs)
        exported_header_map.clear()

    # Add in raw headers and include dirs from attrs.
    raw_headers.extend(value_or(ctx.attrs.raw_headers, []))
    include_dirs.extend([ctx.label.path.add(x) for x in ctx.attrs.public_include_directories])
    system_include_dirs.extend([ctx.label.path.add(x) for x in ctx.attrs.public_system_include_directories])

    header_root = prepare_headers(ctx, exported_header_map, "buck-headers")

    # Process args to handle the `$(cxx-header-tree)` macro.
    args = []
    for arg in cxx_attr_exported_preprocessor_flags(ctx):
        if _needs_cxx_header_tree_hack(arg):
            if header_root == None or header_root.symlink_tree == None:
                fail("No headers")
            arg = _cxx_header_tree_hack_replacement(header_root.symlink_tree)
        args.append(arg)

    # Propagate the exported header tree.
    if header_root != None:
        inc_flag = _header_style_flag(style)
        args.extend([inc_flag, header_root.include_path])

    # Embed raw headers as hidden artifacts in our args.  This means downstream
    # cases which use these args don't also need to know to add raw headers.
    if raw_headers:
        # NOTE(agallagher): It's a bit weird adding an "empty" arg, but this
        # appears to do the job (and not e.g. expand to `""`).
        args.append(cmd_args().hidden(raw_headers))

    modular_args = []

    # Append any extra preprocessor info passed in via the constructor params
    for pre in extra_preprocessors:
        args.extend(pre.args)
        modular_args.extend(pre.modular_args)

    return CPreprocessor(
        args = args,
        headers = exported_headers,
        raw_headers = raw_headers,
        include_dirs = include_dirs,
        system_include_dirs = system_include_dirs,
        modular_args = modular_args,
    )

def cxx_private_preprocessor_info(
        ctx: "context",
        headers_layout: CxxHeadersLayout.type,
        raw_headers: ["artifact"] = [],
        extra_preprocessors: [CPreprocessor.type] = [],
        non_exported_deps: ["dependency"] = [],
        is_test: bool.type = False) -> (CPreprocessor.type, [CPreprocessor.type]):
    private_preprocessor = _cxx_private_preprocessor_info(ctx, headers_layout, raw_headers, extra_preprocessors)

    test_preprocessors = []
    if is_test:
        for non_exported_dep in non_exported_deps:
            preprocessor_for_tests = non_exported_dep.get(CPreprocessorForTestsInfo)
            if preprocessor_for_tests and ctx.label.name in preprocessor_for_tests.test_names:
                test_preprocessors.append(preprocessor_for_tests.own_non_exported_preprocessor)

    return (private_preprocessor, test_preprocessors)

def _cxx_private_preprocessor_info(
        ctx: "context",
        headers_layout: CxxHeadersLayout.type,
        raw_headers: ["artifact"],
        extra_preprocessors: [CPreprocessor.type]) -> CPreprocessor.type:
    """
    This rule's preprocessor info which is only applied to the compilation of
    its source, and not propagated to dependents.
    """

    headers = cxx_attr_headers(ctx, headers_layout)

    # `apple_*` rules allow headers to be included via only a basename if those
    # are headers (private or exported) from the same target.
    if headers_layout.naming == CxxHeadersNaming("apple"):
        headers.extend(
            _remap_headers_to_basename(
                headers + cxx_attr_exported_headers(ctx, headers_layout),
            ),
        )

    uses_modules = False

    # Include any headers provided via constructor params and determine whether
    # to use modules
    for pp in extra_preprocessors:
        headers += pp.headers
        uses_modules = uses_modules or pp.uses_modules

    header_map = {paths.join(h.namespace, h.name): h.artifact for h in headers}

    all_raw_headers = []
    include_dirs = []

    # If headers-as-raw-headers is enabled, convert exported headers to raw
    # headers, with the appropriate include directories.
    raw_headers_mode = _attr_headers_as_raw_headers_mode(ctx)
    inferred_inc_dirs = as_raw_headers(ctx, header_map, raw_headers_mode)
    if inferred_inc_dirs != None:
        all_raw_headers.extend(header_map.values())
        include_dirs.extend(inferred_inc_dirs)
        header_map.clear()

    # Add in raw headers and include dirs from attrs.
    all_raw_headers.extend(raw_headers)
    include_dirs.extend([ctx.label.path.add(x) for x in ctx.attrs.include_directories])

    # Create private header tree and propagate via args.
    args = []
    header_root = prepare_headers(ctx, header_map, "buck-private-headers")
    if header_root != None:
        args.extend(["-I", header_root.include_path])

    # Embed raw headers as hidden artifacts in our args.  This means downstream
    # cases which use these args don't also need to know to add raw headers.
    if all_raw_headers:
        # NOTE(agallagher): It's a bit weird adding an "empty" arg, but this
        # appears to do the job (and not e.g. expand to `""`).
        args.append(cmd_args().hidden(all_raw_headers))

    return CPreprocessor(
        args = args,
        headers = headers,
        raw_headers = all_raw_headers,
        include_dirs = include_dirs,
        uses_modules = uses_modules,
    )

def _by_language_cxx(x: {"": ""}) -> [""]:
    return cxx_by_language_ext(x, ".cpp")

def _header_style_flag(style: HeaderStyle.type) -> str.type:
    if style == HeaderStyle("local"):
        return "-I"
    if style == HeaderStyle("system"):
        return "-isystem"
    fail("unsupported header style: {}".format(style))

def _attr_headers_as_raw_headers_mode(ctx: "context") -> HeadersAsRawHeadersMode.type:
    """
    Return the `HeadersAsRawHeadersMode` setting to use for this rule.
    """

    mode = get_cxx_toolchain_info(ctx).headers_as_raw_headers_mode

    # If the platform hasn't set a raw headers translation mode, we don't do anything.
    if mode == None:
        return HeadersAsRawHeadersMode("disabled")

    # Otherwise use the rule-specific setting, if provided (not available on prebuilt_cxx_library).
    if getattr(ctx.attrs, "headers_as_raw_headers_mode", None) != None:
        return HeadersAsRawHeadersMode(ctx.attrs.headers_as_raw_headers_mode)

    # Fallback to platform default.
    return mode

def _needs_cxx_header_tree_hack(arg: "") -> bool.type:
    # The macro $(cxx-header-tree) is used in exactly once place, and its a place which isn't very
    # Buck v2 compatible. We replace $(cxx-header-tree) with HACK-CXX-HEADER-TREE at attribute time,
    # then here we substitute in the real header tree.
    return "HACK-CXX-HEADER-TREE" in repr(arg)

def _cxx_header_tree_hack_replacement(header_tree: "artifact") -> "cmd_args":
    # Unfortunately, we can't manipulate flags very precisely (for good reasons), so we rely on
    # knowing the form it takes.
    # The source is: -fmodule-map-file=$(cxx-header-tree)/module.modulemap
    return cmd_args(header_tree, format = "-fmodule-map-file={}/module.modulemap")

# Remap the given headers to be includable via their basenames (for use with
# "apple" style header naming).
def _remap_headers_to_basename(headers: [CHeader.type]) -> [CHeader.type]:
    remapped_headers = []
    for header in headers:
        if not header.named:
            remapped_headers.append(CHeader(
                artifact = header.artifact,
                name = paths.basename(header.name),
                namespace = "",
                named = False,
            ))
    return remapped_headers
