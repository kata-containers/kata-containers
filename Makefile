# Copyright (c) 2020 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

# List of available components
COMPONENTS =

COMPONENTS += agent

default: all

all: $(COMPONENTS)

test:
	bash ci/go-test.sh

# Create a set of rules for the specified component:
#
# - The component depends on its Makefile.
# - "build-$(component)" is an alias for "$(component)".
define make_component_rules
$(1) : src/$(1)/Makefile
	make -C src/$(1)
build-$(1) : $(1)

check-$(1) : src/$(1)/Makefile
	make -C src/$(1) check

clean-$(1) : src/$(1)/Makefile
	make -C src/$(1) clean

install-$(1) : $(1)
	make -C src/$(1) install

test-$(1) : $(1)
	make -C src/$(1) test

.PHONY: \
    $(1) \
    test-$(1) \
    install-$(1) \
    clean-$(1) \
    check-$(1) \
    build-$(1) \

endef

# Create the rules for the supported components.
$(foreach c,$(COMPONENTS),$(eval $(call make_component_rules,$(c))))

# Create a "${target}-all" alias which will cause each components
# rule to be called.
define make_all_rules
$(1)-all: $(foreach c,$(COMPONENTS),$(1)-$(c))

.PHONY: $(1) $(1)-all
endef

# Rules that support a "-all" suffix
ALL_RULES = build check clean install test

# Create the "-all" rules
$(foreach a,$(ALL_RULES),$(eval $(call make_all_rules,$(a))))

# Support "make ${target}"
# (which is an alias for "make ${target}-all").
$(ALL_RULES) : % : %-all

.PHONY: all default
