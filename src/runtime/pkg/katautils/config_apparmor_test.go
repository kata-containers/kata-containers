// Copyright (c) 2026 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0

package katautils

import (
	"errors"
	"os"
	"path/filepath"
	"strings"
	"syscall"
	"testing"
)

func TestToGuestExtensionImagesAppArmorPaths(t *testing.T) {
	dir := t.TempDir()
	image := filepath.Join(dir, "apparmor.img")
	if err := os.WriteFile(image, []byte("policy image"), 0600); err != nil {
		t.Fatal(err)
	}
	link := filepath.Join(dir, "image-link")
	if err := os.Symlink(image, link); err != nil {
		t.Fatal(err)
	}
	dangling := filepath.Join(dir, "dangling")
	if err := os.Symlink(filepath.Join(dir, "missing"), dangling); err != nil {
		t.Fatal(err)
	}
	fifo := filepath.Join(dir, "fifo")
	if err := syscall.Mkfifo(fifo, 0600); err != nil {
		t.Fatal(err)
	}

	for _, tc := range []struct {
		name string
		path string
		fail bool
	}{
		{name: "missing path", fail: true},
		{name: "whitespace", path: " \t", fail: true},
		{name: "nonexistent", path: filepath.Join(dir, "missing"), fail: true},
		{name: "invalid", path: "bad\x00path", fail: true},
		{name: "directory", path: dir, fail: true},
		{name: "fifo", path: fifo, fail: true},
		{name: "dangling symlink", path: dangling, fail: true},
		{name: "regular file", path: image},
		{name: "symlink", path: link},
	} {
		t.Run(tc.name, func(t *testing.T) {
			got, err := toGuestExtensionImages([]guestExtensionImage{{Name: "apparmor", Path: tc.path}})
			if tc.fail {
				if err == nil || !strings.Contains(err.Error(), "guest_extension_images 'apparmor' has invalid path") {
					t.Fatalf("expected explicit AppArmor path error, got images=%+v, err=%v", got, err)
				}
				if got != nil {
					t.Fatalf("invalid AppArmor configuration returned images: %+v", got)
				}
				return
			}
			if err != nil || len(got) != 1 || got[0].Name != "apparmor" || got[0].Path != tc.path {
				t.Fatalf("valid AppArmor image changed: images=%+v, err=%v", got, err)
			}
		})
	}
}

func TestToGuestExtensionImagesAppArmorUnreadable(t *testing.T) {
	if os.Geteuid() == 0 {
		t.Skip("root can read mode-000 files")
	}
	image := filepath.Join(t.TempDir(), "unreadable.img")
	if err := os.WriteFile(image, nil, 0000); err != nil {
		t.Fatal(err)
	}
	_, err := toGuestExtensionImages([]guestExtensionImage{{Name: "apparmor", Path: image}})
	if err == nil || !strings.Contains(err.Error(), "apparmor") || !errors.Is(err, os.ErrPermission) {
		t.Fatalf("expected AppArmor permission error, got %v", err)
	}
}

func TestToGuestExtensionImagesCompatibility(t *testing.T) {
	for _, imgs := range [][]guestExtensionImage{nil, {}} {
		got, err := toGuestExtensionImages(imgs)
		if err != nil || got != nil {
			t.Fatalf("unconfigured baseline changed: images=%+v, err=%v", got, err)
		}
	}
	// Existing extensions retain empty and nonexistent paths, and empty verity
	// parameters. Only an explicit AppArmor entry requires a readable image.
	imgs := []guestExtensionImage{
		{Name: "coco"},
		{Name: "gpu", Path: filepath.Join(t.TempDir(), "not-installed.img")},
	}
	got, err := toGuestExtensionImages(imgs)
	if err != nil || len(got) != len(imgs) {
		t.Fatalf("other extensions changed: images=%+v, err=%v", got, err)
	}
	for i, img := range imgs {
		if got[i].Name != img.Name || got[i].Path != img.Path || got[i].VerityParams != img.VerityParams {
			t.Fatalf("extension changed: got=%+v, want=%+v", got[i], img)
		}
	}
	for _, img := range []guestExtensionImage{
		{},
		{Name: "bad name"},
		{Name: "coco", VerityParams: "invalid"},
	} {
		if _, err := toGuestExtensionImages([]guestExtensionImage{img}); err == nil {
			t.Fatalf("existing name/verity validation was bypassed: %+v", img)
		}
	}
}
