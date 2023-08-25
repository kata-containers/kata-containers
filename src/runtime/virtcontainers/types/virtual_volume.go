package types

import (
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"strings"

	"github.com/pkg/errors"
)

const (
	minBlockSize = 1 << 9
	maxBlockSize = 1 << 19
)

const (
	KataVirtualVolumeDirectBlockType     = "direct_block"
	KataVirtualVolumeImageRawBlockType   = "image_raw_block"
	KataVirtualVolumeLayerRawBlockType   = "layer_raw_block"
	KataVirtualVolumeImageNydusBlockType = "image_nydus_block"
	KataVirtualVolumeLayerNydusBlockType = "layer_nydus_block"
	KataVirtualVolumeImageNydusFsType    = "image_nydus_fs"
	KataVirtualVolumeLayerNydusFsType    = "layer_nydus_fs"
	KataVirtualVolumeImageGuestPullType  = "image_guest_pull"
)

// DmVerityInfo contains configuration information for DmVerity device.
type DmVerityInfo struct {
	HashType  string `json:"hashtype"`
	Hash      string `json:"hash"`
	BlockNum  uint64 `json:"blocknum"`
	Blocksize uint64 `json:"blocksize"`
	Hashsize  uint64 `json:"hashsize"`
	Offset    uint64 `json:"offset"`
}

// DirectAssignedVolume contains meta information for a directly assigned volume.
type DirectAssignedVolume struct {
	Metadata map[string]string `json:"metadata"`
}

// ImagePullVolume contains meta information for pulling an image inside the guest.
type ImagePullVolume struct {
	Metadata map[string]string `json:"metadata"`
}

// NydusImageVolume contains Nydus image volume information.
type NydusImageVolume struct {
	Config      string `json:"config"`
	SnapshotDir string `json:"snapshot_dir"`
}

// KataVirtualVolume encapsulates information for extra mount options and direct volumes.
type KataVirtualVolume struct {
	VolumeType   string                `json:"volume_type"`
	Source       string                `json:"source,omitempty"`
	FSType       string                `json:"fs_type,omitempty"`
	Options      []string              `json:"options,omitempty"`
	DirectVolume *DirectAssignedVolume `json:"direct_volume,omitempty"`
	ImagePull    *ImagePullVolume      `json:"image_pull,omitempty"`
	NydusImage   *NydusImageVolume     `json:"nydus_image,omitempty"`
	DmVerity     *DmVerityInfo         `json:"dm_verity,omitempty"`
}

func (d *DmVerityInfo) IsValid() error {
	err := d.validateHashType()
	if err != nil {
		return err
	}

	if d.BlockNum == 0 || d.BlockNum > uint64(^uint32(0)) {
		return fmt.Errorf("Zero block count for DmVerity device %s", d.Hash)
	}

	if !isValidBlockSize(d.Blocksize) || !isValidBlockSize(d.Hashsize) {
		return fmt.Errorf("Unsupported verity block size: data_block_size = %d, hash_block_size = %d", d.Blocksize, d.Hashsize)
	}

	if d.Offset%d.Hashsize != 0 || d.Offset < d.Blocksize*d.BlockNum {
		return fmt.Errorf("Invalid hashvalue offset %d for DmVerity device %s", d.Offset, d.Hash)
	}

	return nil
}

func (d *DirectAssignedVolume) IsValid() bool {
	return d.Metadata != nil
}

func (i *ImagePullVolume) IsValid() bool {
	return i.Metadata != nil
}

func (n *NydusImageVolume) IsValid() bool {
	return len(n.Config) > 0 || len(n.SnapshotDir) > 0
}

func (k *KataVirtualVolume) IsValid() bool {
	return len(k.VolumeType) > 0 &&
		(k.DirectVolume == nil || k.DirectVolume.IsValid()) &&
		(k.ImagePull == nil || k.ImagePull.IsValid()) &&
		(k.NydusImage == nil || k.NydusImage.IsValid()) &&
		(k.DmVerity == nil || k.DmVerity.IsValid() == nil)
}

func (d *DmVerityInfo) validateHashType() error {
	switch strings.ToLower(d.HashType) {
	case "sha256":
		return d.isValidHash(64, "sha256")
	case "sha1":
		return d.isValidHash(40, "sha1")
	default:
		return fmt.Errorf("Unsupported hash algorithm %s for DmVerity device %s", d.HashType, d.Hash)
	}
}

func isValidBlockSize(blockSize uint64) bool {
	return minBlockSize <= blockSize && blockSize <= maxBlockSize
}

func (d *DmVerityInfo) isValidHash(expectedLen int, hashType string) error {
	_, err := hex.DecodeString(d.Hash)
	if len(d.Hash) != expectedLen || err != nil {
		return fmt.Errorf("Invalid hash value %s:%s for DmVerity device with %s", hashType, d.Hash, hashType)
	}
	return nil
}

func ParseDmVerityInfo(option string) (*DmVerityInfo, error) {
	no := &DmVerityInfo{}
	if err := json.Unmarshal([]byte(option), no); err != nil {
		return nil, errors.Wrapf(err, "DmVerityInfo json unmarshal err")
	}
	if err := no.IsValid(); err != nil {
		return nil, fmt.Errorf("DmVerityInfo is not correct, %+v; error = %+v", no, err)
	}
	return no, nil
}

func ParseKataVirtualVolume(option string) (*KataVirtualVolume, error) {
	opt, err := base64.StdEncoding.DecodeString(option)
	if err != nil {
		return nil, errors.Wrap(err, "KataVirtualVolume base64 decoding err")
	}
	no := &KataVirtualVolume{}
	if err := json.Unmarshal(opt, no); err != nil {
		return nil, errors.Wrapf(err, "KataVirtualVolume json unmarshal err")
	}
	if !no.IsValid() {
		return nil, fmt.Errorf("KataVirtualVolume is not correct, %+v", no)
	}

	return no, nil
}
