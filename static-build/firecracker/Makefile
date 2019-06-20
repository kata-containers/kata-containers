MK_DIR :=$(shell dirname $(realpath $(lastword $(MAKEFILE_LIST))))
CONFIG_DIR := $(MK_DIR)/../../scripts/

build:
	"$(MK_DIR)/build-static-firecracker.sh"

clean:
	rm -rf "$(MK_DIR)/firecracker"
	rm "$(MK_DIR)/firecracker-static"
	rm "$(MK_DIR)/jailer-static"
