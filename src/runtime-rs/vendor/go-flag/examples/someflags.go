package main

import (
	"flag"
	"fmt"
)

func main() {
	force := flag.Bool("f", false, "force")
	lines := flag.Int("lines", 10, "lines")
	flag.Parse()
	fmt.Printf("force = %v\n", *force)
	fmt.Printf("lines = %v\n", *lines)
	args := flag.Args()
	for _, arg := range args {
		fmt.Println(arg)
	}
}
