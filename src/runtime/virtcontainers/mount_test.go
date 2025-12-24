// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"

	ktu "github.com/kata-containers/kata-containers/src/runtime/pkg/katatestutils"
	"github.com/pkg/errors"
	"github.com/stretchr/testify/assert"
)

const (
	testDirMode = os.FileMode(0750)
)

var tc ktu.TestConstraint

func init() {
	tc = ktu.NewTestConstraint(false)
}

func TestIsSystemMount(t *testing.T) {
	assert := assert.New(t)
	tests := []struct {
		mnt      string
		expected bool
	}{
		{"/sys", true},
		{"/sys/", true},
		{"/sys//", true},
		{"/sys/fs", true},
		{"/sys/fs/", true},
		{"/sys/fs/cgroup", true},
		{"/sysfoo", false},
		{"/home", false},
		{"/dev/block/", false},
		{"/mnt/dev/foo", false},
		{"/../sys", true},
		{"/../sys/", true},
		{"/../sys/fs/cgroup", true},
		{"/../sysfoo", false},
	}

	for _, test := range tests {
		result := isSystemMount(test.mnt)
		assert.Exactly(result, test.expected)
	}
}

func TestIsHostDeviceCreateFile(t *testing.T) {
	assert := assert.New(t)
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(ktu.TestDisabledNeedRoot)
	}
	// Create regular file in /dev

	path := "/dev/foobar"
	f, err := os.Create(path)
	assert.NoError(err)
	f.Close()

	isDevice, err := isHostDevice(path)
	assert.False(isDevice)
	assert.NoError(err)
	assert.NoError(os.Remove(path))
}

func TestGetDeviceForPathRoot(t *testing.T) {
	assert := assert.New(t)
	dev, err := getDeviceForPath("/")
	assert.NoError(err)

	expected := "/"

	assert.Equal(dev.mountPoint, expected)
}

func TestGetDeviceForPathEmptyPath(t *testing.T) {
	assert := assert.New(t)
	_, err := getDeviceForPath("")
	assert.Error(err)
}

func TestGetDeviceForPath(t *testing.T) {
	assert := assert.New(t)

	dev, err := getDeviceForPath("///")
	assert.NoError(err)

	assert.Equal(dev.mountPoint, "/")

	_, err = getDeviceForPath("/../../.././././../.")
	assert.NoError(err)

	_, err = getDeviceForPath("/root/file with spaces")
	assert.Error(err)
}

func TestIsDockerVolume(t *testing.T) {
	assert := assert.New(t)
	path := "/var/lib/docker/volumes/00da1347c7cf4f15db35f/_data"
	isDockerVolume := IsDockerVolume(path)
	assert.True(isDockerVolume)

	path = "/var/lib/testdir"
	isDockerVolume = IsDockerVolume(path)
	assert.False(isDockerVolume)
}

func TestIsEmtpyDir(t *testing.T) {
	assert := assert.New(t)
	path := "/var/lib/kubelet/pods/5f0861a0-a987-4a3a-bb0f-1058ddb9678f/volumes/kubernetes.io~empty-dir/foobar"
	result := isEmptyDir(path)
	assert.True(result)

	// expect the empty-dir to be second to last in path
	result = isEmptyDir(filepath.Join(path, "bazzzzz"))
	assert.False(result)
}

func TestIsConfigMap(t *testing.T) {
	assert := assert.New(t)
	path := "/var/lib/kubelet/pods/5f0861a0-a987-4a3a-bb0f-1058ddb9678f/volumes/kubernetes.io~configmap/config"
	result := isConfigMap(path)
	assert.True(result)

	// expect the empty-dir to be second to last in path
	result = isConfigMap(filepath.Join(path, "bazzzzz"))
	assert.False(result)

}
func TestIsSecret(t *testing.T) {
	assert := assert.New(t)
	path := "/var/lib/kubelet/pods/5f0861a0-a987-4a3a-bb0f-1058ddb9678f/volumes/kubernetes.io~secret"
	result := isSecret(path)
	assert.False(result)

	// expect the empty-dir to be second to last in path
	result = isSecret(filepath.Join(path, "sweet-token"))
	assert.True(result)

	result = isConfigMap(filepath.Join(path, "sweet-token-dir", "whoops"))
	assert.False(result)
}

func TestIsWatchable(t *testing.T) {
	if os.Getuid() != 0 {
		t.Skip("Test disabled as requires root user")
	}

	assert := assert.New(t)

	path := ""
	result := isWatchableMount(path)
	assert.False(result)

	// path does not exist, failure expected:
	path = "/var/lib/kubelet/pods/5f0861a0-a987-4a3a-bb0f-1058ddb9678f/volumes/kubernetes.io~empty-dir/foobar"
	result = isWatchableMount(path)
	assert.False(result)

	testPath := t.TempDir()

	// Verify secret is successful (single file mount):
	//   /tmppath/kubernetes.io~secret/super-secret-thing
	secretpath := filepath.Join(testPath, K8sSecret)
	err := os.MkdirAll(secretpath, 0777)
	assert.NoError(err)
	secret := filepath.Join(secretpath, "super-secret-thing")
	_, err = os.Create(secret)
	assert.NoError(err)
	result = isWatchableMount(secret)
	assert.True(result)

	// Verify that if we have too many files, it will no longer be watchable:
	// /tmp/kubernetes.io~configmap/amazing-dir-of-configs/
	//                                  | - c0
	//                                  | - c1
	//                                    ...
	//                                  | - c7
	// should be okay.
	//
	// 9 files should cause the mount to be deemed "not watchable"
	configs := filepath.Join(testPath, K8sConfigMap, "amazing-dir-of-configs")
	err = os.MkdirAll(configs, 0777)
	assert.NoError(err)

	for i := 0; i < 8; i++ {
		_, err := os.Create(filepath.Join(configs, fmt.Sprintf("c%v", i)))
		assert.NoError(err)
		result = isWatchableMount(configs)
		assert.True(result)
	}
	_, err = os.Create(filepath.Join(configs, "toomuch"))
	assert.NoError(err)
	result = isWatchableMount(configs)
	assert.False(result)
}

func TestParseKubeletVolumeSubpathsMountPoint(t *testing.T) {
	tests := []struct {
		name    string
		p       string
		wantVol string
		wantOK  bool
	}{
		{
			name:   "no-volume-subpaths",
			p:      filepath.Join("var", "lib", "kubelet", "pods", "uid", "volumes", "kubernetes.io~empty-dir", "vol"),
			wantOK: false,
		},
		{
			name:    "basic",
			p:       filepath.Join("var", "lib", "kubelet", "pods", "uid", "volume-subpaths", "myvol", "myctr", "0"),
			wantVol: "myvol",
			wantOK:  true,
		},
		{
			name:   "marker-at-end-fast-path-miss",
			p:      filepath.Join("var", "lib", "kubelet", "pods", "uid", "volume-subpaths"),
			wantOK: false,
		},
		{
			name:   "substring-does-not-match",
			p:      filepath.Join("a", "volume-subpathsX", "myvol", "c", "0"),
			wantOK: false,
		},
		{
			name:    "multiple-occurrences-first-wins",
			p:       filepath.Join("a", "volume-subpaths", "v1", "c", "0", "b", "volume-subpaths", "v2", "d", "0"),
			wantVol: "v1",
			wantOK:  true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			gotVol, gotOK := parseKubeletVolumeSubpathsMountPoint(tt.p)
			if gotVol != tt.wantVol || gotOK != tt.wantOK {
				t.Fatalf("parseKubeletVolumeSubpathsMountPoint(%q) = (%q, %v), want (%q, %v)",
					tt.p, gotVol, gotOK, tt.wantVol, tt.wantOK)
			}
		})
	}
}

func TestSplitKubeletEmptyDirTarget(t *testing.T) {
	tmp := t.TempDir()
	vol := "cache"
	volRoot := filepath.Join(tmp, "pods", "uid", "volumes", "kubernetes.io~empty-dir", vol)

	tests := []struct {
		name        string
		targetAbs   string
		volumeName  string
		wantVolRoot string
		wantSubRel  string
		wantOK      bool
	}{
		{
			name:       "not-abs",
			targetAbs:  filepath.Join("relative", "volumes", "kubernetes.io~empty-dir", vol),
			volumeName: vol,
			wantOK:     false,
		},
		{
			name:        "volume-root-only",
			targetAbs:   volRoot,
			volumeName:  vol,
			wantVolRoot: volRoot,
			wantSubRel:  "",
			wantOK:      true,
		},
		{
			name:        "with-subpath",
			targetAbs:   filepath.Join(volRoot, "a", "b", "c"),
			volumeName:  vol,
			wantVolRoot: volRoot,
			wantSubRel:  filepath.Join("a", "b", "c"),
			wantOK:      true,
		},
		{
			name:       "wrong-volume-name",
			targetAbs:  filepath.Join(volRoot, "a"),
			volumeName: "other",
			wantOK:     false,
		},
		{
			name:       "not-emptydir-volume-type",
			targetAbs:  filepath.Join(tmp, "pods", "uid", "volumes", "kubernetes.io~secret", vol, "a"),
			volumeName: vol,
			wantOK:     false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			gotVolRoot, gotSubRel, gotOK := splitKubeletEmptyDirTarget(tt.targetAbs, tt.volumeName)
			if gotVolRoot != tt.wantVolRoot || gotSubRel != tt.wantSubRel || gotOK != tt.wantOK {
				t.Fatalf("splitKubeletEmptyDirTarget(%q, %q) = (%q, %q, %v), want (%q, %q, %v)",
					tt.targetAbs, tt.volumeName,
					gotVolRoot, gotSubRel, gotOK,
					tt.wantVolRoot, tt.wantSubRel, tt.wantOK)
			}
		})
	}
}

func TestSanitizeSubRel(t *testing.T) {
	sep := string(filepath.Separator)
	tests := []struct {
		name    string
		in      string
		want    string
		wantErr bool
	}{
		{name: "empty", in: "", want: "", wantErr: false},
		{name: "dot", in: ".", want: "", wantErr: false},
		{name: "simple", in: "a" + sep + "b", want: filepath.Join("a", "b"), wantErr: false},
		{name: "cleans-to-root", in: "a" + sep + "..", want: "", wantErr: false},
		{name: "cleans-path", in: "a" + sep + "b" + sep + ".." + sep + "c", want: filepath.Join("a", "c"), wantErr: false},

		{name: "absolute", in: sep + "etc", wantErr: true},
		{name: "traversal-only", in: "..", wantErr: true},
		{name: "traversal-prefix", in: ".." + sep + "a", wantErr: true},
		{name: "traversal-after-clean", in: "a" + sep + ".." + sep + ".." + sep + "b", wantErr: true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got, err := sanitizeSubRel(tt.in)
			if tt.wantErr {
				if err == nil {
					t.Fatalf("sanitizeSubRel(%q) expected error, got nil (got=%q)", tt.in, got)
				}
				return
			}
			if err != nil {
				t.Fatalf("sanitizeSubRel(%q) unexpected error: %v", tt.in, err)
			}
			if got != tt.want {
				t.Fatalf("sanitizeSubRel(%q) = %q, want %q", tt.in, got, tt.want)
			}
		})
	}
}

func TestUnescapeMountinfoField(t *testing.T) {
	tests := []struct {
		in   string
		want string
	}{
		{in: "foo\\040bar", want: "foo bar"},
		{in: "foo\\011bar", want: "foo\tbar"},
		{in: "foo\\012bar", want: "foo\nbar"},
		{in: "foo\\134bar", want: "foo\\bar"},
		{in: "bad\\x20", want: "bad\\x20"},
		{in: "trail\\", want: "trail\\"},
	}
	for _, tt := range tests {
		t.Run(tt.in, func(t *testing.T) {
			got := unescapeMountinfoField(tt.in)
			if got != tt.want {
				t.Fatalf("unescapeMountinfoField(%q) = %q, want %q", tt.in, got, tt.want)
			}
		})
	}
}

func TestFindMountSourceForMountPointFromData_MatchWithSymlinkAndEscapes(t *testing.T) {
	tmp := t.TempDir()

	mp := filepath.Join(tmp, "mp with space")
	if err := os.MkdirAll(mp, 0o755); err != nil {
		t.Fatalf("MkdirAll: %v", err)
	}
	mpLink := filepath.Join(tmp, "mp-link")
	if err := os.Symlink(mp, mpLink); err != nil {
		t.Fatalf("Symlink: %v", err)
	}

	sep := string(filepath.Separator)
	mountpointInMountinfo := mp + sep + "."
	root := filepath.Join(tmp, "src with space")

	line := "36 25 0:32 " + escapeMountinfoToken(root) + " " + escapeMountinfoToken(mountpointInMountinfo) + " rw - ext4 /dev/sda1 rw\n"
	got, err := findMountSourceForMountPointFromData(mpLink, []byte(line))
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if got != root {
		t.Fatalf("got source %q, want %q", got, root)
	}
}

func TestFindMountSourceForMountPointFromData_NotFound(t *testing.T) {
	tmp := t.TempDir()
	mp := filepath.Join(tmp, "mp")
	if err := os.MkdirAll(mp, 0o755); err != nil {
		t.Fatalf("MkdirAll: %v", err)
	}

	data := []byte("36 25 0:32 /src /somewhere rw - ext4 /dev/sda1 rw\n")
	_, err := findMountSourceForMountPointFromData(mp, data)
	if err == nil {
		t.Fatalf("expected error, got nil")
	}
}

func TestGetKubeletEmptyDirSubpathInfo_WithStubs(t *testing.T) {
	origFind := findMountSourceForMountPointFn
	origResolve := resolveMountSourcePathFn
	t.Cleanup(func() {
		findMountSourceForMountPointFn = origFind
		resolveMountSourcePathFn = origResolve
	})

	tmp := t.TempDir()
	vol := "vol1"

	mp := filepath.Join(tmp, "pods", "uid", "volume-subpaths", vol, "ctr", "0")
	target := filepath.Join(tmp, "pods", "uid", "volumes", "kubernetes.io~empty-dir", vol, "app-logs")

	findMountSourceForMountPointFn = func(mountPoint string) (string, error) {
		if mountPoint != mp {
			t.Fatalf("findMountSourceForMountPointFn got mountPoint=%q, want %q", mountPoint, mp)
		}
		return "/proc/123/fd/9", nil
	}
	resolveMountSourcePathFn = func(source string) (string, error) {
		if source != "/proc/123/fd/9" {
			t.Fatalf("resolveMountSourcePathFn got source=%q, want %q", source, "/proc/123/fd/9")
		}
		return target, nil
	}

	info := GetKubeletEmptyDirSubpathInfo(mp)
	if info == nil {
		t.Fatalf("expected non-nil info")
	}
	if info.VolumeName != vol {
		t.Fatalf("VolumeName=%q, want %q", info.VolumeName, vol)
	}
	if info.SubPath != "app-logs" {
		t.Fatalf("SubPath=%q, want %q", info.SubPath, "app-logs")
	}
	if info.TargetPath != target {
		t.Fatalf("TargetPath=%q, want %q", info.TargetPath, target)
	}

	ok := IsKubeletEmptyDirSubpath(mp)
	if !ok {
		t.Fatalf("IsKubeletEmptyDirSubpath=%v, want true", ok)
	}
}

func TestGetKubeletEmptyDirSubpathInfo_NotEmptyDir(t *testing.T) {
	origFind := findMountSourceForMountPointFn
	origResolve := resolveMountSourcePathFn
	t.Cleanup(func() {
		findMountSourceForMountPointFn = origFind
		resolveMountSourcePathFn = origResolve
	})

	tmp := t.TempDir()
	vol := "vol1"
	mp := filepath.Join(tmp, "pods", "uid", "volume-subpaths", vol, "ctr", "0")
	target := filepath.Join(tmp, "pods", "uid", "volumes", "kubernetes.io~secret", vol, "app-logs")

	findMountSourceForMountPointFn = func(string) (string, error) { return "src", nil }
	resolveMountSourcePathFn = func(string) (string, error) { return target, nil }

	if info := GetKubeletEmptyDirSubpathInfo(mp); info != nil {
		t.Fatalf("expected nil info, got %#v", info)
	}
}

func TestGetKubeletEmptyDirSubpathInfo_SourceLookupError(t *testing.T) {
	origFind := findMountSourceForMountPointFn
	origResolve := resolveMountSourcePathFn
	t.Cleanup(func() {
		findMountSourceForMountPointFn = origFind
		resolveMountSourcePathFn = origResolve
	})

	tmp := t.TempDir()
	mp := filepath.Join(tmp, "pods", "uid", "volume-subpaths", "vol1", "ctr", "0")

	findMountSourceForMountPointFn = func(string) (string, error) { return "", errors.New("boom") }
	resolveMountSourcePathFn = func(string) (string, error) {
		t.Fatalf("resolve should not be called on find error")
		return "", nil
	}

	if info := GetKubeletEmptyDirSubpathInfo(mp); info != nil {
		t.Fatalf("expected nil info, got %#v", info)
	}
}

func TestGetKubeletEmptyDirSubpathInfo_ResolveError(t *testing.T) {
	origFind := findMountSourceForMountPointFn
	origResolve := resolveMountSourcePathFn
	t.Cleanup(func() {
		findMountSourceForMountPointFn = origFind
		resolveMountSourcePathFn = origResolve
	})

	tmp := t.TempDir()
	mp := filepath.Join(tmp, "pods", "uid", "volume-subpaths", "vol1", "ctr", "0")

	findMountSourceForMountPointFn = func(string) (string, error) { return "src", nil }
	resolveMountSourcePathFn = func(string) (string, error) { return "", errors.New("boom") }

	if info := GetKubeletEmptyDirSubpathInfo(mp); info != nil {
		t.Fatalf("expected nil info, got %#v", info)
	}
}

func TestGetKubeletEmptyDirSubpathInfo_NoVolumeSubpaths_DoesNotCallStubs(t *testing.T) {
	origFind := findMountSourceForMountPointFn
	origResolve := resolveMountSourcePathFn
	t.Cleanup(func() {
		findMountSourceForMountPointFn = origFind
		resolveMountSourcePathFn = origResolve
	})

	findMountSourceForMountPointFn = func(string) (string, error) {
		t.Fatalf("should not be called")
		return "", nil
	}
	resolveMountSourcePathFn = func(string) (string, error) {
		t.Fatalf("should not be called")
		return "", nil
	}

	if info := GetKubeletEmptyDirSubpathInfo(filepath.Join("not", "a", "subpath")); info != nil {
		t.Fatalf("expected nil, got %#v", info)
	}
}

func escapeMountinfoToken(s string) string {
	r := strings.NewReplacer(
		"\\", "\\134",
		" ", "\\040",
		"\t", "\\011",
		"\n", "\\012",
	)
	return r.Replace(s)
}

func TestResolveMountSourcePath_ProcSelfFD(t *testing.T) {
	tmp := t.TempDir()
	f, err := os.CreateTemp(tmp, "src-*")
	if err != nil {
		t.Fatalf("CreateTemp: %v", err)
	}
	defer f.Close()

	source := fmt.Sprintf("/proc/self/fd/%d", f.Fd())
	got, err := resolveMountSourcePath(source)
	if err != nil {
		t.Fatalf("resolveMountSourcePath: %v", err)
	}
	want := filepath.Clean(f.Name())
	if got != want {
		t.Fatalf("got %q, want %q", got, want)
	}
}

func TestResolveMountSourcePath_ProcSelfFD_DeletedSuffixTrim(t *testing.T) {
	tmp := t.TempDir()
	f, err := os.CreateTemp(tmp, "src-*")
	if err != nil {
		t.Fatalf("CreateTemp: %v", err)
	}
	name := f.Name()
	defer f.Close()

	if err := os.Remove(name); err != nil {
		t.Fatalf("Remove: %v", err)
	}

	source := fmt.Sprintf("/proc/self/fd/%d", f.Fd())
	got, err := resolveMountSourcePath(source)
	if err != nil {
		t.Fatalf("resolveMountSourcePath: %v", err)
	}
	want := filepath.Clean(name)
	if got != want {
		t.Fatalf("got %q, want %q", got, want)
	}
}

func TestResolveMountSourcePath_NonProcPath(t *testing.T) {
	sep := string(filepath.Separator)
	in := "a" + sep + ".." + sep + "b"
	got, err := resolveMountSourcePath(in)
	if err != nil {
		t.Fatalf("resolveMountSourcePath: %v", err)
	}
	if got != "b" {
		t.Fatalf("got %q, want %q", got, "b")
	}
}
