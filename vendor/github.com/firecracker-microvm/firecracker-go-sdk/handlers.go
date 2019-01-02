package firecracker

import (
	"context"
)

// Handler name constants
const (
	StartVMMHandlerName                = "fcinit.StartVMM"
	BootstrapLoggingHandlerName        = "fcinit.BootstrapLogging"
	CreateMachineHandlerName           = "fcinit.CreateMachine"
	CreateBootSourceHandlerName        = "fcinit.CreateBootSource"
	AttachDrivesHandlerName            = "fcinit.AttachDrives"
	CreateNetworkInterfacesHandlerName = "fcinit.CreateNetworkInterfaces"
	AddVsocksHandlerName               = "fcinit.AddVsocks"
	SetMetadataHandlerName             = "fcinit.SetMetadata"

	ValidateCfgHandlerName = "validate.Cfg"
)

// StartVMMHandler is a named handler that will handle starting of the VMM.
// This handler will also set the exit channel on completion.
var StartVMMHandler = Handler{
	Name: StartVMMHandlerName,
	Fn: func(ctx context.Context, m *Machine) error {
		return m.startVMM(ctx)
	},
}

// BootstrapLoggingHandler is a named handler that will set up fifo logging of
// firecracker process.
var BootstrapLoggingHandler = Handler{
	Name: BootstrapLoggingHandlerName,
	Fn: func(ctx context.Context, m *Machine) error {
		if err := m.setupLogging(ctx); err != nil {
			m.logger.Warnf("setupLogging() returned %s. Continuing anyway.", err)
		} else {
			m.logger.Debugf("setup logging: success")
		}

		return nil
	},
}

// CreateMachineHandler is a named handler that will "create" the machine and
// upload any necessary configuration to the firecracker process.
var CreateMachineHandler = Handler{
	Name: CreateMachineHandlerName,
	Fn: func(ctx context.Context, m *Machine) error {
		return m.createMachine(ctx)
	},
}

// CreateBootSourceHandler is a named handler that will set up the booting
// process of the firecracker process.
var CreateBootSourceHandler = Handler{
	Name: CreateBootSourceHandlerName,
	Fn: func(ctx context.Context, m *Machine) error {
		return m.createBootSource(ctx, m.cfg.KernelImagePath, m.cfg.KernelArgs)
	},
}

// AttachDrivesHandler is a named handler that will attach all drives for the
// firecracker process.
var AttachDrivesHandler = Handler{
	Name: AttachDrivesHandlerName,
	Fn: func(ctx context.Context, m *Machine) error {
		return m.attachDrives(ctx, m.cfg.Drives...)
	},
}

// CreateNetworkInterfacesHandler is a named handler that sets up network
// interfaces to the firecracker process.
var CreateNetworkInterfacesHandler = Handler{
	Name: CreateNetworkInterfacesHandlerName,
	Fn: func(ctx context.Context, m *Machine) error {
		return m.createNetworkInterfaces(ctx, m.cfg.NetworkInterfaces...)
	},
}

// AddVsocksHandler is a named handler that adds vsocks to the firecracker
// process.
var AddVsocksHandler = Handler{
	Name: AddVsocksHandlerName,
	Fn: func(ctx context.Context, m *Machine) error {
		return m.addVsocks(ctx, m.cfg.VsockDevices...)
	},
}

// NewSetMetadataHandler is a named handler that puts the metadata into the
// firecracker process.
func NewSetMetadataHandler(metadata interface{}) Handler {
	return Handler{
		Name: SetMetadataHandlerName,
		Fn: func(ctx context.Context, m *Machine) error {
			return m.SetMetadata(ctx, m.Metadata)
		},
	}
}

var defaultValidationHandlerList = HandlerList{}.Append(
	Handler{
		Name: ValidateCfgHandlerName,
		Fn: func(ctx context.Context, m *Machine) error {
			// ensure that the configuration is valid for the
			// FcInit handlers.
			return m.cfg.Validate()
		},
	},
)

var defaultFcInitHandlerList = HandlerList{}.Append(
	StartVMMHandler,
	BootstrapLoggingHandler,
	CreateMachineHandler,
	CreateBootSourceHandler,
	AttachDrivesHandler,
	CreateNetworkInterfacesHandler,
	AddVsocksHandler,
)

var defaultHandlers = Handlers{
	Validation: defaultValidationHandlerList,
	FcInit:     defaultFcInitHandlerList,
}

// Handler represents a named handler that contains a name and a function which
// is used to execute during the initialization process of a machine.
type Handler struct {
	Name string
	Fn   func(context.Context, *Machine) error
}

// Handlers is a container that houses categories of handler lists.
type Handlers struct {
	Validation HandlerList
	FcInit     HandlerList
}

// Run will execute all handlers in the Handlers object by flattening the lists
// into a single list and running.
func (h Handlers) Run(ctx context.Context, m *Machine) error {
	l := HandlerList{}.Append(
		h.Validation.list...,
	).Append(
		h.FcInit.list...,
	)

	return l.Run(ctx, m)
}

// HandlerList represents a list of named handler that can be used to execute a
// flow of instructions for a given machine.
type HandlerList struct {
	list []Handler
}

// Append will append a new handler to the handler list.
func (l HandlerList) Append(handlers ...Handler) HandlerList {
	l.list = append(l.list, handlers...)

	return l
}

// Len return the length of the given handler list
func (l HandlerList) Len() int {
	return len(l.list)
}

// Has will iterate through the handler list and check to see if the the named
// handler exists.
func (l HandlerList) Has(name string) bool {
	for _, h := range l.list {
		if h.Name == name {
			return true
		}
	}

	return false
}

// Swap will replace all elements of the given name with the new handler.
func (l HandlerList) Swap(handler Handler) HandlerList {
	newList := HandlerList{}
	for _, h := range l.list {
		if h.Name == handler.Name {
			newList.list = append(newList.list, handler)
			continue
		}

		newList.list = append(newList.list, h)
	}

	return newList
}

// Swappend will either append, if there isn't an element within the handler
// list, otherwise it will replace all elements with the given name.
func (l HandlerList) Swappend(handler Handler) HandlerList {
	if l.Has(handler.Name) {
		return l.Swap(handler)
	}

	return l.Append(handler)
}

// Remove will return an updated handler with all instances of the specific
// named handler being removed.
func (l HandlerList) Remove(name string) HandlerList {
	newList := HandlerList{}
	for _, h := range l.list {
		if h.Name != name {
			newList.list = append(newList.list, h)
		}
	}

	return newList
}

// Clear clears all named handler in the list.
func (l HandlerList) Clear() HandlerList {
	l.list = l.list[0:0]
	return l
}

// Run will execute each instruction in the handler list. If an error occurs in
// any of the handlers, then the list will halt execution and return the error.
func (l HandlerList) Run(ctx context.Context, m *Machine) error {
	for _, handler := range l.list {
		m.logger.Debugf("Running handler %s", handler.Name)
		if err := handler.Fn(ctx, m); err != nil {
			return err
		}
	}

	return nil
}
