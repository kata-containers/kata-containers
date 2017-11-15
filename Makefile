all:
	go build proxy.go
	make -C test

clean:
	rm -f proxy
	make -C test clean
