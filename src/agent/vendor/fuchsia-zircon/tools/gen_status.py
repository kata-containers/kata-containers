#!/usr/bin/env python

# Copyright 2016 The Fuchsia Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

# A tool for autogenerating the mapping between Status and zx_status_t
# Usage: python gen_status.py zircon/system/public/zircon/errors.h {sys,enum,match}
import re
import sys

status_re = re.compile('#define\s+(ZX_\w+)\s+\((\-?\d+)\)$')

def parse(in_filename):
    result = []
    for line in file(in_filename):
        m = status_re.match(line)
        if m:
            result.append((m.group(1), int(m.group(2))))
    return result

def to_snake_case(name):
    result = []
    for element in name.split('_'):
        result.append(element[0] + element[1:].lower())
    return ''.join(result)

def out(style, l):
    print('// Auto-generated using tools/gen_status.py')
    longest = max(len(name) for (name, num) in l)
    if style == 'sys':
        for (name, num) in l:
            print('pub const %s : zx_status_t = %d;' % (name.ljust(longest), num))
    if style == 'enum':
        print('pub enum Status {')
        for (name, num) in l:
            print('    %s = %d,' % (to_snake_case(name[3:]), num))
        print('');
        print('    /// Any zx_status_t not in the set above will map to the following:')
        print('    UnknownOther = -32768,')
        print('}')
    if style == 'match':
        for (name, num) in l:
            print('            sys::%s => Status::%s,' % (name, to_snake_case(name[3:])))
        print('            _ => Status::UnknownOther,')


l = parse(sys.argv[1])
out(sys.argv[2], l)
