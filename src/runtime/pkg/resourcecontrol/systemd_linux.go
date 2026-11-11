// Copyright (c) 2026 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package resourcecontrol

import (
	"context"
	"errors"
	"os/signal"
	"syscall"
	"time"

	systemdDbus "github.com/coreos/go-systemd/v22/dbus"
	"github.com/godbus/dbus/v5"
)

type systemdStopConnection interface {
	StopUnitContext(context.Context, string, string, chan<- string) (int, error)
	Close()
}

var newSystemdStopConnection = func(ctx context.Context) (systemdStopConnection, error) {
	return systemdDbus.NewWithContext(ctx)
}

func deleteCgroupsSystemd(cgroupPath string) error {
	_, unit, err := getSliceAndUnit(cgroupPath)
	if err != nil {
		return err
	}

	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()
	conn, err := newSystemdStopConnection(ctx)
	if err != nil {
		return err
	}
	defer conn.Close()

	signal.Ignore(syscall.SIGTERM)
	_, err = conn.StopUnitContext(ctx, unit, "replace", nil)
	var unitError dbus.Error
	if errors.As(err, &unitError) && unitError.Name == "org.freedesktop.systemd1.NoSuchUnit" {
		return nil
	}
	return err
}
