
VERSION_FILE := ../../../../VERSION

release:
	sed -i 's/appVersion: .*/appVersion: $(shell cat $(VERSION_FILE))/g' kata-deploy/Chart.yaml
all: package

package: helm release
	$(HELM) package ./kata-deploy

.PHONY: clean

clean:
	rm kata-deploy-*.tgz
