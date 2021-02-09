# Copyright 2019 fsyncd, Berlin, Germany.
# Additional material, copyright of the containerd authors.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.
 
TOOLCHAIN := x86_64-unknown-linux-musl
VPATH = target/system target/$(TOOLCHAIN)/debug target/$(TOOLCHAIN)/release

SRCS := $(shell find . -type f -name '*.rs' | grep -v 'tests')
ID := $(shell date +%s)

.PHONY: all check clean vsock kmod vm

all: vsock echo_server

check: vsock echo_server
	cargo fmt --all -- --check
	cargo clippy --all-targets --all-features -- -D warnings
	cargo test --all

clean:
	cargo clean

vsock: $(SRCS)
	cargo build --lib

echo_server: vsock echo_server/src/main.rs
	cargo build --manifest-path=echo_server/Cargo.toml

# Set up required host kernel modules
kmod:
	sudo /sbin/modprobe -r vmw_vsock_vmci_transport
	sudo /sbin/modprobe -r vmw_vsock_virtio_transport_common
	sudo /sbin/modprobe -r vsock
	sudo /sbin/modprobe vhost_vsock

# Start a virtio socket enabled vm
vm: initrd.cpio
	sudo qemu-system-x86_64 -kernel test_fixture/bzImage -initrd target/$(TOOLCHAIN)/debug/initrd.cpio \
		-enable-kvm -m 256 -device vhost-vsock-pci,id=vhost-vsock-pci0,guest-cid=3 -nographic -append "console=ttyS0"

# Create a simple operating system image for the vm
initrd.cpio: echo_server
	-rm -f target/$(TOOLCHAIN)/debug/initrd.cpio
	mkdir -p /tmp/$(ID)
	cp test_fixture/busybox.cpio /tmp/$(ID)/initrd.cpio
	cp test_fixture/init /tmp/$(ID)/init
	cp echo_server/target/$(TOOLCHAIN)/debug/echo_server /tmp/$(ID)/
	(cd '/tmp/$(ID)' && find . | grep -v 'initrd.cpio' | cpio -H newc -o --append -F initrd.cpio)
	mv /tmp/$(ID)/initrd.cpio target/$(TOOLCHAIN)/debug/
	rm -Rf /tmp/$(ID)