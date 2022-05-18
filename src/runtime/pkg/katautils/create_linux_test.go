package katautils

func TestSetEphemeralStorageType(t *testing.T) {
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(ktu.TestDisabledNeedRoot)
	}

	assert := assert.New(t)

	dir := t.TempDir()

	ephePath := filepath.Join(dir, vc.K8sEmptyDir, "tmp-volume")
	err := os.MkdirAll(ephePath, testDirMode)
	assert.Nil(err)

	err = syscall.Mount("tmpfs", ephePath, "tmpfs", 0, "")
	assert.Nil(err)
	defer syscall.Unmount(ephePath, 0)

	ociSpec := specs.Spec{}
	var ociMounts []specs.Mount
	mount := specs.Mount{
		Source: ephePath,
	}

	ociMounts = append(ociMounts, mount)
	ociSpec.Mounts = ociMounts
	ociSpec = SetEphemeralStorageType(ociSpec, false)

	mountType := ociSpec.Mounts[0].Type
	assert.Equal(mountType, "ephemeral",
		"Unexpected mount type, got %s expected ephemeral", mountType)
}
