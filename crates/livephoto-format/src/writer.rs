use std::collections::BTreeSet;
use std::fs;
use std::io::{Cursor, Seek, SeekFrom};
use std::path::Path;

use crate::chunk::{ChunkEnvelope, ChunkFlags, ChunkKind, TocEntryV1, TocPayloadV1};
use crate::error::{ConformanceIssue, Error, Result};
use crate::manifest::{AndroidBridgeV1, AppleBridgeV1, ManifestV1, VendorPayloadV1};
use crate::reader::LivePhotoFile;
use crate::types::{FileFlags, LpFileHeaderV1};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strictness {
    Strict,
    Recovery,
}

#[derive(Debug, Clone, Copy)]
pub struct ReaderOptions {
    pub strictness: Strictness,
    pub verify_checksums: bool,
    pub max_manifest_size: usize,
    pub max_thumbnail_size: usize,
}

impl Default for ReaderOptions {
    fn default() -> Self {
        Self {
            strictness: Strictness::Strict,
            verify_checksums: true,
            max_manifest_size: 1024 * 1024,
            max_thumbnail_size: 32 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct WriterOptions {
    pub emit_crc32c: bool,
    pub strict_validation: bool,
}

impl Default for WriterOptions {
    fn default() -> Self {
        Self {
            emit_crc32c: true,
            strict_validation: true,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ValidationReport {
    pub issues: Vec<ConformanceIssue>,
}

impl ValidationReport {
    pub fn is_clean(&self) -> bool {
        self.issues.is_empty()
    }
}

#[derive(Debug, Clone)]
pub enum OptionalChunk {
    Thumbnail {
        mime: String,
        bytes: Vec<u8>,
    },
    ExifRaw(Vec<u8>),
    ExifJson(serde_json::Value),
    Xmp(Vec<u8>),
    AppleBridge(AppleBridgeV1),
    AndroidBridge(AndroidBridgeV1),
    Vendor(VendorPayloadV1),
    UnknownJson {
        chunk_type: [u8; 4],
        flags: u64,
        payload: serde_json::Value,
    },
    UnknownBinary {
        chunk_type: [u8; 4],
        flags: u64,
        payload: Vec<u8>,
    },
}

#[derive(Debug, Clone)]
pub struct LivePhotoAsset {
    pub manifest: ManifestV1,
    pub photo: Vec<u8>,
    pub video: Vec<u8>,
    pub optional_chunks: Vec<OptionalChunk>,
}

impl LivePhotoAsset {
    pub fn validate(&self) -> Result<()> {
        self.manifest.validate_template()?;
        if self.photo.is_empty() {
            return Err(Error::ManifestValidationFailure(
                "photo payload must not be empty".to_string(),
            ));
        }
        if self.video.is_empty() {
            return Err(Error::ManifestValidationFailure(
                "video payload must not be empty".to_string(),
            ));
        }
        if serde_json::to_vec(&self.manifest)?.len() > 1024 * 1024 {
            return Err(Error::ManifestValidationFailure(
                "manifest exceeds 1 MiB limit".to_string(),
            ));
        }
        for optional in &self.optional_chunks {
            if let OptionalChunk::Thumbnail { bytes, .. } = optional
                && bytes.len() > 32 * 1024 * 1024
            {
                return Err(Error::ManifestValidationFailure(
                    "thumbnail exceeds 32 MiB limit".to_string(),
                ));
            }
        }
        Ok(())
    }

    pub fn write_to_bytes(&self, options: WriterOptions) -> Result<Vec<u8>> {
        if options.strict_validation {
            self.validate()?;
        }
        let mut next_chunk_id = 1u64;
        let meta_chunk_id = next_chunk_id;
        next_chunk_id += 1;
        let photo_chunk_id = next_chunk_id;
        next_chunk_id += 1;
        let video_chunk_id = next_chunk_id;
        next_chunk_id += 1;

        let mut manifest = self.manifest.clone();
        manifest.photo_chunk_id = photo_chunk_id;
        manifest.video_chunk_id = video_chunk_id;

        let mut optional_metadata = OptionalMetadata::default();
        let mut optional_envelopes = Vec::new();
        for optional in &self.optional_chunks {
            let flags = optional.default_flags();
            if flags & ChunkFlags::ENCRYPTED != 0 {
                optional_metadata.encrypted_chunks_present = true;
            }
            let (kind, payload) = optional.to_payload()?;
            let envelope =
                ChunkEnvelope::new(kind, next_chunk_id, flags, payload, options.emit_crc32c);
            match kind {
                ChunkKind::Thmb => {
                    optional_metadata.thumbnail_chunk_id = Some(next_chunk_id);
                    if let OptionalChunk::Thumbnail { mime, .. } = optional {
                        manifest
                            .extensions
                            .entry("thumbnail_mime".to_string())
                            .or_insert_with(|| serde_json::Value::String(mime.clone()));
                    }
                }
                ChunkKind::Exif => {
                    optional_metadata.exif_chunk_id = Some(next_chunk_id);
                    let exif_format = match optional {
                        OptionalChunk::ExifRaw(_) => "raw",
                        OptionalChunk::ExifJson(_) => "json",
                        _ => unreachable!(),
                    };
                    manifest
                        .extensions
                        .entry("exif_format".to_string())
                        .or_insert_with(|| serde_json::Value::String(exif_format.to_string()));
                }
                ChunkKind::Xmp => optional_metadata.xmp_chunk_id = Some(next_chunk_id),
                ChunkKind::Appl => optional_metadata.apple_bridge_chunk_id = Some(next_chunk_id),
                ChunkKind::Andr => optional_metadata.android_bridge_chunk_id = Some(next_chunk_id),
                _ => {}
            }
            optional_envelopes.push(envelope);
            next_chunk_id += 1;
        }

        manifest.thumbnail_chunk_id = manifest
            .thumbnail_chunk_id
            .or(optional_metadata.thumbnail_chunk_id);
        manifest.exif_chunk_id = manifest.exif_chunk_id.or(optional_metadata.exif_chunk_id);
        manifest.xmp_chunk_id = manifest.xmp_chunk_id.or(optional_metadata.xmp_chunk_id);
        manifest.apple_bridge_chunk_id = manifest
            .apple_bridge_chunk_id
            .or(optional_metadata.apple_bridge_chunk_id);
        manifest.android_bridge_chunk_id = manifest
            .android_bridge_chunk_id
            .or(optional_metadata.android_bridge_chunk_id);
        let apple_bridge_chunk_id = manifest.apple_bridge_chunk_id;
        let android_bridge_chunk_id = manifest.android_bridge_chunk_id;
        if apple_bridge_chunk_id.is_some() {
            push_bridge_descriptor(&mut manifest, "apple-live-photo", apple_bridge_chunk_id);
        }
        if android_bridge_chunk_id.is_some() {
            push_bridge_descriptor(
                &mut manifest,
                "android-motion-photo",
                android_bridge_chunk_id,
            );
        }
        manifest.validate()?;

        let meta_payload = serde_json::to_vec_pretty(&manifest)?;
        let meta_flags = ChunkFlags::REQUIRED_FOR_PRIMARY_PLAYBACK;
        let meta = ChunkEnvelope::new(
            ChunkKind::Meta,
            meta_chunk_id,
            meta_flags,
            meta_payload,
            options.emit_crc32c,
        );
        let photo = ChunkEnvelope::new(
            ChunkKind::Phot,
            photo_chunk_id,
            ChunkFlags::REQUIRED_FOR_PRIMARY_PLAYBACK,
            self.photo.clone(),
            options.emit_crc32c,
        );
        let video = ChunkEnvelope::new(
            ChunkKind::Vide,
            video_chunk_id,
            ChunkFlags::REQUIRED_FOR_PRIMARY_PLAYBACK,
            self.video.clone(),
            options.emit_crc32c,
        );

        let mut envelopes = vec![meta, photo, video];
        envelopes.extend(optional_envelopes);

        let mut cursor = Cursor::new(Vec::new());
        let mut header = LpFileHeaderV1::new(meta_chunk_id);
        header.flags = build_file_flags(&manifest, &optional_metadata);
        header.write_to(&mut cursor)?;

        let mut toc_entries = Vec::with_capacity(envelopes.len() + 1);
        let mut seen_ids = BTreeSet::new();
        for envelope in &envelopes {
            if !seen_ids.insert(envelope.header.chunk_id) {
                return Err(Error::DuplicateChunkId(envelope.header.chunk_id));
            }
            let file_offset = cursor.position();
            envelope.write_to(&mut cursor)?;
            toc_entries.push(TocEntryV1 {
                chunk_id: envelope.header.chunk_id,
                chunk_type: envelope.header.chunk_type,
                file_offset,
                total_length: envelope.total_length(),
                stored_length: envelope.header.stored_length,
                flags: envelope.header.flags,
            });
        }

        let toc_payload = TocPayloadV1 {
            entries: toc_entries.clone(),
        };
        let toc_payload_bytes = toc_payload.to_bytes()?;
        let toc_offset = cursor.position();
        let toc_chunk = ChunkEnvelope::new(
            ChunkKind::Tocc,
            next_chunk_id,
            0,
            toc_payload_bytes,
            options.emit_crc32c,
        );
        let toc_total_length = toc_chunk.total_length();
        toc_chunk.write_to(&mut cursor)?;
        let file_size = cursor.position();

        header.toc_offset = toc_offset;
        header.toc_length = toc_total_length;
        header.file_size = file_size;
        cursor.seek(SeekFrom::Start(0))?;
        header.write_to(&mut cursor)?;
        Ok(cursor.into_inner())
    }

    pub fn write_to_path<P: AsRef<Path>>(&self, path: P, options: WriterOptions) -> Result<()> {
        let bytes = self.write_to_bytes(options)?;
        fs::write(path, bytes)?;
        Ok(())
    }
}

#[derive(Debug, Default)]
struct OptionalMetadata {
    thumbnail_chunk_id: Option<u64>,
    exif_chunk_id: Option<u64>,
    xmp_chunk_id: Option<u64>,
    apple_bridge_chunk_id: Option<u64>,
    android_bridge_chunk_id: Option<u64>,
    encrypted_chunks_present: bool,
}

impl OptionalChunk {
    fn default_flags(&self) -> u64 {
        match self {
            Self::AppleBridge(_) | Self::AndroidBridge(_) => ChunkFlags::DETACHED_BRIDGE_METADATA,
            Self::UnknownJson { flags, .. } | Self::UnknownBinary { flags, .. } => *flags,
            _ => 0,
        }
    }

    fn to_payload(&self) -> Result<(ChunkKind, Vec<u8>)> {
        match self {
            Self::Thumbnail { bytes, .. } => Ok((ChunkKind::Thmb, bytes.clone())),
            Self::ExifRaw(bytes) => Ok((ChunkKind::Exif, bytes.clone())),
            Self::ExifJson(value) => Ok((ChunkKind::Exif, serde_json::to_vec_pretty(value)?)),
            Self::Xmp(bytes) => Ok((ChunkKind::Xmp, bytes.clone())),
            Self::AppleBridge(value) => Ok((ChunkKind::Appl, serde_json::to_vec_pretty(value)?)),
            Self::AndroidBridge(value) => Ok((ChunkKind::Andr, serde_json::to_vec_pretty(value)?)),
            Self::Vendor(value) => Ok((ChunkKind::Vend, serde_json::to_vec_pretty(value)?)),
            Self::UnknownJson {
                chunk_type,
                payload,
                ..
            } => Ok((
                ChunkKind::Unknown(*chunk_type),
                serde_json::to_vec_pretty(payload)?,
            )),
            Self::UnknownBinary {
                chunk_type,
                payload,
                ..
            } => Ok((ChunkKind::Unknown(*chunk_type), payload.clone())),
        }
    }
}

pub fn inspect_file<P: AsRef<Path>>(
    path: P,
    options: ReaderOptions,
) -> Result<(LivePhotoFile, ValidationReport)> {
    let file = LivePhotoFile::open(path, options)?;
    let report = file.validate_conformance();
    Ok((file, report))
}

pub fn sniff_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"\x89PNG\r\n\x1A\n") {
        Some("image/png")
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        Some("image/webp")
    } else if bytes.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
        Some("video/webm")
    } else if bytes.get(4..8) == Some(b"ftyp") {
        match bytes.get(8..12) {
            Some(b"qt  ") => Some("video/quicktime"),
            Some(b"heic") | Some(b"heix") | Some(b"hevc") | Some(b"hevx") | Some(b"mif1")
            | Some(b"msf1") => Some("image/heic"),
            Some(b"avif") => Some("image/avif"),
            _ => Some("video/mp4"),
        }
    } else {
        None
    }
}

fn build_file_flags(manifest: &ManifestV1, optional_metadata: &OptionalMetadata) -> u64 {
    let mut flags = 0u64;
    if optional_metadata.encrypted_chunks_present {
        flags |= FileFlags::ENCRYPTED_CHUNKS_PRESENT;
    }
    if manifest.apple_bridge_chunk_id.is_some() {
        flags |= FileFlags::APPLE_BRIDGE_PRESENT;
    }
    if manifest.android_bridge_chunk_id.is_some() {
        flags |= FileFlags::ANDROID_BRIDGE_PRESENT;
    }
    flags
}

fn push_bridge_descriptor(manifest: &mut ManifestV1, target: &str, chunk_id: Option<u64>) {
    let Some(chunk_id) = chunk_id else {
        return;
    };
    if !manifest
        .bridges
        .iter()
        .any(|bridge| bridge.target == target && bridge.chunk_id == chunk_id)
    {
        manifest.bridges.push(crate::manifest::BridgeDescriptorV1 {
            target: target.to_string(),
            chunk_id,
        });
    }
}

pub fn optional_chunks_from_asset_like(
    asset: &crate::reader::LivePhotoAssetLike,
) -> Vec<OptionalChunk> {
    asset
        .optional_chunks
        .iter()
        .map(|(kind, flags, payload)| match kind {
            ChunkKind::Thmb => OptionalChunk::Thumbnail {
                mime: "application/octet-stream".to_string(),
                bytes: payload.clone(),
            },
            ChunkKind::Exif => OptionalChunk::ExifRaw(payload.clone()),
            ChunkKind::Xmp => OptionalChunk::Xmp(payload.clone()),
            ChunkKind::Appl => serde_json::from_slice(payload)
                .map(OptionalChunk::AppleBridge)
                .unwrap_or_else(|_| OptionalChunk::UnknownBinary {
                    chunk_type: kind.as_bytes(),
                    flags: *flags,
                    payload: payload.clone(),
                }),
            ChunkKind::Andr => serde_json::from_slice(payload)
                .map(OptionalChunk::AndroidBridge)
                .unwrap_or_else(|_| OptionalChunk::UnknownBinary {
                    chunk_type: kind.as_bytes(),
                    flags: *flags,
                    payload: payload.clone(),
                }),
            ChunkKind::Vend => serde_json::from_slice(payload)
                .map(OptionalChunk::Vendor)
                .unwrap_or_else(|_| OptionalChunk::UnknownBinary {
                    chunk_type: kind.as_bytes(),
                    flags: *flags,
                    payload: payload.clone(),
                }),
            ChunkKind::Unknown(chunk_type) => OptionalChunk::UnknownBinary {
                chunk_type: *chunk_type,
                flags: *flags,
                payload: payload.clone(),
            },
            ChunkKind::Meta | ChunkKind::Phot | ChunkKind::Vide | ChunkKind::Tocc => {
                OptionalChunk::UnknownBinary {
                    chunk_type: kind.as_bytes(),
                    flags: *flags,
                    payload: payload.clone(),
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::PlaybackPolicyV1;

    fn sample_manifest() -> ManifestV1 {
        ManifestV1 {
            schema: "livephoto/v1".to_string(),
            asset_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            created_at_ms: 1,
            duration_ms: 1500,
            width: 1080,
            height: 1440,
            cover_timestamp_ms: 800,
            photo_chunk_id: 0,
            video_chunk_id: 0,
            photo_mime: "image/jpeg".to_string(),
            video_mime: "video/mp4".to_string(),
            has_audio: true,
            playback: PlaybackPolicyV1::default(),
            ..ManifestV1::default()
        }
    }

    #[test]
    fn roundtrip_asset() {
        let asset = LivePhotoAsset {
            manifest: sample_manifest(),
            photo: vec![0xFF, 0xD8, 0xFF, 0xD9],
            video: b"\0\0\0\x18ftypmp42".to_vec(),
            optional_chunks: vec![],
        };
        let bytes = asset.write_to_bytes(WriterOptions::default()).unwrap();
        let parsed = LivePhotoFile::from_bytes(&bytes, ReaderOptions::default()).unwrap();
        assert_eq!(parsed.manifest.schema, "livephoto/v1");
        assert_eq!(
            parsed.get_photo().unwrap().payload,
            vec![0xFF, 0xD8, 0xFF, 0xD9]
        );
    }

    #[test]
    fn emits_crc32c_by_default() {
        let asset = LivePhotoAsset {
            manifest: sample_manifest(),
            photo: vec![0xFF, 0xD8, 0xFF, 0xD9],
            video: b"\0\0\0\x18ftypmp42".to_vec(),
            optional_chunks: vec![],
        };
        let bytes = asset.write_to_bytes(WriterOptions::default()).unwrap();
        let parsed = LivePhotoFile::from_bytes(&bytes, ReaderOptions::default()).unwrap();
        assert!(parsed.chunks.iter().all(|chunk| chunk.header.crc32c != 0));
    }

    #[test]
    fn allows_disabling_crc32c() {
        let asset = LivePhotoAsset {
            manifest: sample_manifest(),
            photo: vec![0xFF, 0xD8, 0xFF, 0xD9],
            video: b"\0\0\0\x18ftypmp42".to_vec(),
            optional_chunks: vec![],
        };
        let bytes = asset
            .write_to_bytes(WriterOptions {
                emit_crc32c: false,
                strict_validation: true,
            })
            .unwrap();
        let parsed = LivePhotoFile::from_bytes(
            &bytes,
            ReaderOptions {
                verify_checksums: false,
                ..ReaderOptions::default()
            },
        )
        .unwrap();
        assert!(parsed.chunks.iter().all(|chunk| chunk.header.crc32c == 0));
    }

    #[test]
    fn preserves_unknown_optional_chunks_for_rewrite() {
        let asset = LivePhotoAsset {
            manifest: sample_manifest(),
            photo: vec![0xFF, 0xD8, 0xFF, 0xD9],
            video: b"\0\0\0\x18ftypmp42".to_vec(),
            optional_chunks: vec![OptionalChunk::UnknownBinary {
                chunk_type: *b"ZZZZ",
                flags: 0xAB,
                payload: b"opaque".to_vec(),
            }],
        };
        let bytes = asset.write_to_bytes(WriterOptions::default()).unwrap();
        let parsed = LivePhotoFile::from_bytes(&bytes, ReaderOptions::default()).unwrap();
        let rebuilt = LivePhotoAsset {
            manifest: parsed.manifest.clone(),
            photo: parsed.get_photo().unwrap().payload.clone(),
            video: parsed.get_video().unwrap().payload.clone(),
            optional_chunks: optional_chunks_from_asset_like(&parsed.to_asset()),
        };
        let rebuilt_bytes = rebuilt.write_to_bytes(WriterOptions::default()).unwrap();
        let rebuilt_file =
            LivePhotoFile::from_bytes(&rebuilt_bytes, ReaderOptions::default()).unwrap();
        assert!(
            rebuilt_file
                .to_asset()
                .optional_chunks
                .iter()
                .any(|(kind, _, payload)| *kind == ChunkKind::Unknown(*b"ZZZZ")
                    && payload == b"opaque")
        );
    }

    #[test]
    fn writes_bridge_flags_and_manifest_extensions() {
        let asset = LivePhotoAsset {
            manifest: sample_manifest(),
            photo: vec![0xFF, 0xD8, 0xFF, 0xD9],
            video: b"\0\0\0\x18ftypmp42".to_vec(),
            optional_chunks: vec![
                OptionalChunk::Thumbnail {
                    mime: "image/jpeg".to_string(),
                    bytes: vec![0xFF, 0xD8, 0xFF, 0xD9],
                },
                OptionalChunk::ExifJson(serde_json::json!({"iso": 100})),
                OptionalChunk::AppleBridge(crate::manifest::AppleBridgeV1 {
                    asset_identifier: "550e8400-e29b-41d4-a716-446655440000".to_string(),
                    still_image_time_ms: 800,
                    photo_codec_hint: Some("image/heic".to_string()),
                    video_codec_hint: Some("video/quicktime".to_string()),
                    maker_apple_key_17: None,
                    quicktime_content_identifier: None,
                }),
                OptionalChunk::AndroidBridge(crate::manifest::AndroidBridgeV1 {
                    presentation_timestamp_us: 800000,
                    xmp_format: "container".to_string(),
                    primary_image_role: "display".to_string(),
                    embedded_video_role: "motion".to_string(),
                }),
            ],
        };
        let bytes = asset.write_to_bytes(WriterOptions::default()).unwrap();
        let parsed = LivePhotoFile::from_bytes(&bytes, ReaderOptions::default()).unwrap();
        assert_ne!(
            parsed.header.flags & crate::types::FileFlags::APPLE_BRIDGE_PRESENT,
            0
        );
        assert_ne!(
            parsed.header.flags & crate::types::FileFlags::ANDROID_BRIDGE_PRESENT,
            0
        );
        assert_eq!(
            parsed.manifest.extensions.get("thumbnail_mime"),
            Some(&serde_json::Value::String("image/jpeg".to_string()))
        );
        assert_eq!(
            parsed.manifest.extensions.get("exif_format"),
            Some(&serde_json::Value::String("json".to_string()))
        );
        assert!(
            parsed
                .manifest
                .bridges
                .iter()
                .any(|bridge| bridge.target == "apple-live-photo")
        );
        assert!(
            parsed
                .manifest
                .bridges
                .iter()
                .any(|bridge| bridge.target == "android-motion-photo")
        );
    }

    #[test]
    fn writes_compact_file_flags_for_encryption_and_bridges() {
        let asset = LivePhotoAsset {
            manifest: sample_manifest(),
            photo: vec![0xFF, 0xD8, 0xFF, 0xD9],
            video: b"\0\0\0\x18ftypmp42".to_vec(),
            optional_chunks: vec![
                OptionalChunk::UnknownBinary {
                    chunk_type: *b"SECR",
                    flags: ChunkFlags::ENCRYPTED,
                    payload: b"ciphertext".to_vec(),
                },
                OptionalChunk::AppleBridge(crate::manifest::AppleBridgeV1 {
                    asset_identifier: "550e8400-e29b-41d4-a716-446655440000".to_string(),
                    still_image_time_ms: 800,
                    photo_codec_hint: None,
                    video_codec_hint: None,
                    maker_apple_key_17: None,
                    quicktime_content_identifier: None,
                }),
                OptionalChunk::AndroidBridge(crate::manifest::AndroidBridgeV1 {
                    presentation_timestamp_us: 800000,
                    xmp_format: "container".to_string(),
                    primary_image_role: "display".to_string(),
                    embedded_video_role: "motion".to_string(),
                }),
            ],
        };
        let bytes = asset.write_to_bytes(WriterOptions::default()).unwrap();
        let parsed = LivePhotoFile::from_bytes(&bytes, ReaderOptions::default()).unwrap();
        assert_eq!(
            parsed.header.flags,
            crate::types::FileFlags::ENCRYPTED_CHUNKS_PRESENT
                | crate::types::FileFlags::APPLE_BRIDGE_PRESENT
                | crate::types::FileFlags::ANDROID_BRIDGE_PRESENT
        );
    }

    #[test]
    fn preserves_hash_and_sign_chunks_as_unknown_for_rewrite() {
        let asset = LivePhotoAsset {
            manifest: sample_manifest(),
            photo: vec![0xFF, 0xD8, 0xFF, 0xD9],
            video: b"\0\0\0\x18ftypmp42".to_vec(),
            optional_chunks: vec![
                OptionalChunk::UnknownBinary {
                    chunk_type: *b"HASH",
                    flags: 0,
                    payload: b"{\"alg\":\"sha256\"}".to_vec(),
                },
                OptionalChunk::UnknownBinary {
                    chunk_type: *b"SIGN",
                    flags: 0,
                    payload: b"{\"algorithm\":\"ed25519\"}".to_vec(),
                },
            ],
        };
        let bytes = asset.write_to_bytes(WriterOptions::default()).unwrap();
        let parsed = LivePhotoFile::from_bytes(&bytes, ReaderOptions::default()).unwrap();
        let rebuilt = LivePhotoAsset {
            manifest: parsed.manifest.clone(),
            photo: parsed.get_photo().unwrap().payload.clone(),
            video: parsed.get_video().unwrap().payload.clone(),
            optional_chunks: optional_chunks_from_asset_like(&parsed.to_asset()),
        };
        assert!(rebuilt.optional_chunks.iter().any(|optional| matches!(
            optional,
            OptionalChunk::UnknownBinary {
                chunk_type,
                payload,
                ..
            } if *chunk_type == *b"HASH" && payload == b"{\"alg\":\"sha256\"}"
        )));
        assert!(rebuilt.optional_chunks.iter().any(|optional| matches!(
            optional,
            OptionalChunk::UnknownBinary {
                chunk_type,
                payload,
                ..
            } if *chunk_type == *b"SIGN" && payload == b"{\"algorithm\":\"ed25519\"}"
        )));
    }
}
