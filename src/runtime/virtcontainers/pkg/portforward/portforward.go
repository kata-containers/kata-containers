package portforward

import (
	"context"
	"errors"
	"fmt"
	"io"
	"net"
	"strconv"
	"strings"
	"sync"
	"sync/atomic"

	"github.com/mdlayher/vsock"
	log "github.com/sirupsen/logrus"
)

type portForwarder struct {
	containerID string
	port        uint32

	vSockPort uint32
	vSockCID  string
	vSockAddr net.Addr

	listener *vsock.Listener

	// housekeeping
	refCount *uint32
}

var portForwarderMap map[string]*portForwarder
var portForwarderLock *sync.Mutex

var logger = log.WithFields(log.Fields{
	"source":    "virtcontainers",
	"subsystem": "portforward",
})

func init() {
	portForwarderMap = map[string]*portForwarder{}
	portForwarderLock = &sync.Mutex{}
}

func NewPortForwarder(containerID string, port uint32) (*portForwarder, error) {
	portForwarderLock.Lock()
	defer portForwarderLock.Unlock()

	key := getContainerPort(containerID, port)
	if forwarder, ok := portForwarderMap[key]; ok {
		atomic.AddUint32(forwarder.refCount, 1)
		return forwarder, nil
	}

	refCount := uint32(1)
	forwarder := &portForwarder{
		containerID: containerID,
		port:        port,
		refCount:    &refCount,
	}

	listener, err := vsock.Listen(0, nil)
	if err != nil {
		return nil, err
	}
	forwarder.listener = listener

	addr := listener.Addr()
	if cid, port, err := parseVSockAddr(addr); err != nil {
		return nil, err
	} else {
		forwarder.vSockCID = cid
		forwarder.vSockPort = port
	}
	forwarder.vSockAddr = addr

	logger.WithFields(log.Fields{
		"container-id": containerID,
		"port":         port,
		"vsock-addr":   addr,
	}).Debug("portforwarding listener started")

	portForwarderMap[key] = forwarder
	return forwarder, nil
}

func (p *portForwarder) Forward(ctx context.Context, to net.Conn) error {
	defer atomic.AddUint32(p.refCount, ^uint32(0))

	errChan := make(chan error)
	doneChan := make(chan struct{})

	connChan := make(chan net.Conn)
	go func() {
		c, err := p.listener.Accept()
		if err != nil {
			errChan <- err
			return
		}
		connChan <- c
	}()

	var c net.Conn
	select {
	case <-ctx.Done():
		return ctx.Err()
	case err := <-errChan:
		return err
	case c = <-connChan:
		defer c.Close()
	}

	wg := sync.WaitGroup{}
	wg.Add(2)

	go func() {
		wg.Wait()
		doneChan <- struct{}{}
	}()

	go func() {
		defer wg.Done()
		if _, err := io.Copy(c, to); err != nil {
			if !errors.Is(err, io.EOF) {
				errChan <- err
			}
		}
	}()

	go func() {
		defer wg.Done()
		if _, err := io.Copy(to, c); err != nil {
			if !errors.Is(err, io.EOF) {
				errChan <- err
			}
		}
	}()

	select {
	case <-ctx.Done():
		return ctx.Err()
	case <-doneChan:
		return nil
	case err := <-errChan:
		return err
	}
}

func (p *portForwarder) Close() {
	portForwarderLock.Lock()
	defer portForwarderLock.Unlock()

	// Close and cleanup from pool of listeners
	// if ref count is 0. This technique ensures
	// that unused listeners do not pollute the system
	if atomic.LoadUint32(p.refCount) == 0 {
		logger.WithFields(log.Fields{
			"container-id": p.containerID,
			"port":         p.port,
			"vsock-addr":   p.vSockAddr,
		}).Debug("portforwarding listener closed")

		p.listener.Close()
		delete(portForwarderMap, getContainerPort(p.containerID, p.port))
	}
}

func (p *portForwarder) Reset() error {
	portForwarderLock.Lock()
	defer portForwarderLock.Unlock()

	listener, err := vsock.Listen(0, nil)
	if err != nil {
		return err
	}
	p.listener = listener

	addr := listener.Addr()

	if cid, port, err := parseVSockAddr(addr); err != nil {
		return err
	} else {
		p.vSockCID = cid
		p.vSockPort = port
	}
	p.vSockAddr = addr
	return nil
}

func (p *portForwarder) GetContainerID() string {
	return p.containerID
}

func (p *portForwarder) GetPort() uint32 {
	return p.port
}

func (p *portForwarder) GetVSockPort() uint32 {
	return p.vSockPort
}

func (p *portForwarder) GetVSockCID() string {
	return p.vSockCID
}

func (p *portForwarder) GetVSockAddr() net.Addr {
	return p.vSockAddr
}

func getContainerPort(containerID string, port uint32) string {
	return fmt.Sprintf("%s:%d", containerID, port)
}

func parseVSockAddr(addr net.Addr) (string, uint32, error) {
	if addr.Network() != "vsock" {
		return "", 0, errors.New("invalid vsock listen addr")
	}

	items := strings.Split(addr.String(), ":")
	if len(items) != 2 {
		return "", 0, errors.New("invalid vsock listen addr")
	}

	fields := strings.FieldsFunc(items[0], func(r rune) bool {
		return r == '(' || r == ')'
	})

	if len(fields) != 2 {
		return "", 0, errors.New("invalid vsock listen addr")
	}

	cid := fields[1]

	port, err := strconv.ParseUint(items[1], 10, 32)
	if err != nil {
		return "", 0, err
	}

	return cid, uint32(port), nil
}
