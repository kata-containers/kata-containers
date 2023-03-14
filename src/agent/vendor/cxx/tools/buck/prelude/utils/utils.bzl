# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

# General utilities shared between multiple rules.

def value_or(x: [None, "_a"], default: "_a") -> "_a":
    return default if x == None else x

# Flatten a list of lists into a list
def flatten(xss: [["_a"]]) -> ["_a"]:
    return [x for xs in xss for x in xs]

# Flatten a list of dicts into a dict
def flatten_dict(xss: [{"_a": "_b"}]) -> {"_a": "_b"}:
    return {k: v for xs in xss for k, v in xs.items()}

# Fail if given condition is not met.
def expect(x: bool.type, msg: str.type = "condition not expected", *fmt):
    if not x:
        fmt_msg = msg.format(*fmt)
        fail(fmt_msg)

def expect_non_none(val, msg: str.type = "unexpected none", *fmt_args, **fmt_kwargs):
    """
    Require the given value not be `None`.
    """
    if val == None:
        fail(msg.format(*fmt_args, **fmt_kwargs))
    return val

def from_named_set(srcs: [{str.type: ["artifact", "dependency"]}, [["artifact", "dependency"]]]) -> {str.type: ["artifact", "dependency"]}:
    """
    Normalize parameters of optionally named sources to a dictionary mapping
    names to sources, deriving the name from the short path when it's not
    explicitly provided.
    """

    if type(srcs) == type([]):
        srcs_dict = {}
        for src in srcs:
            if type(src) == "artifact":
                name = src.short_path
            else:
                # If the src is a `dependency`, use the short path of the
                # default output.
                expect(
                    len(src[DefaultInfo].default_outputs) == 1,
                    "expected exactly one default output from {} ({})"
                        .format(src, src[DefaultInfo].default_outputs),
                )
                [artifact] = src[DefaultInfo].default_outputs
                name = artifact.short_path
            srcs_dict[name] = src
        return srcs_dict
    else:
        return srcs

def map_idx(key: "_a", vals: ["_b"]) -> ["_c"]:
    return [x[key] for x in vals]

def filter_idx(key: "_a", vals: ["_b"]) -> ["_b"]:
    return [x for x in vals if key in x]

def filter_and_map_idx(key: "_a", vals: ["_b"]) -> ["_c"]:
    return [x[key] for x in vals if key in x]

def idx(x: ["_a", None], key: "_b") -> ["_c", None]:
    return x[key] if x != None else None

# TODO(T127134666) remove this once we have a native function that does this
def dedupe_by_value(vals: ["_a"]) -> ["_a"]:
    return {val: None for val in vals}.keys()
