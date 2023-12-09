//
// Copyright 2017 The Kubernetes Authors.
// Copyright (c) 2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

package state

import (
	"path"
	"testing"

	"github.com/stretchr/testify/require"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"
)

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
