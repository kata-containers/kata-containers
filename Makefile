all:
	go build -o kata-shim

test: all
	go test -v -race

clean:
	rm -f kata-shim
