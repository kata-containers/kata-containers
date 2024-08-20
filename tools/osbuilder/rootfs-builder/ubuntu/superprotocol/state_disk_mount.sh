#!/bin/bash
wipefs -a /dev/vdb
RANDOM_KEY=$(dd if=/dev/urandom bs=1 count=32 2>/dev/null | base64)
echo "\$RANDOM_KEY" | cryptsetup luksFormat /dev/vdb --batch-mode
echo "\$RANDOM_KEY" | cryptsetup luksOpen /dev/vdb crypto
mkfs.ext4 /dev/mapper/crypto