#!/bin/bash

tmp_dir=$(mktemp -d)
lower=$1
dest=$2

set -e

# Mount writable temp fs.
mkdir -p $tmp_dir/rw
mount -t tmpfs -o size=512K none $tmp_dir/rw

# Prepare directories in rw mount and mount overlay fs.
mkdir -p $tmp_dir/rw/overlay
mkdir -p $tmp_dir/rw/work
mount -t overlay none -o lowerdir=$lower,upperdir=$tmp_dir/rw/overlay,workdir=$tmp_dir/rw/work $dest

# Unmount temporary mount so that it's cleaned up automatically when the overlay is.
umount $tmp_dir/rw

# We don't need the temp dir anymore.
rm -rf $tmp_dir
