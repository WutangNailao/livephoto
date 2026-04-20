use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::media::{PhotoFormat, VideoFormat};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InteractionHint {
    Tap,
    Hover,
    Press,
    Viewport,
    Programmatic,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PosterStrategy {
    Explicit,
    VideoFrame,
    Generated,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AlphaMode {
    None,
    Straight,
    Premultiplied,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExifFormat {
    Raw,
    Json,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlaybackPolicyV1 {
    pub autoplay: bool,
    #[serde(rename = "loop")]
    pub loop_: bool,
    pub bounce: bool,
    pub muted_by_default: bool,
    pub return_to_cover: bool,
    #[serde(default)]
    pub hold_last_frame: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interaction_hint: Option<InteractionHint>,
}

impl PlaybackPolicyV1 {
    pub fn new() -> Self {
        Self {
            autoplay: false,
            loop_: false,
            bounce: false,
            muted_by_default: false,
            return_to_cover: true,
            hold_last_frame: false,
            interaction_hint: None,
        }
    }
}

impl Default for PlaybackPolicyV1 {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BridgeDescriptorV1 {
    pub target: String,
    pub chunk_id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ManifestV1 {
    pub schema: String,
    pub asset_id: String,
    pub created_at_ms: u64,
    pub duration_ms: u64,
    pub width: u32,
    pub height: u32,
    pub cover_timestamp_ms: u64,
    pub photo_chunk_id: u64,
    pub video_chunk_id: u64,
    pub photo_mime: String,
    pub video_mime: String,
    pub has_audio: bool,
    pub playback: PlaybackPolicyV1,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotation_degrees: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail_chunk_id: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exif_chunk_id: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xmp_chunk_id: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apple_bridge_chunk_id: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub android_bridge_chunk_id: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bridges: Vec<BridgeDescriptorV1>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capture_device: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub software: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_space: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alpha_mode: Option<AlphaMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub poster_strategy: Option<PosterStrategy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_seek_pre_roll_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extensions: BTreeMap<String, serde_json::Value>,
}

impl ManifestV1 {
    pub fn validate(&self) -> Result<()> {
        if self.schema != "livephoto/v1" {
            return Err(Error::ManifestValidationFailure(
                "schema must be livephoto/v1".to_string(),
            ));
        }
        if self.asset_id.trim().is_empty() {
            return Err(Error::ManifestValidationFailure(
                "asset_id must not be empty".to_string(),
            ));
        }
        if self.duration_ms == 0 {
            return Err(Error::ManifestValidationFailure(
                "duration_ms must be greater than zero".to_string(),
            ));
        }
        if self.width == 0 || self.height == 0 {
            return Err(Error::ManifestValidationFailure(
                "width and height must be greater than zero".to_string(),
            ));
        }
        if self.cover_timestamp_ms > self.duration_ms {
            return Err(Error::ManifestValidationFailure(
                "cover_timestamp_ms must not exceed duration_ms".to_string(),
            ));
        }
        self.validate_chunk_references()?;
        validate_photo_mime(&self.photo_mime)?;
        validate_video_mime(&self.video_mime)?;
        Ok(())
    }

    pub fn validate_template(&self) -> Result<()> {
        if self.schema != "livephoto/v1" {
            return Err(Error::ManifestValidationFailure(
                "schema must be livephoto/v1".to_string(),
            ));
        }
        if self.asset_id.trim().is_empty() {
            return Err(Error::ManifestValidationFailure(
                "asset_id must not be empty".to_string(),
            ));
        }
        if self.duration_ms == 0 {
            return Err(Error::ManifestValidationFailure(
                "duration_ms must be greater than zero".to_string(),
            ));
        }
        if self.width == 0 || self.height == 0 {
            return Err(Error::ManifestValidationFailure(
                "width and height must be greater than zero".to_string(),
            ));
        }
        if self.cover_timestamp_ms > self.duration_ms {
            return Err(Error::ManifestValidationFailure(
                "cover_timestamp_ms must not exceed duration_ms".to_string(),
            ));
        }
        validate_photo_mime(&self.photo_mime)?;
        validate_video_mime(&self.video_mime)?;
        Ok(())
    }

    fn validate_chunk_references(&self) -> Result<()> {
        if self.photo_chunk_id == self.video_chunk_id {
            return Err(Error::ManifestValidationFailure(
                "photo_chunk_id and video_chunk_id must be different".to_string(),
            ));
        }
        Ok(())
    }
}

fn validate_photo_mime(mime: &str) -> Result<()> {
    if PhotoFormat::from_mime(mime).is_some() {
        Ok(())
    } else {
        Err(Error::UnsupportedCodec(format!(
            "unsupported photo mime: {mime}"
        )))
    }
}

fn validate_video_mime(mime: &str) -> Result<()> {
    if VideoFormat::from_mime(mime).is_some() {
        Ok(())
    } else {
        Err(Error::UnsupportedCodec(format!(
            "unsupported video mime: {mime}"
        )))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppleBridgeV1 {
    pub asset_identifier: String,
    pub still_image_time_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub photo_codec_hint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video_codec_hint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maker_apple_key_17: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quicktime_content_identifier: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AndroidBridgeV1 {
    pub presentation_timestamp_us: u64,
    pub xmp_format: String,
    pub primary_image_role: String,
    pub embedded_video_role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VendorPayloadV1 {
    pub vendor_id: String,
    pub payload: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_manifest(asset_id: &str) -> ManifestV1 {
        ManifestV1 {
            schema: "livephoto/v1".to_string(),
            asset_id: asset_id.to_string(),
            created_at_ms: 1,
            duration_ms: 1500,
            width: 1080,
            height: 1440,
            cover_timestamp_ms: 800,
            photo_chunk_id: 1,
            video_chunk_id: 2,
            photo_mime: "image/jpeg".to_string(),
            video_mime: "video/mp4".to_string(),
            has_audio: true,
            playback: PlaybackPolicyV1::default(),
            ..ManifestV1::default()
        }
    }

    #[test]
    fn accepts_non_uuid_asset_id() {
        let manifest = sample_manifest("asset-demo-01");

        manifest.validate().unwrap();
        manifest.validate_template().unwrap();
    }
}
