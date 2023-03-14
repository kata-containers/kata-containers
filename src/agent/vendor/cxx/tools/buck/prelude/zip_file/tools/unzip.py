# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

import argparse
import sys
import zipfile


def _parse_args():
    parser = argparse.ArgumentParser()
    parser.add_argument("--src", required=True, help="File to extract")
    parser.add_argument("--dst", required=True, help="Output directory")

    return parser.parse_args()


def do_unzip(src, dst):
    z = zipfile.ZipFile(src)
    z.extractall(dst)


def main():
    args = _parse_args()
    print("Source zip is: {}".format(args.src), file=sys.stderr)
    print("Output destination is: {}".format(args.dst), file=sys.stderr)
    do_unzip(args.src, args.dst)


if __name__ == "__main__":
    main()
