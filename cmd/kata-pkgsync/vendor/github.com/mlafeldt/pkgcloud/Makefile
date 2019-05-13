.PHONY: all
all: pkgcloud-push build

build:
	go build

pkgcloud-push:
	go build ./cmd/$@

.PHONY: test
test:
	go test -v ./...

.PHONY: generate
generate:
	go generate -x ./...


.PHONY: clean
clean:
	rm -f pkgcloud-push
