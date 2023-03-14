##
## Copyright 2017 Marc Stevens <marc@marc-stevens.nl>, Dan Shumow (danshu@microsoft.com)
## Distributed under the MIT Software License.
## See accompanying file LICENSE.txt or copy at
## https://opensource.org/licenses/MIT
##

# dynamic library compatibility
# 1. If the library source code has changed at all since the last update,
#    then increment revision (‘c:r:a’ becomes ‘c:r+1:a’).
# 2. If any interfaces have been added, removed, or changed since the last update,
#    increment current, and set revision to 0.
# 3. If any interfaces have been added since the last public release, then increment age.
# 4. If any interfaces have been removed or changed since the last public release,
#    then set age to 0.
LIBCOMPAT=1:0:0

PREFIX ?= /usr/local
BINDIR=$(PREFIX)/bin
LIBDIR=$(PREFIX)/lib
INCLUDEDIR=$(PREFIX)/include/sha1dc

CC ?= gcc
LD ?= gcc
CC_DEP ?= $(CC)

ifeq ($(shell uname),Darwin)
LIBTOOL ?= glibtool
INSTALL ?= install
else
LIBTOOL ?= libtool
INSTALL ?= install
endif


CFLAGS=-O2 -Wall -Werror -Wextra -pedantic -std=c90 -Ilib
LDFLAGS=

LT_CC:=$(LIBTOOL) --tag=CC --mode=compile $(CC)
LT_CC_DEP:=$(CC)
LT_LD:=$(LIBTOOL) --tag=CC --mode=link $(CC)
LT_INSTALL:=$(LIBTOOL) --tag=CC --mode=install $(INSTALL)

MKDIR=mkdir -p

ifneq (, $(shell which $(LIBTOOL) 2>/dev/null ))
CC:=$(LT_CC)
CC_DEP:=$(LT_CC_DEP)
LD:=$(LT_LD)
LDLIB:=$(LT_LD)
LIB_EXT:=la
else
LIB_EXT:=a
LD:=$(CC)
LT_INSTALL:=$(INSTALL)
endif

CFLAGS+=$(TARGETCFLAGS)
LDFLAGS+=$(TARGETLDFLAGS)


LIB_DIR=lib
LIB_DEP_DIR=dep_lib
LIB_OBJ_DIR=obj_lib
SRC_DIR=src
SRC_DEP_DIR=dep_src
SRC_OBJ_DIR=obj_src

H_DEP:=$(shell find . -type f -name "*.h")
FS_LIB=$(wildcard $(LIB_DIR)/*.c)
FS_SRC=$(wildcard $(SRC_DIR)/*.c)
FS_OBJ_LIB=$(FS_LIB:$(LIB_DIR)/%.c=$(LIB_OBJ_DIR)/%.lo)
FS_OBJ_SRC=$(FS_SRC:$(SRC_DIR)/%.c=$(SRC_OBJ_DIR)/%.lo)
FS_OBJ=$(FS_OBJ_SRC) $(FS_OBJ_LIB)
FS_DEP_LIB=$(FS_LIB:$(LIB_DIR)/%.c=$(LIB_DEP_DIR)/%.d)
FS_DEP_SRC=$(FS_SRC:$(SRC_DIR)/%.c=$(SRC_DEP_DIR)/%.d)
FS_DEP=$(FS_DEP_SRC) $(FS_DEP_LIB)

.SUFFIXES: .c .d

.PHONY: all
all: library tools

.PHONY: install
install: all
	$(LT_INSTALL) -d $(LIBDIR) $(BINDIR) $(INCLUDEDIR)
	$(LT_INSTALL) bin/libsha1detectcoll.$(LIB_EXT) $(LIBDIR)/libsha1detectcoll.$(LIB_EXT)
	$(LT_INSTALL) lib/sha1.h $(INCLUDEDIR)/sha1.h
	$(LT_INSTALL) bin/sha1dcsum $(BINDIR)/sha1dcsum
	$(LT_INSTALL) bin/sha1dcsum_partialcoll $(BINDIR)/sha1dcsum_partialcoll

.PHONY: uninstall
uninstall:
	-$(RM) $(BINDIR)/sha1dcsum
	-$(RM) $(BINDIR)/sha1dcsum_partialcoll
	-$(RM) $(INCLUDEDIR)/sha1.h
	-$(RM) $(LIBDIR)/libsha1detectcoll.$(LIB_EXT)

.PHONY: clean
clean:
	-find . -type f -name '*.a' -print -delete
	-find . -type f -name '*.d' -print -delete
	-find . -type f -name '*.o' -print -delete
	-find . -type f -name '*.la' -print -delete
	-find . -type f -name '*.lo' -print -delete
	-find . -type f -name '*.so' -print -delete
	-find . -type d -name '.libs' -print | xargs rm -rv
	-rm -rf bin

.PHONY: test
test: tools
	test e98a60b463a6868a6ce351ab0166c0af0c8c4721 != `bin/sha1dcsum test/sha1_reducedsha_coll.bin | cut -d' ' -f1` || (echo "\nError: Compiled for incorrect endianness" && false)
	test a56374e1cf4c3746499bc7c0acb39498ad2ee185 = `bin/sha1dcsum test/sha1_reducedsha_coll.bin | cut -d' ' -f1`
	test 16e96b70000dd1e7c85b8368ee197754400e58ec = `bin/sha1dcsum test/shattered-1.pdf | cut -d' ' -f1`
	test e1761773e6a35916d99f891b77663e6405313587 = `bin/sha1dcsum test/shattered-2.pdf | cut -d' ' -f1`
	test dd39885a2a5d8f59030b451e00cb45da9f9d3828 = `bin/sha1dcsum_partialcoll test/sha1_reducedsha_coll.bin | cut -d' ' -f1` 
	test d3a1d09969c3b57113fd17b23e01dd3de74a99bb = `bin/sha1dcsum_partialcoll test/shattered-1.pdf | cut -d' ' -f1`
	test 92246b0b718f4c704d37bb025717cbc66babf102 = `bin/sha1dcsum_partialcoll test/shattered-2.pdf | cut -d' ' -f1`
	bin/sha1dcsum test/*
	bin/sha1dcsum_partialcoll test/*
	
.PHONY: check
check: test

.PHONY: tools
tools: sha1dcsum sha1dcsum_partialcoll

.PHONY: sha1dcsum
sha1dcsum: bin/sha1dcsum

.PHONY: sha1dcsum_partialcoll
sha1dcsum_partialcoll: bin/sha1dcsum_partialcoll


.PHONY: library
library: bin/libsha1detectcoll.$(LIB_EXT)

bin/libsha1detectcoll.la: $(FS_OBJ_LIB)
	$(MKDIR) $(shell dirname $@) && $(LDLIB) $(LDFLAGS) $(FS_OBJ_LIB) -rpath $(LIBDIR) -version-info $(LIBCOMPAT) -o bin/libsha1detectcoll.la
	
bin/libsha1detectcoll.a: $(FS_OBJ_LIB)
	$(MKDIR) $(shell dirname $@) && $(AR) cru bin/libsha1detectcoll.a $(FS_OBJ_LIB)

bin/sha1dcsum: $(FS_OBJ_SRC) bin/libsha1detectcoll.$(LIB_EXT)
	$(LD) $(LDFLAGS) $(FS_OBJ_SRC) -Lbin -lsha1detectcoll -o bin/sha1dcsum

bin/sha1dcsum_partialcoll: $(FS_OBJ_SRC) bin/libsha1detectcoll.$(LIB_EXT)
	$(LD) $(LDFLAGS) $(FS_OBJ_SRC) -Lbin -lsha1detectcoll -o bin/sha1dcsum_partialcoll


$(SRC_DEP_DIR)/%.d: $(SRC_DIR)/%.c
	$(MKDIR) $(shell dirname $@) && $(CC_DEP) $(CFLAGS) -M -MF $@ $<

$(SRC_OBJ_DIR)/%.lo ${SRC_OBJ_DIR}/%.o: ${SRC_DIR}/%.c ${SRC_DEP_DIR}/%.d $(H_DEP)
	$(MKDIR) $(shell dirname $@) && $(CC) $(CFLAGS) -o $@ -c $<


$(LIB_DEP_DIR)/%.d: $(LIB_DIR)/%.c
	$(MKDIR) $(shell dirname $@) && $(CC_DEP) $(CFLAGS) -M -MF $@ $<

$(LIB_OBJ_DIR)/%.lo $(LIB_OBJ_DIR)/%.o: $(LIB_DIR)/%.c $(LIB_DEP_DIR)/%.d $(H_DEP)
	$(MKDIR) $(shell dirname $@) && $(CC) $(CFLAGS) -o $@ -c $<

-include $(FS_DEP)
