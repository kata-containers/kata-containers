PREFIX := /usr
BIN_DIR := $(PREFIX)/bin
VC_BIN_DIR := $(BIN_DIR)/virtcontainers/bin
TEST_BIN_DIR := $(VC_BIN_DIR)/test
VIRTC_DIR := hack/virtc
VIRTC_BIN := virtc
HOOK_DIR := hook/mock
HOOK_BIN := hook
CC_SHIM_DIR := shim/mock/cc-shim
CC_SHIM_BIN := cc-shim
KATA_SHIM_DIR := shim/mock/kata-shim
KATA_SHIM_BIN := kata-shim

#
# Pretty printing
#

V	      = @
Q	      = $(V:1=)
QUIET_GOBUILD = $(Q:@=@echo    '     GOBUILD  '$@;)

#
# Build
#

all: build binaries

build:
	$(QUIET_GOBUILD)go build $(go list ./... | grep -v /vendor/)

virtc:
	$(QUIET_GOBUILD)go build -o $(VIRTC_DIR)/$@ $(VIRTC_DIR)/*.go

hook:
	$(QUIET_GOBUILD)go build -o $(HOOK_DIR)/$@ $(HOOK_DIR)/*.go

cc-shim:
	$(QUIET_GOBUILD)go build -o $(CC_SHIM_DIR)/$@ $(CC_SHIM_DIR)/*.go

kata-shim:
	$(QUIET_GOBUILD)go build -o $(KATA_SHIM_DIR)/$@ $(KATA_SHIM_DIR)/*.go

binaries: virtc hook cc-shim kata-shim

#
# Tests
#

check: check-go-static check-go-test

check-go-static:
	bash .ci/go-lint.sh

check-go-test:
	bash .ci/go-test.sh \
		$(TEST_BIN_DIR)/$(CC_SHIM_BIN) \
		$(TEST_BIN_DIR)/$(KATA_SHIM_BIN) \
		$(TEST_BIN_DIR)/$(HOOK_BIN)

#
# Install
#

define INSTALL_EXEC
	install -D $1 $(VC_BIN_DIR)/ || exit 1;
endef

define INSTALL_TEST_EXEC
	install -D $1 $(TEST_BIN_DIR)/ || exit 1;
endef

install:
	@mkdir -p $(VC_BIN_DIR)
	$(call INSTALL_EXEC,$(VIRTC_DIR)/$(VIRTC_BIN))
	@mkdir -p $(TEST_BIN_DIR)
	$(call INSTALL_TEST_EXEC,$(HOOK_DIR)/$(HOOK_BIN))
	$(call INSTALL_TEST_EXEC,$(CC_SHIM_DIR)/$(CC_SHIM_BIN))
	$(call INSTALL_TEST_EXEC,$(KATA_SHIM_DIR)/$(KATA_SHIM_BIN))

#
# Uninstall
#

define UNINSTALL_EXEC
	rm -f $(call FILE_SAFE_TO_REMOVE,$(VC_BIN_DIR)/$1) || exit 1;
endef

define UNINSTALL_TEST_EXEC
	rm -f $(call FILE_SAFE_TO_REMOVE,$(TEST_BIN_DIR)/$1) || exit 1;
endef

uninstall:
	$(call UNINSTALL_EXEC,$(VIRTC_BIN))
	$(call UNINSTALL_TEST_EXEC,$(HOOK_BIN))
	$(call UNINSTALL_TEST_EXEC,$(CC_SHIM_BIN))
	$(call UNINSTALL_TEST_EXEC,$(KATA_SHIM_BIN))

#
# Clean
#

# Input: filename to check.
# Output: filename, assuming the file exists and is safe to delete.
define FILE_SAFE_TO_REMOVE =
$(shell test -e "$(1)" && test "$(1)" != "/" && echo "$(1)")
endef

CLEAN_FILES += $(VIRTC_DIR)/$(VIRTC_BIN)
CLEAN_FILES += $(HOOK_DIR)/$(HOOK_BIN)
CLEAN_FILES += $(SHIM_DIR)/$(CC_SHIM_BIN)
CLEAN_FILES += $(SHIM_DIR)/$(KATA_SHIM_BIN)

clean:
	rm -f $(foreach f,$(CLEAN_FILES),$(call FILE_SAFE_TO_REMOVE,$(f)))

.PHONY: \
	all \
	build \
	virtc \
	hook \
	shim \
	binaries \
	check \
	check-go-static \
	check-go-test \
	install \
	uninstall \
	clean
