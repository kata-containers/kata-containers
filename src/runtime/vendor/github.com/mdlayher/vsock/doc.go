// Package vsock provides access to Linux VM sockets (AF_VSOCK) for
// communication between a hypervisor and its virtual machines.
//
// The types in this package implement interfaces provided by package net and
// may be used in applications that expect a net.Listener or net.Conn.
//
//   - *Addr implements net.Addr
//   - *Conn implements net.Conn
//   - *Listener implements net.Listener
//
// Go version support
//
// This package supports varying levels of functionality depending on the version
// of Go used during compilation. The Listener and Conn types produced by this
// package are backed by non-blocking I/O, in order to integrate with Go's
// runtime network poller in Go 1.11+. Additional functionality is available
// starting in Go 1.12+.
//
// Go 1.12+ (recommended):
//   - *Listener:
//     - Accept blocks until a connection is received
//     - Close can interrupt Accept and make it return a permanent error
//     - SetDeadline can set timeouts which can interrupt Accept and make it return a
//       temporary error
//   - *Conn:
//     - SetDeadline family of methods are fully supported
//     - CloseRead and CloseWrite can close the reading or writing sides of a
//       Conn, respectively
//     - SyscallConn provides access to raw network control/read/write functionality
//
// Go 1.11 (not recommended):
//   - *Listener:
//     - Accept is non-blocking and should be called in a loop, checking for
//       net.Error.Temporary() == true and sleeping for a short period to avoid wasteful
//       CPU cycle consumption
//     - Close makes Accept return a permanent error on the next loop iteration
//     - SetDeadline is not supported and will always return an error
//   - *Conn:
//     - SetDeadline family of methods are fully supported
//     - CloseRead and CloseWrite are not supported and will always return an error
//     - SyscallConn is not supported and will always return an error
//
// Go 1.10 (not recommended):
//   - *Listener:
//     - Accept blocks until a connection is received
//     - Close cannot unblock Accept
//     - SetDeadline is not supported and will always return an error
//   - *Conn:
//     - SetDeadline is not supported and will always return an error
//     - CloseRead and CloseWrite are not supported and will always return an error
//     - SyscallConn is not supported and will always return an error
//
// Stability
//
// At this time, package vsock is in a pre-v1.0.0 state. Changes are being made
// which may impact the exported API of this package and others in its ecosystem.
//
// If you depend on this package in your application, please use Go modules when
// building your application.
package vsock
