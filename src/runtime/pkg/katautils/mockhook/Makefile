# Copyright Red Hat.
#
# SPDX-License-Identifier: Apache-2.0
#

BIN = hook
SRC = hook.go

V              = @
Q              = $(V:1=)
QUIET_BUILD    = $(Q:@=@echo    '     BUILD    '$@;)

BUILDFLAGS     =

all: $(BIN)

$(BIN): $(SRC)
	$(QUIET_BUILD)go build $(BUILDFLAGS) -o $@ $^

clean:
	rm -f $(BIN)
