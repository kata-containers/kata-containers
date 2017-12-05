//+build !linux

package vsock

import "testing"

func TestUnimplemented(t *testing.T) {
	want := errUnimplemented

	if _, got := listenStream(0); want != got {
		t.Fatalf("unexpected error from listenStream:\n- want: %v\n-  got: %v",
			want, got)
	}

	if _, got := dialStream(0, 0); want != got {
		t.Fatalf("unexpected error from dialStream:\n- want: %v\n-  got: %v",
			want, got)
	}
}
