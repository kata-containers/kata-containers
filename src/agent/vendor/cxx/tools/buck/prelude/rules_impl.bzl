# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

# Android
load("@prelude//android:android.bzl", _android_extra_attributes = "extra_attributes", _android_implemented_rules = "implemented_rules")
load("@prelude//android:configuration.bzl", "is_building_android_binary_attr")

# Apple
load("@prelude//apple:apple_rules_impl.bzl", _apple_extra_attributes = "extra_attributes", _apple_implemented_rules = "implemented_rules")

# Configuration
load("@prelude//configurations:rules.bzl", _config_extra_attributes = "extra_attributes", _config_implemented_rules = "implemented_rules")
load("@prelude//cxx:cxx.bzl", "cxx_binary_impl", "cxx_library_impl", "cxx_precompiled_header_impl", "cxx_test_impl", "prebuilt_cxx_library_impl")
load("@prelude//cxx:cxx_toolchain.bzl", "cxx_toolchain_impl")
load("@prelude//cxx:cxx_toolchain_types.bzl", "CxxPlatformInfo", "CxxToolchainInfo", "DistLtoToolsInfo")
load("@prelude//cxx:debug.bzl", "SplitDebugMode")

# C++
load("@prelude//cxx:headers.bzl", "CPrecompiledHeaderInfo")
load("@prelude//cxx:omnibus.bzl", "omnibus_environment_attr")
load("@prelude//cxx:prebuilt_cxx_library_group.bzl", "prebuilt_cxx_library_group_impl")
load("@prelude//cxx/user:link_group_map.bzl", "link_group_map_attr")
load("@prelude//go:cgo_library.bzl", "cgo_library_impl")
load("@prelude//go:coverage.bzl", "GoCoverageMode")

# Go
load("@prelude//go:go_binary.bzl", "go_binary_impl")
load("@prelude//go:go_library.bzl", "go_library_impl")
load("@prelude//go:go_test.bzl", "go_test_impl")
load("@prelude//go:toolchain.bzl", "GoToolchainInfo")

# Haskell
load("@prelude//haskell:haskell.bzl", "HaskellLibraryProvider", "HaskellPlatformInfo", "HaskellToolchainInfo", "haskell_binary_impl", "haskell_library_impl", "haskell_prebuilt_library_impl")
load("@prelude//haskell:haskell_ghci.bzl", "haskell_ghci_impl")
load("@prelude//haskell:haskell_haddock.bzl", "haskell_haddock_impl")
load("@prelude//haskell:haskell_ide.bzl", "haskell_ide_impl")

# Http archive
load("@prelude//http_archive:http_archive.bzl", "http_archive_impl")

# Java
load("@prelude//java:java.bzl", _java_extra_attributes = "extra_attributes", _java_implemented_rules = "implemented_rules")

# JavaScript
load("@prelude//js:js.bzl", _js_extra_attributes = "extra_attributes", _js_implemented_rules = "implemented_rules")

# Kotlin
load("@prelude//kotlin:kotlin.bzl", _kotlin_extra_attributes = "extra_attributes", _kotlin_implemented_rules = "implemented_rules")
load("@prelude//linking:link_info.bzl", "LinkOrdering")

# Lua
load("@prelude//lua:cxx_lua_extension.bzl", "cxx_lua_extension_impl")
load("@prelude//lua:lua_binary.bzl", "lua_binary_impl")
load("@prelude//lua:lua_library.bzl", "lua_library_impl")

# OCaml
load("@prelude//ocaml:ocaml.bzl", "ocaml_binary_impl", "ocaml_library_impl", "ocaml_object_impl", "prebuilt_ocaml_library_impl")
load("@prelude//ocaml:providers.bzl", "OCamlPlatformInfo", "OCamlToolchainInfo")

# Python
load("@prelude//python:cxx_python_extension.bzl", "cxx_python_extension_impl")
load("@prelude//python:prebuilt_python_library.bzl", "prebuilt_python_library_impl")
load("@prelude//python:python_binary.bzl", "python_binary_impl")
load("@prelude//python:python_library.bzl", "python_library_impl")
load("@prelude//python:python_needed_coverage_test.bzl", "python_needed_coverage_test_impl")
load("@prelude//python:python_test.bzl", "python_test_impl")
load("@prelude//python:toolchain.bzl", "PythonPlatformInfo", "PythonToolchainInfo")

# Python Bootstrap
load("@prelude//python_bootstrap:python_bootstrap.bzl", "PythonBootstrapSources", "PythonBootstrapToolchainInfo", "python_bootstrap_binary_impl", "python_bootstrap_library_impl")

# Rust
load("@prelude//rust:rust_binary.bzl", "rust_binary_impl", "rust_test_impl")
load("@prelude//rust:rust_library.bzl", "prebuilt_rust_library_impl", "rust_library_impl")
load("@prelude//rust:rust_toolchain.bzl", "RustPlatformInfo", "RustToolchainInfo")

# Zip file
load("@prelude//zip_file:zip_file.bzl", _zip_file_extra_attributes = "extra_attributes", _zip_file_implemented_rules = "implemented_rules")

# General
load(":alias.bzl", "alias_impl", "configured_alias_impl", "versioned_alias_impl")
load(":attributes.bzl", "IncludeType", "LinkableDepType", "Linkage", "Platform", "attributes")
load(":command_alias.bzl", "command_alias_impl")
load(":export_file.bzl", "export_file_impl")
load(":filegroup.bzl", "filegroup_impl")
load(":genrule.bzl", "genrule_attributes", "genrule_impl")
load(":http_file.bzl", "http_file_impl")
load(":remote_file.bzl", "remote_file_impl")
load(":sh_binary.bzl", "sh_binary_impl")
load(":sh_test.bzl", "sh_test_impl")
load(":test_suite.bzl", "test_suite_impl")

# Other
load(":worker_tool.bzl", "worker_tool")

def _merge_dictionaries(dicts):
    result = {}
    for d in dicts:
        for key, value in d.items():
            if key in result:
                fail("Duplicate key: '{}' while merging dictionaries".format(key))
            result[key] = value

    return result

implemented_rules = struct(
    #common rules
    alias = alias_impl,
    command_alias = command_alias_impl,
    configured_alias = configured_alias_impl,
    export_file = export_file_impl,
    filegroup = filegroup_impl,
    genrule = genrule_impl,
    http_archive = http_archive_impl,
    http_file = http_file_impl,
    remote_file = remote_file_impl,
    sh_binary = sh_binary_impl,
    sh_test = sh_test_impl,
    test_suite = test_suite_impl,
    versioned_alias = versioned_alias_impl,
    worker_tool = worker_tool,

    #c++
    cxx_binary = cxx_binary_impl,
    cxx_test = cxx_test_impl,
    cxx_toolchain = cxx_toolchain_impl,
    cxx_genrule = genrule_impl,
    cxx_library = cxx_library_impl,
    cxx_precompiled_header = cxx_precompiled_header_impl,
    cxx_python_extension = cxx_python_extension_impl,
    prebuilt_cxx_library = prebuilt_cxx_library_impl,
    prebuilt_cxx_library_group = prebuilt_cxx_library_group_impl,

    #go
    cgo_library = cgo_library_impl,
    go_binary = go_binary_impl,
    go_library = go_library_impl,
    go_test = go_test_impl,

    #haskell
    haskell_library = haskell_library_impl,
    haskell_binary = haskell_binary_impl,
    haskell_ghci = haskell_ghci_impl,
    haskell_haddock = haskell_haddock_impl,
    haskell_ide = haskell_ide_impl,
    haskell_prebuilt_library = haskell_prebuilt_library_impl,

    #lua
    cxx_lua_extension = cxx_lua_extension_impl,
    lua_binary = lua_binary_impl,
    lua_library = lua_library_impl,

    #ocaml
    ocaml_binary = ocaml_binary_impl,
    ocaml_object = ocaml_object_impl,
    ocaml_library = ocaml_library_impl,
    prebuilt_ocaml_library = prebuilt_ocaml_library_impl,

    #python
    prebuilt_python_library = prebuilt_python_library_impl,
    python_binary = python_binary_impl,
    python_library = python_library_impl,
    python_test = python_test_impl,
    python_needed_coverage_test = python_needed_coverage_test_impl,

    #python bootstrap
    python_bootstrap_binary = python_bootstrap_binary_impl,
    python_bootstrap_library = python_bootstrap_library_impl,

    #rust
    rust_binary = rust_binary_impl,
    rust_library = rust_library_impl,
    prebuilt_rust_library = prebuilt_rust_library_impl,
    rust_test = rust_test_impl,

    #merged **kwargs
    **_merge_dictionaries([
        _android_implemented_rules,
        _apple_implemented_rules,
        _config_implemented_rules,
        _java_implemented_rules,
        _js_implemented_rules,
        _kotlin_implemented_rules,
        _zip_file_implemented_rules,
    ])
)

def _cxx_python_extension_attrs():
    # cxx_python_extension is a subset of cxx_library, plus a base_module.
    # So we can reuse cxx_library, we augment it with the additional attributes it defines.
    # This isn't the ideal way to reuse it (we'd rather cxx_library was split it multiple reusable parts),
    # but it's the pragmatic way of getting it working for now.
    library = attributes["cxx_library"]
    me = attributes["cxx_python_extension"]
    res = {k: attrs.default_only(library[k]) for k in library if k not in me}
    res.update({
        "allow_embedding": attrs.bool(default = True),
        # Copied from cxx_library.
        "allow_huge_dwp": attrs.bool(default = False),
        "allow_suffixing": attrs.bool(default = True),
        "link_whole": attrs.default_only(attrs.bool(default = True)),
        "precompiled_header": attrs.option(attrs.dep(providers = [CPrecompiledHeaderInfo]), default = None),
        "preferred_linkage": attrs.default_only(attrs.string(default = "any")),
        "use_link_groups": attrs.bool(default = False),
        "_cxx_hacks": attrs.default_only(attrs.dep(default = "prelude//cxx/tools:cxx_hacks")),
        "_cxx_toolchain": _cxx_toolchain(),
        "_omnibus_environment": omnibus_environment_attr(),
        # Copied from python_library.
        "_python_toolchain": _python_toolchain(),
    })
    return res

def _python_test_attrs():
    return {
        "allow_huge_dwp": attrs.bool(default = False),
        "bundled_runtime": attrs.bool(default = False),
        "enable_distributed_thinlto": attrs.bool(default = False),
        "package_split_dwarf_dwp": attrs.bool(default = False),
        "remote_execution": attrs.option(attrs.dict(key = attrs.string(), value = attrs.string(), sorted = False)),
        "resources": attrs.named_set(attrs.one_of(attrs.dep(), attrs.source(allow_directory = True)), sorted = True, default = []),
        "_create_manifest_for_source_dir": _create_manifest_for_source_dir(),
        "_cxx_hacks": attrs.default_only(attrs.dep(default = "prelude//cxx/tools:cxx_hacks")),
        "_cxx_toolchain": _cxx_toolchain(),
        "_omnibus_environment": omnibus_environment_attr(),
        "_python_toolchain": _python_toolchain(),
        "_target_os_type": _target_os_type(),
        "_test_main": attrs.source(default = "prelude//python/tools:__test_main__.py"),
    }

def _cxx_binary_and_test_attrs():
    return {
        "allow_huge_dwp": attrs.bool(default = False),
        "auto_link_groups": attrs.bool(default = False),
        # Linker flags that only apply to the executable link, used for link
        # strategies (e.g. link groups) which may link shared libraries from
        # top-level binary context.
        "binary_linker_flags": attrs.list(attrs.arg(), default = []),
        "bolt_flags": attrs.list(attrs.arg(), default = []),
        "bolt_gdb_index": attrs.option(attrs.source(), default = None),
        "bolt_profile": attrs.option(attrs.source(), default = None),
        "enable_distributed_thinlto": attrs.bool(default = False),
        "link_group_map": link_group_map_attr(),
        "link_whole": attrs.default_only(attrs.bool(default = False)),
        "precompiled_header": attrs.option(attrs.dep(providers = [CPrecompiledHeaderInfo]), default = None),
        "resources": attrs.named_set(attrs.one_of(attrs.dep(), attrs.source(allow_directory = True)), sorted = True, default = []),
        "use_link_groups": attrs.bool(default = False),
        "_cxx_hacks": attrs.dep(default = "prelude//cxx/tools:cxx_hacks"),
        "_cxx_toolchain": _cxx_toolchain(),
    }

NativeLinkStrategy = ["separate", "native", "merged"]

def _package_python_binary_remotely():
    return select({
        "DEFAULT": False,
        "ovr_config//os/constraints:android": True,
    })

def _python_binary_attrs():
    cxx_binary_attrs = {k: v for k, v in attributes["cxx_binary"].items()}
    cxx_binary_attrs.update(_cxx_binary_and_test_attrs())
    python_binary_attrs = attributes["python_binary"]
    updated_attrs = {k: attrs.default_only(cxx_binary_attrs[k]) for k in cxx_binary_attrs if k not in python_binary_attrs}

    # allow non-default value for the args below
    updated_attrs.update({
        "allow_huge_dwp": attrs.bool(default = False),
        "auto_link_groups": attrs.bool(default = False),
        "bundled_runtime": attrs.bool(default = False),
        "cxx_main": attrs.option(attrs.source(), default = None),
        "enable_distributed_thinlto": attrs.bool(default = False),
        "executable_deps": attrs.list(attrs.dep(), default = []),
        "executable_name": attrs.option(attrs.string(), default = None),
        "link_group_map": link_group_map_attr(),
        "link_style": attrs.enum(LinkableDepType, default = "static"),
        "native_link_strategy": attrs.option(attrs.enum(NativeLinkStrategy), default = None),
        "package_split_dwarf_dwp": attrs.bool(default = False),
        "par_style": attrs.option(attrs.string(), default = None),
        "use_link_groups": attrs.bool(default = False),
        "_create_manifest_for_source_dir": _create_manifest_for_source_dir(),
        "_cxx_hacks": attrs.default_only(attrs.dep(default = "prelude//cxx/tools:cxx_hacks")),
        "_cxx_toolchain": _cxx_toolchain(),
        "_omnibus_environment": omnibus_environment_attr(),
        "_package_remotely": attrs.bool(default = _package_python_binary_remotely()),
        "_python_toolchain": _python_toolchain(),
        "_target_os_type": _target_os_type(),
    })
    return updated_attrs

def _toolchain(lang: str.type, providers: [""]) -> "attribute":
    return attrs.default_only(attrs.toolchain_dep(default = "toolchains//:" + lang, providers = providers))

def _cxx_toolchain():
    return _toolchain("cxx", [CxxToolchainInfo, CxxPlatformInfo])

def _haskell_toolchain():
    return _toolchain("haskell", [HaskellToolchainInfo, HaskellPlatformInfo])

def _rust_toolchain():
    return _toolchain("rust", [RustToolchainInfo, RustPlatformInfo])

def _go_toolchain():
    return _toolchain("go", [GoToolchainInfo])

def _ocaml_toolchain():
    return _toolchain("ocaml", [OCamlToolchainInfo, OCamlPlatformInfo])

def _python_toolchain():
    return _toolchain("python", [PythonToolchainInfo, PythonPlatformInfo])

def _python_bootstrap_toolchain():
    return _toolchain("python_bootstrap", [PythonBootstrapToolchainInfo])

def _target_os_type() -> "attribute":
    # FIXME: prelude// should be standalone (not refer to ovr_config//)
    return attrs.enum(Platform, default = select({
        "DEFAULT": "linux",
        "ovr_config//os:macos": "macos",
        "ovr_config//os:windows": "windows",
    }))

def _create_manifest_for_source_dir():
    return attrs.exec_dep(default = "prelude//python/tools:create_manifest_for_source_dir")

inlined_extra_attributes = {

    # go
    "cgo_library": {
        "_cxx_toolchain": _cxx_toolchain(),
        "_go_toolchain": _go_toolchain(),
    },
    "command_alias": {
        "_find_and_replace_bat": attrs.default_only(attrs.exec_dep(default = "prelude//tools:find_and_replace.bat")),
        "_target_os_type": _target_os_type(),
    },
    # The 'actual' attribute of configured_alias is a configured_label, which is
    # currently unimplemented. Map it to dep so we can simply forward the providers.
    "configured_alias": {
        # TODO(nga): "actual" attribute exists here only to display it in query,
        #   actual `actual` attribute used in rule implementation is named `configured_actual`.
        #   Logically this should be `attrs.configuration_label`, but `configuration_label`
        #   is currently an alias for `attrs.dep`, which makes non-transitioned dependency
        #   also a dependency along with transitioned dependency. (See D40255132).
        "actual": attrs.label(),
        # We use a separate field instead of re-purposing `actual`, as we want
        # to keep output format compatibility with v1.
        "configured_actual": attrs.option(attrs.configured_dep()),
        # If `configured_actual` is `None`, fallback to this unconfigured dep.
        "fallback_actual": attrs.option(attrs.dep()),
        "platform": attrs.option(attrs.configuration_label()),
    },
    "cxx_binary": _cxx_binary_and_test_attrs(),

    #c++
    "cxx_genrule": genrule_attributes() | {
        "_cxx_toolchain": _cxx_toolchain(),
        "_target_os_type": _target_os_type(),
    },
    "cxx_library": {
        "allow_huge_dwp": attrs.bool(default = False),
        "deps_query": attrs.option(attrs.query(), default = None),
        "extra_xcode_sources": attrs.list(attrs.source(allow_directory = True), default = []),
        "force_emit_omnibus_shared_root": attrs.bool(default = False),
        "link_deps_query_whole": attrs.bool(default = False),
        "link_group_map": link_group_map_attr(),
        "precompiled_header": attrs.option(attrs.dep(providers = [CPrecompiledHeaderInfo]), default = None),
        "prefer_stripped_objects": attrs.bool(default = False),
        "preferred_linkage": attrs.enum(Linkage, default = "any"),
        "resources": attrs.named_set(attrs.one_of(attrs.dep(), attrs.source(allow_directory = True)), sorted = True, default = []),
        "supports_python_dlopen": attrs.option(attrs.bool(), default = None),
        "use_link_groups": attrs.bool(default = False),
        "_cxx_hacks": attrs.default_only(attrs.dep(default = "prelude//cxx/tools:cxx_hacks")),
        "_cxx_toolchain": _cxx_toolchain(),
        "_is_building_android_binary": is_building_android_binary_attr(),
        "_omnibus_environment": omnibus_environment_attr(),
    },
    "cxx_python_extension": _cxx_python_extension_attrs(),
    "cxx_test": dict(
        remote_execution = attrs.option(attrs.dict(key = attrs.string(), value = attrs.string(), sorted = False)),
        **_cxx_binary_and_test_attrs()
    ),
    "cxx_toolchain": {
        "archiver": attrs.dep(providers = [RunInfo]),
        "archiver_supports_argfiles": attrs.bool(default = False),
        "asm_compiler": attrs.option(attrs.dep(providers = [RunInfo]), default = None),
        "asm_preprocessor": attrs.option(attrs.dep(providers = [RunInfo]), default = None),
        "assembler": attrs.dep(providers = [RunInfo]),
        "assembler_preprocessor": attrs.option(attrs.dep(providers = [RunInfo]), default = None),
        "bolt_enabled": attrs.bool(default = False),
        "c_compiler": attrs.dep(providers = [RunInfo]),
        "cxx_compiler": attrs.dep(providers = [RunInfo]),
        "link_ordering": attrs.enum(LinkOrdering.values(), default = "preorder"),
        "linker": attrs.dep(providers = [RunInfo]),
        "nm": attrs.dep(providers = [RunInfo]),
        "objcopy_for_shared_library_interface": attrs.dep(providers = [RunInfo]),
        # Used for resolving any 'platform_*' attributes.
        "platform_name": attrs.option(attrs.string(), default = None),
        "private_headers_symlinks_enabled": attrs.bool(default = True),
        "public_headers_symlinks_enabled": attrs.bool(default = True),
        "ranlib": attrs.option(attrs.dep(providers = [RunInfo]), default = None),
        "requires_objects": attrs.bool(default = False),
        "split_debug_mode": attrs.enum(SplitDebugMode.values(), default = "none"),
        "strip": attrs.dep(providers = [RunInfo]),
        "supports_distributed_thinlto": attrs.bool(default = False),
        "use_archiver_flags": attrs.bool(default = True),
        "use_dep_files": attrs.option(attrs.bool(), default = None),
        "_dep_files_processor": attrs.dep(providers = [RunInfo], default = "prelude//cxx/tools:makefile_to_dep_file"),
        "_dist_lto_tools": attrs.default_only(attrs.dep(providers = [DistLtoToolsInfo], default = "prelude//cxx/dist_lto/tools:dist_lto_tools")),
        "_mk_comp_db": attrs.default_only(attrs.dep(providers = [RunInfo], default = "prelude//cxx/tools:make_comp_db")),
        # FIXME: prelude// should be standalone (not refer to fbsource//)
        "_mk_hmap": attrs.default_only(attrs.dep(providers = [RunInfo], default = "fbsource//xplat/buck2/tools/cxx:hmap_wrapper")),
    },
    "export_file": {
        "src": attrs.source(allow_directory = True),
    },
    "filegroup": {
        "srcs": attrs.named_set(attrs.source(allow_directory = True), sorted = False, default = []),
    },
    "genrule": genrule_attributes() | {
        "env": attrs.dict(key = attrs.string(), value = attrs.arg(), sorted = False, default = {}),
        "srcs": attrs.named_set(attrs.source(allow_directory = True), sorted = False, default = []),
        "_target_os_type": _target_os_type(),
    },
    "go_binary": {
        "resources": attrs.list(attrs.one_of(attrs.dep(), attrs.source(allow_directory = True)), default = []),
        "_go_toolchain": _go_toolchain(),
    },
    "go_library": {
        "_go_toolchain": _go_toolchain(),
    },
    "go_test": {
        "coverage_mode": attrs.option(attrs.enum(GoCoverageMode.values()), default = None),
        "resources": attrs.list(attrs.source(allow_directory = True), default = []),
        "_go_toolchain": _go_toolchain(),
        "_testmaingen": attrs.default_only(attrs.exec_dep(default = "prelude//go/tools:testmaingen")),
    },

    # groovy
    "groovy_library": {
        "resources_root": attrs.option(attrs.string(), default = None),
    },
    "groovy_test": {
        "resources_root": attrs.option(attrs.string(), default = None),
    },
    "haskell_binary": {
        "template_deps": attrs.list(attrs.exec_dep(providers = [HaskellLibraryProvider]), default = []),
        "_cxx_toolchain": _cxx_toolchain(),
        "_haskell_toolchain": _haskell_toolchain(),
    },
    "haskell_library": {
        "preferred_linkage": attrs.enum(Linkage, default = "any"),
        "template_deps": attrs.list(attrs.exec_dep(providers = [HaskellLibraryProvider]), default = []),
        "_cxx_toolchain": _cxx_toolchain(),
        "_haskell_toolchain": _haskell_toolchain(),
    },

    # http things get only 1 hash in v1 but in v2 we allow multiple. Also,
    # don't default hashes to empty strings.
    "http_archive": {
        "sha1": attrs.option(attrs.string()),
        "sha256": attrs.option(attrs.string()),
        "_create_exclusion_list": attrs.default_only(attrs.exec_dep(default = "prelude//http_archive/tools:create_exclusion_list")),
    },
    "http_file": {
        "sha1": attrs.option(attrs.string()),
        "sha256": attrs.option(attrs.string()),
    },

    #ocaml
    "ocaml_binary": {
        "_cxx_toolchain": _cxx_toolchain(),
        "_ocaml_toolchain": _ocaml_toolchain(),
    },
    "ocaml_library": {
        "_cxx_toolchain": _cxx_toolchain(),
        "_ocaml_toolchain": _ocaml_toolchain(),
    },
    "ocaml_object": {
        "bytecode_only": attrs.option(attrs.bool(), default = None),
        "compiler_flags": attrs.list(attrs.arg(), default = []),
        "contacts": attrs.list(attrs.string(), default = []),
        "default_host_platform": attrs.option(attrs.configuration_label(), default = None),
        "deps": attrs.list(attrs.dep(), default = []),
        "labels": attrs.list(attrs.string(), default = []),
        "licenses": attrs.list(attrs.source(), default = []),
        "linker_flags": attrs.list(attrs.string(), default = []),
        "ocamldep_flags": attrs.list(attrs.arg(), default = []),
        "platform": attrs.option(attrs.string(), default = None),
        "platform_deps": attrs.list(attrs.tuple(attrs.regex(), attrs.set(attrs.dep(), sorted = True)), default = []),
        "platform_linker_flags": attrs.list(attrs.tuple(attrs.regex(), attrs.list(attrs.string())), default = []),
        "srcs": attrs.option(attrs.named_set(attrs.source(), sorted = False), default = None),
        "warnings_flags": attrs.option(attrs.string(), default = None),
        "within_view": attrs.option(attrs.list(attrs.string())),
        "_cxx_toolchain": _cxx_toolchain(),
        "_ocaml_toolchain": _ocaml_toolchain(),
    },
    "prebuilt_cxx_library": {
        "exported_header_style": attrs.enum(IncludeType, default = "system"),
        "header_dirs": attrs.option(attrs.list(attrs.source(allow_directory = True)), default = None),
        "platform_header_dirs": attrs.option(attrs.list(attrs.tuple(attrs.regex(), attrs.list(attrs.source(allow_directory = True)))), default = None),
        "preferred_linkage": attrs.enum(Linkage, default = "any"),
        "public_include_directories": attrs.set(attrs.string(), sorted = True, default = []),
        "public_system_include_directories": attrs.set(attrs.string(), sorted = True, default = []),
        "raw_headers": attrs.set(attrs.source(), sorted = True, default = []),
        "supports_python_dlopen": attrs.option(attrs.bool(), default = None),
        "versioned_header_dirs": attrs.option(attrs.versioned(attrs.list(attrs.source(allow_directory = True))), default = None),
        "_cxx_toolchain": _cxx_toolchain(),
        "_omnibus_environment": omnibus_environment_attr(),
    },
    "prebuilt_ocaml_library": {

        # These fields in 'attributes.bzl' are wrong.
        #
        # There they are defined in terms of `attrs.string()`. This
        # block overrides/corrects them here so as to be in terms of
        # `attrs.source()`.
        "bytecode_c_libs": attrs.list(attrs.source(), default = []),
        "bytecode_lib": attrs.option(attrs.source()),
        "c_libs": attrs.list(attrs.source(), default = []),
        "include_dir": attrs.option(attrs.source(allow_directory = True)),
        "lib_dir": attrs.option(attrs.source(allow_directory = True)),
        "native_c_libs": attrs.list(attrs.source(), default = []),
        "native_lib": attrs.option(attrs.source()),
    },

    #python
    "prebuilt_python_library": {
        "_create_manifest_for_source_dir": _create_manifest_for_source_dir(),
        "_extract": attrs.default_only(attrs.exec_dep(default = "prelude//python/tools:extract")),
        "_python_toolchain": _python_toolchain(),
    },
    "prebuilt_rust_library": {
        "_cxx_toolchain": _cxx_toolchain(),
        "_rust_toolchain": _rust_toolchain(),
    },
    "python_binary": _python_binary_attrs(),
    #python bootstrap
    "python_bootstrap_binary": {
        "deps": attrs.list(attrs.dep(providers = [PythonBootstrapSources]), default = []),
        "main": attrs.source(),
        "_python_bootstrap_toolchain": _python_bootstrap_toolchain(),
        "_target_os_type": _target_os_type(),
        "_win_python_wrapper": attrs.default_only(attrs.exec_dep(default = "prelude//python_bootstrap/tools:win_python_wrapper")),
    },
    "python_bootstrap_library": {
        "srcs": attrs.list(attrs.source()),
    },
    "python_library": {
        "allow_huge_dwp": attrs.bool(default = False),
        "resources": attrs.named_set(attrs.one_of(attrs.dep(), attrs.source(allow_directory = True)), sorted = True, default = []),
        "_create_manifest_for_source_dir": _create_manifest_for_source_dir(),
        "_cxx_toolchain": _cxx_toolchain(),
        "_python_toolchain": _python_toolchain(),
    },
    "python_needed_coverage_test": {
        "contacts": attrs.list(attrs.string(), default = []),
        "env": attrs.dict(key = attrs.string(), value = attrs.arg(), sorted = False, default = {}),
        "labels": attrs.list(attrs.string(), default = []),
        "needed_coverage": attrs.list(attrs.tuple(attrs.int(), attrs.dep(), attrs.option(attrs.string())), default = []),
        "remote_execution": attrs.option(attrs.dict(key = attrs.string(), value = attrs.string(), sorted = False)),
        "test": attrs.dep(providers = [ExternalRunnerTestInfo]),
    },
    "python_test": _python_test_attrs(),
    "remote_file": {
        "sha1": attrs.option(attrs.string()),
        "sha256": attrs.option(attrs.string()),
        "_unzip_tool": attrs.default_only(attrs.exec_dep(providers = [RunInfo], default = "prelude//zip_file/tools:unzip")),
    },
    #rust
    "rust_binary": {
        "incremental_build_mode": attrs.option(attrs.string()),
        "incremental_enabled": attrs.bool(default = False),
        "resources": attrs.named_set(attrs.one_of(attrs.dep(), attrs.source()), sorted = True, default = []),
        "_cxx_toolchain": _cxx_toolchain(),
        "_rust_toolchain": _rust_toolchain(),
    },
    "rust_library": {
        # linker_flags weren't supported for rust_library in Buck v1 but the
        # fbcode macros pass them anyway. They're typically empty since the
        # config-level flags don't get injected, but it doesn't hurt to accept
        # them and it simplifies the implementation of Rust rules since they
        # don't have to know whether we're building a rust_binary or a
        # rust_library.
        "incremental_build_mode": attrs.option(attrs.string()),
        "incremental_enabled": attrs.bool(default = False),
        "linker_flags": attrs.list(attrs.arg(), default = []),
        "preferred_linkage": attrs.enum(Linkage, default = "any"),
        "resources": attrs.named_set(attrs.one_of(attrs.dep(), attrs.source()), sorted = True, default = []),
        "supports_python_dlopen": attrs.option(attrs.bool(), default = None),
        "_cxx_toolchain": _cxx_toolchain(),
        "_omnibus_environment": omnibus_environment_attr(),
        "_rust_toolchain": _rust_toolchain(),
    },
    "rust_test": {
        "framework": attrs.bool(default = True),
        "incremental_build_mode": attrs.option(attrs.string()),
        "incremental_enabled": attrs.bool(default = False),
        "remote_execution": attrs.option(attrs.dict(key = attrs.string(), value = attrs.string(), sorted = False)),
        "resources": attrs.named_set(attrs.one_of(attrs.dep(), attrs.source()), sorted = True, default = []),
        "_cxx_toolchain": _cxx_toolchain(),
        "_rust_toolchain": _rust_toolchain(),
    },

    # scala
    "scala_library": {
        "resources_root": attrs.option(attrs.string(), default = None),
    },
    "scala_test": {
        "resources_root": attrs.option(attrs.string(), default = None),
    },
    "sh_binary": {
        "resources": attrs.list(attrs.source(allow_directory = True), default = []),
        "_target_os_type": _target_os_type(),
    },
    "sh_test": {
        "list_args": attrs.option(attrs.list(attrs.string()), default = None),
        "list_env": attrs.option(attrs.dict(key = attrs.string(), value = attrs.string(), sorted = False), default = None),
        "run_args": attrs.list(attrs.string(), default = []),
        "run_env": attrs.dict(key = attrs.string(), value = attrs.string(), sorted = False, default = {}),
        "test": attrs.option(attrs.one_of(attrs.dep(), attrs.source()), default = None),
    },
    "test_suite": {
        # On buck1 query, tests attribute on test_suite is treated as deps, while on buck2 it is not.
        # While buck2's behavior makes more sense, we want to preserve buck1 behavior on test_suite for now to make TD behavior match between buck1 and buck2.
        # This diff makes the behaviors match by adding a test_deps attribute to test_suite on buck2 that is used as a deps attribute. In the macro layer, we set test_deps = tests if we are using buck2.
        # For more context: https://fb.prod.workplace.com/groups/603286664133355/posts/682567096205311/?comment_id=682623719532982&reply_comment_id=682650609530293
        "test_deps": attrs.list(attrs.dep(), default = []),
    },
    "worker_tool": {
        # overridden to handle buck1's use of @Value.Default
        "args": attrs.one_of(attrs.arg(), attrs.list(attrs.arg()), default = []),
        # FIXME: prelude// should be standalone (not refer to fbsource//)
        "_worker_tool_runner": attrs.default_only(attrs.dep(default = "fbsource//xplat/buck2/tools/worker:worker_tool_runner")),
    },
}

all_extra_attributes = _merge_dictionaries([
    inlined_extra_attributes,
    _android_extra_attributes,
    _apple_extra_attributes,
    _config_extra_attributes,
    _java_extra_attributes,
    _js_extra_attributes,
    _kotlin_extra_attributes,
    _zip_file_extra_attributes,
])

# Inject test toolchain in all tests.

for rule in [
    "sh_test",
    "rust_test",
    "python_test",
    "python_needed_coverage_test",
    "java_test",
    "go_test",
    "cxx_test",
    "apple_test",
    "android_instrumentation_test",
    "kotlin_test",
    "robolectric_test",
]:
    # NOTE: We make this a `dep` not an `exec_dep` even though we'll execute
    # it, because it needs to execute in the same platform as the test itself
    # (we run tests in the target platform not the exec platform, since the
    # goal is to test the code that is being built!).
    all_extra_attributes[rule] = _merge_dictionaries([all_extra_attributes[rule], {
        "_inject_test_env": attrs.default_only(attrs.dep(default = "prelude//test/tools:inject_test_env")),
    }])

extra_attributes = struct(**all_extra_attributes)
