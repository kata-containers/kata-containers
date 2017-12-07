TARGET = kata-shim

$(TARGET):
	go build -o $@

test:
	go test -v -race

clean:
	rm -f $(TARGET)
