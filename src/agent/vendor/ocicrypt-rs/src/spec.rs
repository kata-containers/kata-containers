// Copyright The ocicrypt Authors.
// SPDX-License-Identifier: Apache-2.0

/// MEDIA_TYPE_LAYER_ENC is MIME type used for encrypted layers.
pub const MEDIA_TYPE_LAYER_ENC: &str = "application/vnd.oci.image.layer.v1.tar+encrypted";

/// MEDIA_TYPE_LAYER_GZIP_ENC is MIME type used for encrypted compressed layers.
pub const MEDIA_TYPE_LAYER_GZIP_ENC: &str = "application/vnd.oci.image.layer.v1.tar+gzip+encrypted";

/// MEDIA_TYPE_LAYER_NON_DISTRIBUTABLE_ENC is MIME type used for non distributable encrypted layers.
pub const MEDIA_TYPE_LAYER_NON_DISTRIBUTABLE_ENC: &str =
    "application/vnd.oci.image.layer.nondistributable.v1.tar+encrypted";

/// MEDIA_TYPE_LAYER_NON_DISTRIBUTABLE_GZIP_ENC is MIME type used for non distributable encrypted compressed layers.
pub const MEDIA_TYPE_LAYER_NON_DISTRIBUTABLE_GZIP_ENC: &str =
    "application/vnd.oci.image.layer.nondistributable.v1.tar+gzip+encrypted";
