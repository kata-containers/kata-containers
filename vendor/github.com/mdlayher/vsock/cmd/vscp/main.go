// Command vscp provides a scp-like utility for copying files over VM
// sockets.  It is meant to show example usage of package vsock, but is
// also useful in scenarios where a virtual machine does not have
// networking configured, but VM sockets are available.
package main

import (
	"flag"
	"io"
	"log"
	"os"

	"github.com/mdlayher/vsock"
)

var (
	flagVerbose = flag.Bool("v", false, "enable verbose logging to stderr")
)

func main() {
	var (
		flagReceive = flag.Bool("r", false, "receive files from another instance of vscp")
		flagSend    = flag.Bool("s", false, "send files to another instance of vscp")

		flagContextID = flag.Uint("c", 0, "send only: context ID of the remote VM socket")
		flagPort      = flag.Uint("p", 0, "- receive: port ID to listen on (random port by default)\n\t- send: port ID to connect to")
	)

	flag.Parse()
	log.SetOutput(os.Stderr)

	// Determine if target should be stdin/stdout or a regular file.
	var target string
	if t := flag.Arg(0); t != "" {
		target = t
	}

	switch {
	case *flagReceive && *flagSend:
		log.Fatalf(`vscp: specify only one of "-r" for receive or "-s" for send`)
	case *flagReceive:
		if *flagContextID != 0 {
			log.Fatalf(`vscp: context ID flag "-c" not valid for receive operation`)
		}

		receive(target, uint32(*flagPort))
	case *flagSend:
		send(target, uint32(*flagContextID), uint32(*flagPort))
	default:
		flag.PrintDefaults()
	}
}

// receive starts a server and receives data from a remote client using
// VM sockets.  The data is written to target, which may be a file,
// or stdout, if no file is specified.
func receive(target string, port uint32) {
	// Log helper functions.
	logf := func(format string, a ...interface{}) {
		logf("receive: "+format, a...)
	}

	fatalf := func(format string, a ...interface{}) {
		log.Fatalf("vscp: receive: "+format, a...)
	}

	// Determine if target is stdout or a file to be created.
	var w io.Writer
	switch target {
	case "":
		logf("empty target, file will be written to stdout")
		w = os.Stdout
	default:
		logf("creating file %q for output", target)
		f, err := os.Create(target)
		if err != nil {
			fatalf("failed to create output file: %q", err)
		}
		defer f.Close()
		w = f
	}

	logf("opening listener: %d", port)

	l, err := vsock.Listen(port)
	if err != nil {
		fatalf("failed to listen: %v", err)
	}
	defer l.Close()

	// Show server's address for setting up client flags.
	log.Printf("receive: listening: %s", l.Addr())

	// Accept a single connection, and receive stream from that connection.
	c, err := l.Accept()
	if err != nil {
		fatalf("failed to accept: %v", err)
	}
	defer c.Close()

	logf("server: %s", c.LocalAddr())
	logf("client: %s", c.RemoteAddr())
	logf("receiving data")

	if _, err := io.Copy(w, c); err != nil {
		fatalf("failed to receive data: %v", err)
	}

	logf("transfer complete")
}

// send dials a server and sends data to it using VM sockets.  The data
// is read from target, which may be a file, or stdin if no file or "-"
// is specified.
func send(target string, cid, port uint32) {
	// Log helper functions.
	logf := func(format string, a ...interface{}) {
		logf("send: "+format, a...)
	}

	fatalf := func(format string, a ...interface{}) {
		log.Fatalf("vscp: send: "+format, a...)
	}

	// Determine if target is stdin or a file to be read in.
	var r io.Reader
	switch target {
	case "", "-":
		logf("empty or stdin target, file will be read from stdin")
		r = os.Stdin
	default:
		logf("opening file %q for input", target)
		f, err := os.Open(target)
		if err != nil {
			fatalf("failed to open input file: %q", err)
		}
		defer f.Close()
		r = f
	}

	logf("dialing: %d.%d", cid, port)

	// Dial a remote server and send a stream to that server.
	c, err := vsock.Dial(cid, port)
	if err != nil {
		fatalf("failed to dial: %v", err)
	}
	defer c.Close()

	logf("client: %s", c.LocalAddr())
	logf("server: %s", c.RemoteAddr())

	logf("sending data")
	if _, err := io.Copy(c, r); err != nil {
		fatalf("failed to send data: %v", err)
	}

	logf("transfer complete")
}

// logf shows verbose logging if -v is specified, or does nothing
// if it is not.
func logf(format string, a ...interface{}) {
	if !*flagVerbose {
		return
	}

	log.Printf(format, a...)
}
