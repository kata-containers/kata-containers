#!/bin/bash

set -euo pipefail;

BUILD_DIR="${BUILD_DIR:-"build"}";
UPLOAD_FILES="${UPLOAD_FILES:-"rootfs.img OVMF.fd OVMF_AMD.fd root_hash.txt vmlinuz"}";
S3_BUCKET="${S3_BUCKET:-"builds-vm"}";
TAG="${TAG:-"build-0"}";

IFS=' ' read -r -a FILES <<< "${UPLOAD_FILES}"
JSON="{\n"

for FILE in "${FILES[@]}"; do
  if [ -f "$BUILD_DIR/$FILE" ]; then
    key=$FILE
    case $FILE in
      rootfs.img) key="rootfs" ;;
      OVMF.fd) key="bios" ;;
      OVMF_AMD.fd) key="bios_amd" ;;
      root_hash.txt) key="root_hash" ;;
      vmlinuz) key="kernel" ;;
    esac

    SHA256=$(sha256sum "$BUILD_DIR/$FILE" | awk '{print $1}')
    JSON+="  \"${key}\": {\n"
    JSON+="    \"bucket\": \"${S3_BUCKET}\",\n"
    JSON+="    \"prefix\": \"${TAG}\",\n"
    JSON+="    \"filename\": \"$FILE\",\n"
    JSON+="    \"sha256\": \"$SHA256\"\n"
    JSON+="  },\n"
  else
    echo "File ${BUILD_DIR}/${FILE} not found"
    exit 1
  fi
done

JSON="${JSON%,*}"
JSON+="\n}"
echo -e "$JSON" > "$BUILD_DIR/vm.json"
cat "$BUILD_DIR/vm.json";
