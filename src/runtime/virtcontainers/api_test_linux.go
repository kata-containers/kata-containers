package virtcontainers

import (
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist/fs"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/mock"
)

func TestCreateSandboxNoopAgentSuccessful(t *testing.T) {
	assert := assert.New(t)
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(testDisabledAsNonRoot)
	}
	defer cleanUp()

	// Pre-create the directory path to avoid panic error. Without this change, ff the test is run as a non-root user,
	// this test will fail because of permission denied error in chown syscall in the utils.MkdirAllWithInheritedOwner() method
	err := os.MkdirAll(fs.MockRunStoragePath(), DirMode)
	assert.NoError(err)

	config := newTestSandboxConfigNoop()

	ctx := WithNewAgentFunc(context.Background(), newMockAgent)
	p, err := CreateSandbox(ctx, config, nil)
	assert.NoError(err)
	assert.NotNil(p)

	s, ok := p.(*Sandbox)
	assert.True(ok)
	assert.NotNil(s)

	sandboxDir := filepath.Join(s.store.RunStoragePath(), p.ID())
	_, err = os.Stat(sandboxDir)
	assert.NoError(err)
}

func TestCreateSandboxKataAgentSuccessful(t *testing.T) {
	assert := assert.New(t)
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(testDisabledAsNonRoot)
	}

	defer cleanUp()

	config := newTestSandboxConfigKataAgent()

	url, err := mock.GenerateKataMockHybridVSock()
	assert.NoError(err)
	defer mock.RemoveKataMockHybridVSock(url)

	hybridVSockTTRPCMock := mock.HybridVSockTTRPCMock{}
	err = hybridVSockTTRPCMock.Start(url)
	assert.NoError(err)
	defer hybridVSockTTRPCMock.Stop()

	ctx := WithNewAgentFunc(context.Background(), newMockAgent)
	p, err := CreateSandbox(ctx, config, nil)
	assert.NoError(err)
	assert.NotNil(p)

	s, ok := p.(*Sandbox)
	assert.True(ok)
	sandboxDir := filepath.Join(s.store.RunStoragePath(), p.ID())
	_, err = os.Stat(sandboxDir)
	assert.NoError(err)
}

func TestCreateSandboxFailing(t *testing.T) {
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(testDisabledAsNonRoot)
	}
	defer cleanUp()
	assert := assert.New(t)

	config := SandboxConfig{}

	ctx := WithNewAgentFunc(context.Background(), newMockAgent)
	p, err := CreateSandbox(ctx, config, nil)
	assert.Error(err)
	assert.Nil(p.(*Sandbox))
}

func TestReleaseSandbox(t *testing.T) {
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(testDisabledAsNonRoot)
	}
	defer cleanUp()

	config := newTestSandboxConfigNoop()

	ctx := WithNewAgentFunc(context.Background(), newMockAgent)
	s, err := CreateSandbox(ctx, config, nil)
	assert.NoError(t, err)
	assert.NotNil(t, s)

	err = s.Release(ctx)
	assert.Nil(t, err, "sandbox release failed: %v", err)
}
