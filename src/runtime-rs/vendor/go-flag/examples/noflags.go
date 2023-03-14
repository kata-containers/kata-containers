package main

import (
	"flag"
	"fmt"
)

func main() {
	flag.Parse()
	args := flag.Args()
	for _, arg := range args {
		fmt.Println(arg)
	}
}
