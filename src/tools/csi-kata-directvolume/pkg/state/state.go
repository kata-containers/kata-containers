//
// Copyright 2017 The Kubernetes Authors.
// Copyright (c) 2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

// Package state manages the internal state of the driver which needs to be maintained
// across driver restarts.
package state

import (
	"encoding/json"
	"errors"
	"os"
	"path/filepath"

	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"
)

type Volume struct {
	VolName string
	VolID   string
	VolSize int64
	VolPath string
	// VolAccessType  AccessType
	ParentVolID    string
	ParentSnapID   string
	NodeID         string
	Kind           string
	ReadOnlyAttach bool
	Attached       bool
	// Staged contains the staging target path at which the volume
	// was staged. A set of paths is used for consistency
	// with Published.
	Staged Strings
	// Published contains the target paths where the volume
	// was published.
	Published Strings

	Metadata map[string]string
}

// State is the interface that the rest of the code has to use to
// access and change state. All error messages contain gRPC
// status codes and can be returned without wrapping.
type State interface {
	// GetVolumeByID retrieves a volume by its unique ID or returns
	// an error including that ID when not found.
	GetVolumeByID(volID string) (Volume, error)

	// GetVolumeByName retrieves a volume by its name or returns
	// an error including that name when not found.
	GetVolumeByName(volName string) (Volume, error)

	// GetVolumes returns all currently existing volumes.
	GetVolumes() []Volume

	// UpdateVolume updates the existing direct volume,
	// identified by its volume ID, or adds it if it does
	// not exist yet.
	UpdateVolume(volume Volume) error

	// DeleteVolume deletes the volume with the given
	// volume ID. It is not an error when such a volume
	// does not exist.
	DeleteVolume(volID string) error
}

type resources struct {
	Volumes []Volume
}

type state struct {
	resources
	statefilePath string
}

var _ State = &state{}

// New retrieves the complete state of the driver from the file if given
// and then ensures that all changes are mirrored immediately in the
// given file. If not given, the initial state is empty and changes
// are not saved.
func New(statefilePath string) (State, error) {
	s := &state{
		statefilePath: statefilePath,
	}

	return s, s.restore()
}

// commit writes a complete replacement before changing the in-memory ledger.
// Failed writes must not lose reservations or consume quota until restart.
func (s *state) commit(volumes []Volume) error {
	data, err := json.Marshal(resources{Volumes: volumes})
	if err != nil {
		return status.Errorf(codes.Internal, "encode volumes: %v", err)
	}
	file, err := os.CreateTemp(filepath.Dir(s.statefilePath), ".state-*")
	if err != nil {
		return status.Errorf(codes.Internal, "create state file: %v", err)
	}
	defer os.Remove(file.Name())
	if _, err = file.Write(data); err == nil {
		err = file.Sync()
	}
	closeErr := file.Close()
	if err == nil {
		err = closeErr
	}
	if err == nil {
		err = os.Rename(file.Name(), s.statefilePath)
	}
	if err != nil {
		return status.Errorf(codes.Internal, "write state file: %v", err)
	}
	s.Volumes = volumes
	return nil
}

func (s *state) restore() error {
	s.Volumes = nil
	data, err := os.ReadFile(s.statefilePath)
	switch {
	case errors.Is(err, os.ErrNotExist):
		// Nothing to do.
		return nil
	case err != nil:
		return status.Errorf(codes.Internal, "error reading state file: %v", err)
	}
	if err := json.Unmarshal(data, &s.resources); err != nil {
		return status.Errorf(codes.Internal, "error encoding volumes from state file %q: %v", s.statefilePath, err)
	}
	return nil
}

func (s *state) GetVolumeByID(volID string) (Volume, error) {
	for _, volume := range s.Volumes {
		if volume.VolID == volID {
			return cloneVolume(volume), nil
		}
	}
	return Volume{}, status.Errorf(codes.NotFound, "volume id %s does not exist in the volumes list", volID)
}

func (s *state) GetVolumeByName(volName string) (Volume, error) {
	for _, volume := range s.Volumes {
		if volume.VolName == volName {
			return cloneVolume(volume), nil
		}
	}
	return Volume{}, status.Errorf(codes.NotFound, "volume name %s does not exist in the volumes list", volName)
}

func (s *state) GetVolumes() []Volume {
	volumes := make([]Volume, len(s.Volumes))
	for i, volume := range s.Volumes {
		volumes[i] = cloneVolume(volume)
	}
	return volumes
}

func (s *state) UpdateVolume(update Volume) error {
	update = cloneVolume(update)
	volumes := s.GetVolumes()
	for i, volume := range volumes {
		if volume.VolID == update.VolID {
			volumes[i] = update
			return s.commit(volumes)
		}
	}
	return s.commit(append(volumes, update))
}

func (s *state) DeleteVolume(volID string) error {
	volumes := s.GetVolumes()
	for i, volume := range volumes {
		if volume.VolID == volID {
			return s.commit(append(volumes[:i], volumes[i+1:]...))
		}
	}
	return nil
}

func cloneVolume(volume Volume) Volume {
	volume.Staged = append(Strings(nil), volume.Staged...)
	volume.Published = append(Strings(nil), volume.Published...)
	if volume.Metadata != nil {
		metadata := make(map[string]string, len(volume.Metadata))
		for key, value := range volume.Metadata {
			metadata[key] = value
		}
		volume.Metadata = metadata
	}
	return volume
}
