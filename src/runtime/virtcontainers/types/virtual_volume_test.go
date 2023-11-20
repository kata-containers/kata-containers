package types

import (
	"encoding/base64"
	"encoding/json"
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestDmVerityInfoValidation(t *testing.T) {
	TestData := []DmVerityInfo{
		{
			HashType:  "md5", // "md5" is not a supported hash algorithm
			Blocksize: 512,
			Hashsize:  512,
			BlockNum:  16384,
			Offset:    8388608,
			Hash:      "9de18652fe74edfb9b805aaed72ae2aa48f94333f1ba5c452ac33b1c39325174",
		},
		{
			HashType:  "sha256",
			Blocksize: 3000, // Invalid block size, not a power of 2.
			Hashsize:  512,
			BlockNum:  16384,
			Offset:    8388608,
			Hash:      "9de18652fe74edfb9b805aaed72ae2aa48f94333f1ba5c452ac33b1c39325174",
		},
		{
			HashType:  "sha256",
			Blocksize: 0, // Invalid block size, less than 512.
			Hashsize:  512,
			BlockNum:  16384,
			Offset:    8388608,
			Hash:      "9de18652fe74edfb9b805aaed72ae2aa48f94333f1ba5c452ac33b1c39325174",
		},
		{
			HashType:  "sha256",
			Blocksize: 524800, // Invalid block size, greater than 524288.
			Hashsize:  512,
			BlockNum:  16384,
			Offset:    8388608,
			Hash:      "9de18652fe74edfb9b805aaed72ae2aa48f94333f1ba5c452ac33b1c39325174",
		},
		{
			HashType:  "sha256",
			Blocksize: 512,
			Hashsize:  3000, // Invalid hash block size, not a power of 2.
			BlockNum:  16384,
			Offset:    8388608,
			Hash:      "9de18652fe74edfb9b805aaed72ae2aa48f94333f1ba5c452ac33b1c39325174",
		},
		{
			HashType:  "sha256",
			Blocksize: 512,
			Hashsize:  0, // Invalid hash block size, less than 512.
			BlockNum:  16384,
			Offset:    8388608,
			Hash:      "9de18652fe74edfb9b805aaed72ae2aa48f94333f1ba5c452ac33b1c39325174",
		},
		{
			HashType:  "sha256",
			Blocksize: 512,
			Hashsize:  524800, // Invalid hash block size, greater than 524288.
			BlockNum:  16384,
			Offset:    8388608,
			Hash:      "9de18652fe74edfb9b805aaed72ae2aa48f94333f1ba5c452ac33b1c39325174",
		},
		{
			HashType:  "sha256",
			Blocksize: 512,
			Hashsize:  512,
			BlockNum:  0, // Invalid BlockNum, it must be greater than 0.
			Offset:    8388608,
			Hash:      "9de18652fe74edfb9b805aaed72ae2aa48f94333f1ba5c452ac33b1c39325174",
		},
		{
			HashType:  "sha256",
			Blocksize: 512,
			Hashsize:  512,
			BlockNum:  16384,
			Offset:    0, // Invalid offset, it must be greater than 0.
			Hash:      "9de18652fe74edfb9b805aaed72ae2aa48f94333f1ba5c452ac33b1c39325174",
		},
		{
			HashType:  "sha256",
			Blocksize: 512,
			Hashsize:  512,
			BlockNum:  16384,
			Offset:    8193, // Invalid offset, it must be aligned to 512.
			Hash:      "9de18652fe74edfb9b805aaed72ae2aa48f94333f1ba5c452ac33b1c39325174",
		},
		{
			HashType:  "sha256",
			Blocksize: 512,
			Hashsize:  512,
			BlockNum:  16384,
			Offset:    8388608 - 4096, // Invalid offset, it must be equal to blocksize * BlockNum.
			Hash:      "9de18652fe74edfb9b805aaed72ae2aa48f94333f1ba5c452ac33b1c39325174",
		},
	}

	for _, d := range TestData {
		assert.Error(t, d.IsValid())
	}
	TestCorrectData := DmVerityInfo{
		HashType:  "sha256",
		Blocksize: 512,
		Hashsize:  512,
		BlockNum:  16384,
		Offset:    8388608,
		Hash:      "9de18652fe74edfb9b805aaed72ae2aa48f94333f1ba5c452ac33b1c39325174",
	}
	assert.NoError(t, TestCorrectData.IsValid())
}

func TestDirectAssignedVolumeValidation(t *testing.T) {
	validDirectVolume := DirectAssignedVolume{
		Metadata: map[string]string{"key": "value"},
	}
	assert.True(t, validDirectVolume.IsValid())

	invalidDirectVolume := DirectAssignedVolume{
		Metadata: nil,
	}
	assert.False(t, invalidDirectVolume.IsValid())
}

func TestImagePullVolumeValidation(t *testing.T) {
	validImagePull := ImagePullVolume{
		Metadata: map[string]string{"key": "value"},
	}
	assert.True(t, validImagePull.IsValid())

	invalidImagePull := ImagePullVolume{
		Metadata: nil,
	}
	assert.False(t, invalidImagePull.IsValid())
}

func TestNydusImageVolumeValidation(t *testing.T) {
	validNydusImage := NydusImageVolume{
		Config:      "config_value",
		SnapshotDir: "",
	}
	assert.True(t, validNydusImage.IsValid())

	invalidNydusImage := NydusImageVolume{
		Config:      "",
		SnapshotDir: "",
	}
	assert.False(t, invalidNydusImage.IsValid())
}

func TestKataVirtualVolumeValidation(t *testing.T) {
	validKataVirtualVolume := KataVirtualVolume{
		VolumeType: "direct_block",
		Source:     "/dev/sdb",
		FSType:     "ext4",
		Options:    []string{"rw"},
		DirectVolume: &DirectAssignedVolume{
			Metadata: map[string]string{"key": "value"},
		},
		// Initialize other fields
	}
	assert.True(t, validKataVirtualVolume.IsValid())

	invalidKataVirtualVolume := KataVirtualVolume{
		VolumeType: "direct_block",
		Source:     "/dev/sdb",
		FSType:     "",
		Options:    nil,
		DirectVolume: &DirectAssignedVolume{
			Metadata: nil,
		},
		// Initialize other fields
	}
	assert.False(t, invalidKataVirtualVolume.IsValid())
}

func TestParseDmVerityInfo(t *testing.T) {
	// Create a mock valid KataVirtualVolume
	validDmVerityInfo := DmVerityInfo{
		HashType:  "sha256",
		Blocksize: 512,
		Hashsize:  512,
		BlockNum:  16384,
		Offset:    8388608,
		Hash:      "9de18652fe74edfb9b805aaed72ae2aa48f94333f1ba5c452ac33b1c39325174",
	}
	validKataVirtualVolumeJSON, _ := json.Marshal(validDmVerityInfo)

	t.Run("Valid Option", func(t *testing.T) {
		volume, err := ParseDmVerityInfo(string(validKataVirtualVolumeJSON))
		assert.NoError(t, err)
		assert.NotNil(t, volume)
		assert.NoError(t, volume.IsValid())
	})

	t.Run("Invalid JSON Option", func(t *testing.T) {
		volume, err := ParseDmVerityInfo("invalid_json")
		assert.Error(t, err)
		assert.Nil(t, volume)
	})

}

func TestParseKataVirtualVolume(t *testing.T) {
	// Create a mock valid KataVirtualVolume
	validKataVirtualVolume := KataVirtualVolume{
		VolumeType: "direct_block",
		Source:     "/dev/sdb",
		FSType:     "ext4",
		Options:    []string{"rw"},
		DirectVolume: &DirectAssignedVolume{
			Metadata: map[string]string{"key": "value"},
		},
		// Initialize other fields
	}
	validKataVirtualVolumeJSON, _ := json.Marshal(validKataVirtualVolume)
	validOption := base64.StdEncoding.EncodeToString(validKataVirtualVolumeJSON)

	t.Run("Valid Option", func(t *testing.T) {
		volume, err := ParseKataVirtualVolume(validOption)

		assert.NoError(t, err)
		assert.NotNil(t, volume)
		assert.True(t, volume.IsValid())
	})

	t.Run("Invalid JSON Option", func(t *testing.T) {
		invalidJSONOption := base64.StdEncoding.EncodeToString([]byte("invalid_json"))
		volume, err := ParseKataVirtualVolume(invalidJSONOption)

		assert.Error(t, err)
		assert.Nil(t, volume)
	})

	invalidBase64Option := "invalid_base64"
	t.Run("Invalid Base64 Option", func(t *testing.T) {
		volume, err := ParseKataVirtualVolume(invalidBase64Option)

		assert.Error(t, err)
		assert.Nil(t, volume)
	})
}
