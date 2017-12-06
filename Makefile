all:
	go build -o kata-shim

test:
	go test -v -race

clean:
	rm -f kata-shim
