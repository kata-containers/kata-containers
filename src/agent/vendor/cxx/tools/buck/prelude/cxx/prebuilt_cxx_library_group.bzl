# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

load(
    "@prelude//cxx:preprocessor.bzl",
    "CPreprocessor",
    "cxx_inherited_preprocessor_infos",
    "cxx_merge_cpreprocessors",
)
load(
    "@prelude//linking:link_groups.bzl",
    "merge_link_group_lib_info",
)
load(
    "@prelude//linking:link_info.bzl",
    "LinkInfo",
    "LinkInfos",
    "LinkStyle",
    "Linkage",
    "LinkedObject",
    "create_merged_link_info",
    "get_actual_link_style",
    "get_link_styles_for_linkage",
)
load(
    "@prelude//linking:linkable_graph.bzl",
    "create_linkable_graph",
    "create_linkable_graph_node",
    "create_linkable_node",
)
load(
    "@prelude//linking:shared_libraries.bzl",
    "SharedLibraryInfo",
    "create_shared_libraries",
    "merge_shared_libraries",
)
load("@prelude//utils:utils.bzl", "expect", "flatten_dict")
load(
    ":cxx_library_utility.bzl",
    "cxx_inherited_link_info",
)

def _linkage(ctx: "context") -> Linkage.type:
    """
    Construct the preferred linkage to use for the given prebuilt library.
    """

    # If we have both shared and static libs, we support any linkage.
    if (ctx.attrs.shared_link and
        (ctx.attrs.static_link or ctx.attrs.static_pic_link)):
        return Linkage("any")

    # Otherwise, if we have a shared library, we only support shared linkage.
    if ctx.attrs.shared_link:
        return Linkage("shared")

    # Otherwise, if we have a static library, we only support static linkage.
    if ctx.attrs.static_link or ctx.attrs.static_pic_link:
        return Linkage("static")

    # Otherwise, header only libs use any linkage.
    return Linkage("any")

def _parse_macro(
        arg: str.type) -> [(str.type, str.type, str.type), None]:
    """
    Parse a lib reference macro (e.g. `$(lib 0)`, `$(rel-lib libfoo.so)`) into
    the format string used to format the arg, the name of the macro parsed, and
    the argument passed to the macro.
    """

    # TODO(T110378124): This is obviously not ideal and longer-term we should
    # probably come up with a better UI for this rule or properly support these
    # macros.

    # If there's not macro, then there's nothing to do.
    if "$(" not in arg:
        return None

    # Extract the macro name and it's arg out of the string.  Also, create a
    # format string with the remaining parts which can be used to format to an
    # actual arg.  This is pretty ugly, but we don't have too complex a case to
    # support (e.g. a single macro with a single arg).
    start, rest = arg.split("$(")
    pos = rest.find(" ")
    macro = rest[:pos]
    rest = rest[pos + 1:]
    pos = rest.find(")")
    param = rest[:pos]
    end = rest[pos + 1:]
    return start + "{}" + end, macro, param

def _get_static_link_args(
        libs: ["artifact"],
        args: [str.type]) -> [""]:
    """
    Format a pair of static link string args and static libs into args to be
    passed to the link, by resolving macro references to libraries.
    """

    link = []

    for arg in args:
        res = _parse_macro(arg)
        if res != None:
            # Macros in the static link line are indexes to the list of static
            # archives.
            fmt, macro, param = res
            expect(macro == "lib")
            link.append(cmd_args(libs[int(param)], format = fmt))
        else:
            link.append(arg)

    return link

def _get_shared_link_args(
        shared_libs: {str.type: "artifact"},
        args: [str.type]) -> [""]:
    """
    Format a pair of shared link string args and shared libs into args to be
    passed to the link, by resolving macro references to libraries.
    """

    link = []

    for arg in args:
        res = _parse_macro(arg)
        if res != None:
            # Macros in the shared link line are named references to the map
            # of all shared libs.
            fmt, macro, lib_name = res
            expect(macro in ("lib", "rel-lib"))
            shared_lib = shared_libs[lib_name]
            if macro == "lib":
                link.append(cmd_args(shared_lib, format = fmt))
            elif macro == "rel-lib":
                # rel-lib means link-without-soname.
                link.append(cmd_args(shared_lib, format = "-L{}").parent())
                link.append("-l" + shared_lib.basename.removeprefix("lib").removesuffix(shared_lib.extension))
        else:
            link.append(arg)

    return link

# The `prebuilt_cxx_library_group` rule is meant to provide fine user control for
# how a group libraries of libraries are added to the link line and was added for
# `fbcode//third-party-buck/platform009/build/IntelComposerXE:mkl_lp64_iomp`, which
# includes libraries with dep cycles, and so must be linked together with flags
# like `--start-group`/`--end-group`.
#
# The link arguments for the various link styles are specified by pair of string
# arguments with macros referenceing a collection of libraries:
#
# - For static link styles, the string link args (e.g. specific in `static_link`)
#   contain macros of the form `$(lib <number>)`, where the number is an index
#   into the corresponding list of static libraries artifacts (e.g. specified in
#   `static_libs`).  For example:
#
#     static_link = ["-Wl,--start-group", "$(lib 0)", "$(lib 1)", "-Wl,--end-group"],
#     static_libs = ["libfoo1.a", "libfoo2.a"],
#
# - For shared linking, the string link args contain macros of the form
#   `$(lib <name>)` or `$(rel-lib <name>)`, where the name is key for shared
#   libraries specified in `shared_libs` or `provided_shared_libs`.  The
#   `lib` macro examples to the full path of the shared library, whereas the
#   `rel-lib` macro expands to `-L<dirname> -l<name>` of the library and is
#   meant to be used in situations where shared library does not contain an
#   embedded soname.  For example:
#
#     shared_link = ["$(lib libfoo1.so)", "$(rel-lib libfoo2.so)"],
#     shared_libs = {
#         "libfoo1.so": "lib/libfoo1.so",
#         "libfoo2.so": "lib/libfoo2.so",
#     },
#
def prebuilt_cxx_library_group_impl(ctx: "context") -> ["provider"]:
    providers = []

    deps = ctx.attrs.deps
    exported_deps = ctx.attrs.exported_deps

    # Figure out preprocessor stuff
    args = []
    args.extend(ctx.attrs.exported_preprocessor_flags)
    for inc_dir in ctx.attrs.include_dirs:
        args += ["-isystem", inc_dir]
    preprocessor = CPreprocessor(args = args)
    inherited_pp_info = cxx_inherited_preprocessor_infos(exported_deps)
    providers.append(cxx_merge_cpreprocessors(ctx, [preprocessor], inherited_pp_info))

    # Figure out all the link styles we'll be building archives/shlibs for.
    preferred_linkage = _linkage(ctx)

    inherited_non_exported_link = cxx_inherited_link_info(ctx, deps)
    inherited_exported_link = cxx_inherited_link_info(ctx, exported_deps)

    # Gather link infos, outputs, and shared libs for effective link style.
    outputs = {}
    libraries = {}
    solibs = {}
    for link_style in get_link_styles_for_linkage(preferred_linkage):
        outs = []
        if link_style == LinkStyle("static"):
            outs.extend(ctx.attrs.static_libs)
            args = _get_static_link_args(ctx.attrs.static_libs, ctx.attrs.static_link)
        elif link_style == LinkStyle("static_pic"):
            outs.extend(ctx.attrs.static_pic_libs)
            args = _get_static_link_args(ctx.attrs.static_pic_libs, ctx.attrs.static_pic_link)
        else:  # shared
            outs.extend(ctx.attrs.shared_libs.values())
            args = _get_shared_link_args(
                flatten_dict([ctx.attrs.shared_libs, ctx.attrs.provided_shared_libs]),
                ctx.attrs.shared_link,
            )
            solibs.update({n: LinkedObject(output = lib) for n, lib in ctx.attrs.shared_libs.items()})
        outputs[link_style] = outs

        # TODO(cjhopman): This is hiding static and shared libs in opaque
        # linker args, it should instead be constructing structured LinkInfo
        # instances
        libraries[link_style] = LinkInfos(default = LinkInfo(
            name = repr(ctx.label),
            pre_flags = args,
        ))

    # Collect per-link-style default outputs.
    default_outputs = {}
    for link_style in LinkStyle:
        actual_link_style = get_actual_link_style(link_style, preferred_linkage)
        default_outputs[link_style] = outputs[actual_link_style]
    providers.append(DefaultInfo(default_outputs = default_outputs[LinkStyle("static")]))

    # Provider for native link.
    providers.append(create_merged_link_info(
        ctx,
        libraries,
        preferred_linkage = preferred_linkage,
        # Export link info from our (non-exported) deps (e.g. when we're linking
        # statically).
        deps = [inherited_non_exported_link],
        # Export link info from our (exported) deps.
        exported_deps = [inherited_exported_link],
    ))

    # Propagate shared libraries up the tree.
    providers.append(merge_shared_libraries(
        ctx.actions,
        create_shared_libraries(ctx, solibs),
        filter(None, [x.get(SharedLibraryInfo) for x in deps + exported_deps]),
    ))

    # Create, augment and provide the linkable graph.
    linkable_graph = create_linkable_graph(
        ctx,
        node = create_linkable_graph_node(
            ctx,
            linkable_node = create_linkable_node(
                ctx = ctx,
                deps = deps,
                exported_deps = exported_deps,
                preferred_linkage = preferred_linkage,
                link_infos = libraries,
                shared_libs = solibs,
            ),
        ),
        deps = deps + exported_deps,
    )
    providers.append(linkable_graph)

    providers.append(merge_link_group_lib_info(deps = deps + exported_deps))

    return providers
