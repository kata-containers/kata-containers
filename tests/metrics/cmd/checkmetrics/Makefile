# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

TARGET     := checkmetrics
PREFIX     := /usr
BINDIR     := $(PREFIX)/bin
DESTTARGET := $(BINDIR)/$(TARGET)

all:
	go build -ldflags "-X main.sysBaseFile=$(DESTBASE)" -o $(TARGET)

install:
	install -D $(TARGET) $(DESTTARGET)

clean:
	rm -f $(DESTTARGET)

.PHONY: install clean
