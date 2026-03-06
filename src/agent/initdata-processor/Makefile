.PHONY: all build test clean install systemd-install

PREFIX ?= /usr
BINDIR ?= $(PREFIX)/bin
SYSTEMD_DIR ?= /etc/systemd/system

all: build

build:
	CARGO_TARGET_DIR=./target cargo build --release

test:
	cargo test

clean:
	CARGO_TARGET_DIR=./target cargo clean

install: build
	install -D -m 755 target/release/initdata-processor $(DESTDIR)$(BINDIR)/initdata-processor

systemd-install:
	install -D -m 644 systemd-services/initdata-processor.service $(DESTDIR)$(SYSTEMD_DIR)/initdata-processor.service
