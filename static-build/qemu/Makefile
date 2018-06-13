MK_DIR :=$(shell dirname $(realpath $(lastword $(MAKEFILE_LIST))))
CONFIG_DIR := $(MK_DIR)/../../scripts/

build:
	"$(MK_DIR)/build-static-qemu.sh"

clean:
	rm -f kata-qemu-static.tar.gz
