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

// TestAtomicSave checks that saving leaves only the state file behind, also
// when the file cannot be replaced.
func TestAtomicSave(t *testing.T) {
	dir := t.TempDir()
	stateFile := path.Join(dir, "state.json")
	s, err := New(stateFile)
	require.NoError(t, err, "construct state")
	volume := Volume{VolID: "foo", VolName: "bar"}
	require.NoError(t, s.UpdateVolume(volume), "add volume")
	entries, err := os.ReadDir(dir)
	require.NoError(t, err)
	require.Len(t, entries, 1, "only the state file must be left")

	// The state file cannot be replaced while a directory has its name.
	require.NoError(t, os.Remove(stateFile))
	require.NoError(t, os.Mkdir(stateFile, 0750))
	require.Error(t, s.UpdateVolume(Volume{VolID: "new"}), "add volume")
	require.Equal(t, []Volume{volume}, s.GetVolumes(), "volumes after the failed change")
	entries, err = os.ReadDir(dir)
	require.NoError(t, err)
	require.Len(t, entries, 1, "a failed save must not leave its temporary file")
}

// TestVolumesAreCopies checks that callers cannot change the state through
// the volumes which they pass in or get back.
func TestVolumesAreCopies(t *testing.T) {
	s, err := New(path.Join(t.TempDir(), "state.json"))
	require.NoError(t, err, "construct state")

	volume := Volume{VolID: "foo", VolName: "bar", Published: Strings{"a", "b"}, Metadata: map[string]string{"key": "value"}}
	require.NoError(t, s.UpdateVolume(volume), "add volume")
	volume.Published[0] = "changed"
	volume.Metadata["key"] = "changed"

	got, err := s.GetVolumeByID("foo")
	require.NoError(t, err)
	require.Equal(t, Strings{"a", "b"}, got.Published)
	require.Equal(t, "value", got.Metadata["key"])

	got.Published[1] = "changed"
	got.Metadata["key"] = "changed"
	got, err = s.GetVolumeByName("bar")
	require.NoError(t, err)
	require.Equal(t, Strings{"a", "b"}, got.Published)
	require.Equal(t, "value", s.GetVolumes()[0].Metadata["key"])
}

// TestFailedSaveKeepsTargets checks an unpublish whose save fails: the
// volume must still have both targets, so that a retry finds its target.
func TestFailedSaveKeepsTargets(t *testing.T) {
	dir := path.Join(t.TempDir(), "state")
	require.NoError(t, os.Mkdir(dir, 0750))
	s, err := New(path.Join(dir, "state.json"))
	require.NoError(t, err, "construct state")
	require.NoError(t, s.UpdateVolume(Volume{VolID: "foo", Published: Strings{"a", "b"}}), "add volume")
	require.NoError(t, os.RemoveAll(dir))

	volume, err := s.GetVolumeByID("foo")
	require.NoError(t, err)
	volume.Published.Remove("a")
	require.Error(t, s.UpdateVolume(volume), "unpublish")

	got, err := s.GetVolumeByID("foo")
	require.NoError(t, err)
	require.Equal(t, Strings{"a", "b"}, got.Published)
}
