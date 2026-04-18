use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;

use crate::chunk::{
    CHUNK_HEADER_SIZE_V1, ChunkKind, ChunkRecord, LpChunkHeaderV1, TocEntryV1, TocPayloadV1,
};
use crate::error::{ConformanceIssue, Error, Result};
use crate::manifest::ManifestV1;
use crate::types::LpFileHeaderV1;
use crate::writer::{ReaderOptions, Strictness, ValidationReport};

#[derive(Debug, Clone)]
pub struct ChunkPayloadView {
    pub header: LpChunkHeaderV1,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct LivePhotoFile {
    pub header: LpFileHeaderV1,
    pub chunks: Vec<ChunkRecord>,
    pub manifest: ManifestV1,
    payloads: BTreeMap<u64, ChunkPayloadView>,
    pub toc: TocPayloadV1,
}

impl LivePhotoFile {
    pub fn open<P: AsRef<Path>>(path: P, options: ReaderOptions) -> Result<Self> {
        let bytes = fs::read(path)?;
        Self::from_bytes(&bytes, options)
    }

    pub fn from_bytes(bytes: &[u8], options: ReaderOptions) -> Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let header = LpFileHeaderV1::read_from(&mut cursor)?;
        header.validate(options.strictness == Strictness::Strict)?;
        if header.file_size != bytes.len() as u64 {
            return Err(Error::MalformedHeader(format!(
                "header file_size {} does not match actual {}",
                header.file_size,
                bytes.len()
            )));
        }

        let toc = match read_toc(bytes, &header) {
            Ok(toc) => toc,
            Err(err) if options.strictness == Strictness::Recovery => scan_chunks_as_toc(bytes)?,
            Err(err) => return Err(err),
        };
        let strict = options.strictness == Strictness::Strict;
        let mut seen_ids = BTreeSet::new();
        let mut records = Vec::with_capacity(toc.entries.len());
        let mut payloads = BTreeMap::new();
        let mut occupied_ranges = Vec::with_capacity(toc.entries.len());

        for entry in &toc.entries {
            if !seen_ids.insert(entry.chunk_id) {
                return Err(Error::DuplicateChunkId(entry.chunk_id));
            }
            let record = read_chunk_record(bytes, entry, strict, options.verify_checksums)?;
            occupied_ranges.push((record.file_offset, record.file_offset + record.total_length));
            payloads.insert(
                record.header.chunk_id,
                ChunkPayloadView {
                    header: record.header,
                    payload: bytes[chunk_payload_range(&record)?].to_vec(),
                },
            );
            records.push(record);
        }
        validate_no_overlap(&occupied_ranges)?;

        ensure_single_tocc(&records)?;
        let manifest_payload = payloads
            .get(&header.primary_manifest_id)
            .ok_or(Error::RequiredChunkMissing("META"))?;
        if manifest_payload.header.kind() != ChunkKind::Meta {
            return Err(Error::RequiredChunkMissing("META"));
        }
        if manifest_payload.payload.len() > options.max_manifest_size {
            return Err(Error::ManifestValidationFailure(format!(
                "manifest exceeds configured limit of {} bytes",
                options.max_manifest_size
            )));
        }
        let manifest: ManifestV1 = serde_json::from_slice(&manifest_payload.payload)
            .map_err(|err| Error::ManifestParseFailure(err.to_string()))?;
        manifest.validate()?;
        ensure_required_kind(&records, ChunkKind::Phot)?;
        ensure_required_kind(&records, ChunkKind::Vide)?;
        ensure_chunk_id_exists(&payloads, manifest.photo_chunk_id, "photo_chunk_id")?;
        ensure_chunk_id_exists(&payloads, manifest.video_chunk_id, "video_chunk_id")?;
        validate_manifest_payload_mime(&manifest, &payloads)?;
        if let Some(thumbnail_chunk_id) = manifest.thumbnail_chunk_id
            && let Some(thumbnail) = payloads.get(&thumbnail_chunk_id)
            && thumbnail.payload.len() > options.max_thumbnail_size
        {
            return Err(Error::ManifestValidationFailure(format!(
                "thumbnail exceeds configured limit of {} bytes",
                options.max_thumbnail_size
            )));
        }

        Ok(Self {
            header,
            chunks: records,
            manifest,
            payloads,
            toc,
        })
    }

    pub fn get_chunk(&self, chunk_id: u64) -> Option<&ChunkPayloadView> {
        self.payloads.get(&chunk_id)
    }

    pub fn get_photo(&self) -> Option<&ChunkPayloadView> {
        self.get_chunk(self.manifest.photo_chunk_id)
    }

    pub fn get_video(&self) -> Option<&ChunkPayloadView> {
        self.get_chunk(self.manifest.video_chunk_id)
    }

    pub fn validate_conformance(&self) -> ValidationReport {
        let mut report = ValidationReport::default();
        for required in [ChunkKind::Meta, ChunkKind::Phot, ChunkKind::Vide] {
            if !self
                .chunks
                .iter()
                .any(|record| record.header.kind() == required)
            {
                report.issues.push(ConformanceIssue {
                    code: "missing_required_chunk",
                    message: format!("missing required chunk {}", required.display_name()),
                });
            }
        }
        if self.toc.entries.is_empty() {
            report.issues.push(ConformanceIssue {
                code: "missing_toc_entries",
                message: "TOC is empty".to_string(),
            });
        }
        if self.manifest.validate().is_err() {
            report.issues.push(ConformanceIssue {
                code: "invalid_manifest",
                message: "manifest failed validation".to_string(),
            });
        }
        if !self.payloads.contains_key(&self.manifest.photo_chunk_id) {
            report.issues.push(ConformanceIssue {
                code: "missing_photo_chunk_id",
                message: "manifest photo_chunk_id does not resolve".to_string(),
            });
        }
        if !self.payloads.contains_key(&self.manifest.video_chunk_id) {
            report.issues.push(ConformanceIssue {
                code: "missing_video_chunk_id",
                message: "manifest video_chunk_id does not resolve".to_string(),
            });
        }
        if let Some(photo) = self.payloads.get(&self.manifest.photo_chunk_id)
            && let Some(sniffed) = crate::writer::sniff_mime(&photo.payload)
            && sniffed != self.manifest.photo_mime
        {
            report.issues.push(ConformanceIssue {
                code: "photo_mime_mismatch",
                message: format!(
                    "manifest photo_mime={} but sniffed {}",
                    self.manifest.photo_mime, sniffed
                ),
            });
        }
        if let Some(video) = self.payloads.get(&self.manifest.video_chunk_id)
            && let Some(sniffed) = crate::writer::sniff_mime(&video.payload)
            && sniffed != self.manifest.video_mime
        {
            report.issues.push(ConformanceIssue {
                code: "video_mime_mismatch",
                message: format!(
                    "manifest video_mime={} but sniffed {}",
                    self.manifest.video_mime, sniffed
                ),
            });
        }
        report
    }

    pub fn to_asset(&self) -> LivePhotoAssetLike {
        let mut optional_chunks = Vec::new();
        for (chunk_id, view) in &self.payloads {
            if *chunk_id == self.manifest.photo_chunk_id
                || *chunk_id == self.manifest.video_chunk_id
                || *chunk_id == self.header.primary_manifest_id
            {
                continue;
            }
            optional_chunks.push((view.header.kind(), view.header.flags, view.payload.clone()));
        }
        LivePhotoAssetLike {
            manifest: self.manifest.clone(),
            photo: self
                .get_photo()
                .map(|v| v.payload.clone())
                .unwrap_or_default(),
            video: self
                .get_video()
                .map(|v| v.payload.clone())
                .unwrap_or_default(),
            optional_chunks,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LivePhotoAssetLike {
    pub manifest: ManifestV1,
    pub photo: Vec<u8>,
    pub video: Vec<u8>,
    pub optional_chunks: Vec<(ChunkKind, u64, Vec<u8>)>,
}

fn read_toc(bytes: &[u8], header: &LpFileHeaderV1) -> Result<TocPayloadV1> {
    let toc_offset = header.toc_offset as usize;
    let toc_length = header.toc_length as usize;
    if toc_offset + toc_length > bytes.len() {
        return Err(Error::MalformedToc(
            "TOC region is out of bounds".to_string(),
        ));
    }
    let mut cursor = Cursor::new(&bytes[toc_offset..toc_offset + toc_length]);
    let toc_header = LpChunkHeaderV1::read_from(&mut cursor)?;
    toc_header.validate(true)?;
    if toc_header.kind() != ChunkKind::Tocc {
        return Err(Error::MalformedToc(
            "chunk at toc_offset is not TOCC".to_string(),
        ));
    }
    let mut payload = vec![0u8; toc_header.stored_length as usize];
    cursor.read_exact(&mut payload)?;
    TocPayloadV1::from_bytes(&payload)
}

fn scan_chunks_as_toc(bytes: &[u8]) -> Result<TocPayloadV1> {
    let mut entries = Vec::new();
    let mut offset = u64::from(crate::types::FILE_HEADER_SIZE_V1);
    while (offset as usize) < bytes.len() {
        if offset as usize + CHUNK_HEADER_SIZE_V1 as usize > bytes.len() {
            break;
        }
        let mut cursor = Cursor::new(&bytes[offset as usize..]);
        let header = LpChunkHeaderV1::read_from(&mut cursor)?;
        if header.header_size != CHUNK_HEADER_SIZE_V1 {
            break;
        }
        let total_length = header.total_length();
        if total_length == 0 || offset + total_length > bytes.len() as u64 {
            break;
        }
        if header.kind() != ChunkKind::Tocc {
            entries.push(TocEntryV1 {
                chunk_id: header.chunk_id,
                chunk_type: header.chunk_type,
                file_offset: offset,
                total_length,
                stored_length: header.stored_length,
                flags: header.flags,
            });
        }
        offset += total_length;
    }
    if entries.is_empty() {
        return Err(Error::MalformedToc(
            "recovery scan could not recover any chunks".to_string(),
        ));
    }
    Ok(TocPayloadV1 { entries })
}

fn read_chunk_record(
    bytes: &[u8],
    entry: &TocEntryV1,
    strict: bool,
    verify_checksums: bool,
) -> Result<ChunkRecord> {
    let offset = entry.file_offset as usize;
    let total_length = entry.total_length as usize;
    if offset + total_length > bytes.len() {
        return Err(Error::InvalidOffsetOrLength(format!(
            "chunk {} exceeds file bounds",
            entry.chunk_id
        )));
    }
    let mut cursor = Cursor::new(&bytes[offset..offset + total_length]);
    let header = LpChunkHeaderV1::read_from(&mut cursor)?;
    header.validate(strict)?;
    if header.chunk_id != entry.chunk_id {
        return Err(Error::MalformedToc(format!(
            "TOC entry chunk id {} does not match actual {}",
            entry.chunk_id, header.chunk_id
        )));
    }
    if header.stored_length != entry.stored_length {
        return Err(Error::MalformedToc(format!(
            "TOC entry stored_length {} does not match actual {}",
            entry.stored_length, header.stored_length
        )));
    }
    let payload_range = (offset + CHUNK_HEADER_SIZE_V1 as usize)
        ..(offset + CHUNK_HEADER_SIZE_V1 as usize + header.stored_length as usize);
    if payload_range.end > bytes.len() {
        return Err(Error::InvalidOffsetOrLength(
            "payload range exceeds file bounds".to_string(),
        ));
    }
    if verify_checksums && header.crc32c != 0 {
        let expected = header.crc32c as u32;
        let actual = crc32c::crc32c(&bytes[payload_range.clone()]);
        if actual != expected {
            return Err(Error::ChecksumMismatch {
                chunk_id: header.chunk_id,
            });
        }
    }
    Ok(ChunkRecord {
        header,
        file_offset: entry.file_offset,
        total_length: entry.total_length,
    })
}

fn chunk_payload_range(record: &ChunkRecord) -> Result<std::ops::Range<usize>> {
    let start = record.file_offset as usize + CHUNK_HEADER_SIZE_V1 as usize;
    let end = start + record.header.stored_length as usize;
    Ok(start..end)
}

fn ensure_required_kind(records: &[ChunkRecord], kind: ChunkKind) -> Result<()> {
    if records.iter().any(|record| record.header.kind() == kind) {
        Ok(())
    } else {
        Err(Error::RequiredChunkMissing(kind.display_name()))
    }
}

fn ensure_chunk_id_exists(
    payloads: &BTreeMap<u64, ChunkPayloadView>,
    chunk_id: u64,
    field: &'static str,
) -> Result<()> {
    if payloads.contains_key(&chunk_id) {
        Ok(())
    } else {
        Err(Error::ManifestValidationFailure(format!(
            "{field} references missing chunk {chunk_id}"
        )))
    }
}

fn ensure_single_tocc(records: &[ChunkRecord]) -> Result<()> {
    let count = records
        .iter()
        .filter(|record| record.header.kind() == ChunkKind::Tocc)
        .count();
    if count > 1 {
        Err(Error::MalformedToc(
            "multiple TOCC chunks found".to_string(),
        ))
    } else {
        Ok(())
    }
}

fn validate_no_overlap(ranges: &[(u64, u64)]) -> Result<()> {
    let mut sorted = ranges.to_vec();
    sorted.sort_unstable_by_key(|range| range.0);
    for pair in sorted.windows(2) {
        if pair[0].1 > pair[1].0 {
            return Err(Error::InvalidOffsetOrLength(
                "chunk regions overlap".to_string(),
            ));
        }
    }
    Ok(())
}

fn validate_manifest_payload_mime(
    manifest: &ManifestV1,
    payloads: &BTreeMap<u64, ChunkPayloadView>,
) -> Result<()> {
    if let Some(photo) = payloads.get(&manifest.photo_chunk_id)
        && let Some(sniffed) = crate::writer::sniff_mime(&photo.payload)
        && sniffed != manifest.photo_mime
    {
        return Err(Error::ManifestValidationFailure(format!(
            "photo_mime {} does not match sniffed {}",
            manifest.photo_mime, sniffed
        )));
    }
    if let Some(video) = payloads.get(&manifest.video_chunk_id)
        && let Some(sniffed) = crate::writer::sniff_mime(&video.payload)
        && sniffed != manifest.video_mime
    {
        return Err(Error::ManifestValidationFailure(format!(
            "video_mime {} does not match sniffed {}",
            manifest.video_mime, sniffed
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Error;
    use crate::manifest::{ManifestV1, PlaybackPolicyV1};
    use crate::writer::{LivePhotoAsset, WriterOptions};

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

    fn sample_asset_bytes() -> Vec<u8> {
        let asset = LivePhotoAsset {
            manifest: sample_manifest(),
            photo: vec![0xFF, 0xD8, 0xFF, 0xD9],
            video: b"\0\0\0\x18ftypmp42".to_vec(),
            optional_chunks: vec![],
        };
        asset.write_to_bytes(WriterOptions::default()).unwrap()
    }

    fn write_u32_le(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn write_u16_le(bytes: &mut [u8], offset: usize, value: u16) {
        bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }

    #[test]
    fn recovery_mode_scans_when_toc_header_is_corrupted() {
        let mut bytes = sample_asset_bytes();
        let toc_offset =
            u64::from_le_bytes(bytes[20..28].try_into().expect("slice length should match"));
        bytes[toc_offset as usize] = b'B';
        let strict = LivePhotoFile::from_bytes(&bytes, ReaderOptions::default());
        assert!(strict.is_err());

        let recovered = LivePhotoFile::from_bytes(
            &bytes,
            ReaderOptions {
                strictness: Strictness::Recovery,
                ..ReaderOptions::default()
            },
        )
        .unwrap();
        assert_eq!(recovered.manifest.schema, "livephoto/v1");
    }

    #[test]
    fn rejects_file_header_sizes_other_than_fixed_v1_size() {
        let mut larger = sample_asset_bytes();
        write_u32_le(&mut larger, 8, crate::types::FILE_HEADER_SIZE_V1 + 4);

        let err = LivePhotoFile::from_bytes(&larger, ReaderOptions::default()).unwrap_err();
        assert!(matches!(
            err,
            Error::MalformedHeader(message) if message.contains("must equal")
        ));

        let recovery_err = LivePhotoFile::from_bytes(
            &larger,
            ReaderOptions {
                strictness: Strictness::Recovery,
                ..ReaderOptions::default()
            },
        )
        .unwrap_err();
        assert!(matches!(
            recovery_err,
            Error::MalformedHeader(message) if message.contains("must equal")
        ));

        let mut smaller = sample_asset_bytes();
        write_u32_le(&mut smaller, 8, crate::types::FILE_HEADER_SIZE_V1 - 1);

        let err = LivePhotoFile::from_bytes(&smaller, ReaderOptions::default()).unwrap_err();
        assert!(matches!(
            err,
            Error::MalformedHeader(message) if message.contains("must equal")
        ));
    }

    #[test]
    fn rejects_chunk_header_sizes_other_than_fixed_v1_size() {
        let mut larger = sample_asset_bytes();
        write_u16_le(
            &mut larger,
            crate::types::FILE_HEADER_SIZE_V1 as usize + 6,
            CHUNK_HEADER_SIZE_V1 + 16,
        );

        let err = LivePhotoFile::from_bytes(&larger, ReaderOptions::default()).unwrap_err();
        assert!(matches!(
            err,
            Error::InvalidOffsetOrLength(message) if message.contains("must equal")
        ));

        let mut smaller = sample_asset_bytes();
        write_u16_le(
            &mut smaller,
            crate::types::FILE_HEADER_SIZE_V1 as usize + 6,
            CHUNK_HEADER_SIZE_V1 - 1,
        );

        let err = LivePhotoFile::from_bytes(&smaller, ReaderOptions::default()).unwrap_err();
        assert!(matches!(
            err,
            Error::InvalidOffsetOrLength(message) if message.contains("must equal")
        ));
    }

    #[test]
    fn recovery_mode_does_not_treat_larger_chunk_headers_as_extensible() {
        let mut bytes = sample_asset_bytes();
        let toc_offset =
            u64::from_le_bytes(bytes[20..28].try_into().expect("slice length should match"));
        bytes[toc_offset as usize] = b'B';
        write_u16_le(
            &mut bytes,
            crate::types::FILE_HEADER_SIZE_V1 as usize + 6,
            CHUNK_HEADER_SIZE_V1 + 16,
        );

        let err = LivePhotoFile::from_bytes(
            &bytes,
            ReaderOptions {
                strictness: Strictness::Recovery,
                ..ReaderOptions::default()
            },
        )
        .unwrap_err();
        assert!(matches!(err, Error::MalformedToc(message) if message.contains("recovery scan")));
    }
}
