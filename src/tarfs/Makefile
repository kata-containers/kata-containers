ifneq ($(KERNELRELEASE),)

obj-m := tarfs.o

else

KDIR ?= /lib/modules/`uname -r`/build
INSTALL_MOD_PATH ?= $$PWD/_install

default:
	$(MAKE) -C $(KDIR) M=$$PWD

install:
	$(MAKE) -C $(KDIR) M=$$PWD INSTALL_MOD_PATH=$(INSTALL_MOD_PATH) modules_install

clean:
	rm -rf _install
	$(MAKE) -C $(KDIR) M=$$PWD clean


endif
