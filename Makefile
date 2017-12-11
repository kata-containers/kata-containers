all:
	go build -o kata-shim

test:
	go test -v -race -coverprofile=coverage.txt -covermode=atomic

clean:
	rm -f kata-shim
