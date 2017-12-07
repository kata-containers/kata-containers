TARGET = kata-shim
SOURCES := $(shell find . 2>&1 | grep -E '.*\.go$$')

$(TARGET): $(SOURCES)
	go build -o $@

test:
	go test -v -race

clean:
	rm -f $(TARGET)
