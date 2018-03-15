// Package vsock provides access to Linux VM sockets (AF_VSOCK) for
// communication between a hypervisor and its virtual machines.
package vsock

import (
	"fmt"
	"net"
)

const (
	// ContextIDHypervisor specifies that a socket should communicate with
	// the hypervisor process.
	ContextIDHypervisor uint32 = 0x0

	// ContextIDReserved is a reserved context ID that is no longer in use,
	// and cannot be used for socket communications.
	ContextIDReserved uint32 = 0x1

	// ContextIDHost specifies that a socket should communicate with other
	// processes than the hypervisor on the host machine.
	ContextIDHost uint32 = 0x2
)

// Listen opens a connection-oriented net.Listener for incoming VM sockets
// connections.  The port parameter specifies the port for the listener.
//
// To allow the server to assign a port automatically, specify 0 for port.
// The address of the server can be retrieved using the Addr method.
//
// The Accept method is used to accept incoming connections.
//
// When the listener is no longer needed, Close must be called to free resources.
func Listen(port uint32) (net.Listener, error) {
	return listenStream(port)
}

// Dial dials a connection-oriented net.Conn to a VM sockets server.
// The contextID and port parameters specify the address of the server.
//
// If dialing a connection from the hypervisor to a virtual machine, the VM's
// context ID should be specified.
//
// If dialing from a VM to the hypervisor, ContextIDHypervisor should be used
// to talk to the hypervisor process, or ContextIDHost should be used to talk
// to other processes on the host machine.
//
// When the connection is no longer needed, Close must be called to free resources.
func Dial(contextID, port uint32) (net.Conn, error) {
	return dialStream(contextID, port)
}

// TODO(mdlayher): ListenPacket and DialPacket (or maybe another parameter for Dial?).

var _ net.Addr = &Addr{}

// An Addr is the address of a VM sockets endpoint.
type Addr struct {
	ContextID uint32
	Port      uint32
}

// Network returns the address's network name, "vsock".
func (a *Addr) Network() string { return "vsock" }

// String returns a human-readable representation of Addr, and indicates if
// ContextID is meant to be used for a hypervisor, host, VM, etc.
func (a *Addr) String() string {
	var host string

	switch a.ContextID {
	case ContextIDHypervisor:
		host = fmt.Sprintf("hypervisor(%d)", a.ContextID)
	case ContextIDReserved:
		host = fmt.Sprintf("reserved(%d)", a.ContextID)
	case ContextIDHost:
		host = fmt.Sprintf("host(%d)", a.ContextID)
	default:
		host = fmt.Sprintf("vm(%d)", a.ContextID)
	}

	return fmt.Sprintf("%s:%d", host, a.Port)
}

// fileName returns a file name for use with os.NewFile for Addr.
func (a *Addr) fileName() string {
	return fmt.Sprintf("%s:%s", a.Network(), a.String())
}
