package vsock

import "testing"

func TestAddr_fileName(t *testing.T) {
	tests := []struct {
		cid  uint32
		port uint32
		s    string
	}{
		{
			cid:  ContextIDHypervisor,
			port: 10,
			s:    "vsock:hypervisor(0):10",
		},
		{
			cid:  ContextIDReserved,
			port: 20,
			s:    "vsock:reserved(1):20",
		},
		{
			cid:  ContextIDHost,
			port: 30,
			s:    "vsock:host(2):30",
		},
		{
			cid:  3,
			port: 40,
			s:    "vsock:vm(3):40",
		},
	}

	for _, tt := range tests {
		t.Run(tt.s, func(t *testing.T) {
			addr := &Addr{
				ContextID: tt.cid,
				Port:      tt.port,
			}

			if want, got := tt.s, addr.fileName(); want != got {
				t.Fatalf("unexpected file name:\n- want: %q\n-  got: %q",
					want, got)
			}
		})
	}
}
