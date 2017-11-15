// A simple proxy that multiplexes a unix socket connection
//
// Copyright 2017 HyperHQ Inc.

package main

import (
	"crypto/md5"
	"flag"
	"fmt"
	"io"
	"net"
	"os"
)

func main() {
	file := flag.String("f", "", "input file to send to server")
	buf := []byte("hello proxy")
	proxyAddr := "/tmp/proxy.sock"

	flag.Parse()

	conn, err := net.Dial("unix", proxyAddr)
	if err != nil {
		fmt.Println("dial failed: ", err)
		return
	}
	defer conn.Close()

	var sum1 string
	var expected int64
	if *file != "" {
		f, err := os.Open(*file)
		if err != nil {
			fmt.Printf("open %s failed: %s\n", *file, err.Error())
			return
		}
		defer f.Close()

		h := md5.New()
		expected, err = io.Copy(h, f)
		if err != nil {
			fmt.Printf("read %s failed: %s\n", *file, err.Error())
			return
		}
		sum1 = fmt.Sprintf("%x", h.Sum(nil))

		_, err = f.Seek(0, os.SEEK_SET)
		if err != nil {
			fmt.Printf("seek %s failed: %s\n", *file, err.Error())
			return
		}
		go io.Copy(conn, f)

	} else {
		sum1 = fmt.Sprintf("%x", md5.Sum(buf))

		size, err := conn.Write(buf)
		if err != nil {
			fmt.Println("write failed: ", err)
			return
		}
		expected = int64(size)
	}

	// read from server
	h := md5.New()
	var result []byte
	for {
		if expected >= 1024 {
			result = make([]byte, 1024, 1024)
		} else if expected > 0 {
			result = make([]byte, expected, expected)
		} else {
			break
		}
		size, err := conn.Read(result)
		if err != nil {
			fmt.Println("read failed: ", err)
			return
		}
		_, err = h.Write(result[:size])
		if err != nil {
			fmt.Println("write hash failed: ", err)
			return
		}
		expected -= int64(size)
	}

	sum2 := fmt.Sprintf("%x", h.Sum(nil))

	if sum1 != sum2 {
		fmt.Printf("unmatched checksum:\norig:\t%s\nnew:\t%s\n", sum1, sum2)
	} else {
		fmt.Printf("matched checksum: %s\n", sum1)
	}
}
