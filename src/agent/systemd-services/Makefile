
.PHONY: systemd-install

PREFIX ?= /usr
BINDIR ?= $(PREFIX)/bin
SYSTEMD_DIR ?= /etc/systemd/system

systemd-install:
	install -D -m 644 guest-components.target $(DESTDIR)$(SYSTEMD_DIR)/guest-components.target
	install -D -m 644 guest-components-imagepull.service $(DESTDIR)$(SYSTEMD_DIR)/guest-components-imagepull.service
	install -D -m 644 guest-components-sealedsecrets.service $(DESTDIR)$(SYSTEMD_DIR)/guest-components-sealedsecrets.service
	install -D -m 644 guest-components-securemount.service $(DESTDIR)$(SYSTEMD_DIR)/guest-components-securemount.service
	install -D -m 644 guest-components-getresource.service $(DESTDIR)$(SYSTEMD_DIR)/guest-components-getresource.service