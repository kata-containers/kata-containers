//
// Copyright 2017 The Kubernetes Authors.
// Copyright (c) 2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

package state

import (
	"os"
	"path"
	"testing"

	"github.com/stretchr/testify/require"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"
)

// TestFailedSave checks that a change which cannot be saved is not applied.
func TestFailedSave(t *testing.T) {
	dir := path.Join(t.TempDir(), "state")
	require.NoError(t, os.Mkdir(dir, 0750))

	s, err := New(path.Join(dir, "state.json"))
	require.NoError(t, err, "construct state")
	volume := Volume{VolID: "foo", VolName: "bar"}
	require.NoError(t, s.UpdateVolume(volume), "add volume")

	// Without its directory the state file cannot be written.
	require.NoError(t, os.RemoveAll(dir))

	require.Error(t, s.UpdateVolume(Volume{VolID: "new"}), "add volume")
	require.Error(t, s.UpdateVolume(Volume{VolID: "foo", VolName: "changed"}), "change volume")
	require.Error(t, s.DeleteVolume("foo"), "delete volume")
	require.Equal(t, []Volume{volume}, s.GetVolumes(), "volumes after the failed changes")
}

func TestVolumes(t *testing.T) {
	tmp := t.TempDir()
	statefileName := path.Join(tmp, "state.json")

	s, err := New(statefileName)
	require.NoError(t, err, "construct state")
	require.Empty(t, s.GetVolumes(), "initial volumes")

	_, err = s.GetVolumeByID("foo")
	require.Equal(t, codes.NotFound, status.Convert(err).Code(), "GetVolumeByID of non-existent volume")
	require.Contains(t, status.Convert(err).Message(), "foo")

	err = s.UpdateVolume(Volume{VolID: "foo", VolName: "bar"})
	require.NoError(t, err, "add volume")

	s, err = New(statefileName)
	require.NoError(t, err, "reconstruct state")
	_, err = s.GetVolumeByID("foo")
	require.NoError(t, err, "get existing volume by ID")
	_, err = s.GetVolumeByName("bar")
	require.NoError(t, err, "get existing volume by name")

	err = s.DeleteVolume("foo")
	require.NoError(t, err, "delete existing volume")

	err = s.DeleteVolume("foo")
	require.NoError(t, err, "delete non-existent volume")

	require.Empty(t, s.GetVolumes(), "final volumes")
}
