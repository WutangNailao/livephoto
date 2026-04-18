use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use livephoto_format::{
    AndroidBridgeV1, AppleBridgeV1, HashPayloadV1, LivePhotoAsset, LivePhotoFile, ManifestV1,
    OptionalChunk, ReaderOptions, Strictness, ValidationReport, WriterOptions, inspect_file,
};

#[derive(Debug, Clone)]
pub struct PackRequest {
    pub manifest: PathBuf,
    pub photo: PathBuf,
    pub video: PathBuf,
    pub out: PathBuf,
    pub thumbnail: Option<PathBuf>,
    pub exif_raw: Option<PathBuf>,
    pub xmp: Option<PathBuf>,
    pub hash_json: Option<PathBuf>,
    pub apple_bridge_json: Option<PathBuf>,
    pub android_bridge_json: Option<PathBuf>,
    pub emit_crc32c: bool,
}

#[derive(Debug, Clone)]
pub struct PackResult {
    pub output_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct InspectRequest {
    pub file: PathBuf,
    pub recovery: bool,
    pub verify_checksums: bool,
}

#[derive(Debug, Clone)]
pub struct InspectResult {
    pub parsed: LivePhotoFile,
    pub report: ValidationReport,
}

#[derive(Debug, Clone)]
pub struct UnpackRequest {
    pub file: PathBuf,
    pub out_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct UnpackResult {
    pub output_dir: PathBuf,
    pub report: ValidationReport,
}

pub fn pack_livephoto(request: PackRequest) -> Result<PackResult> {
    let PackRequest {
        manifest,
        photo,
        video,
        out,
        thumbnail,
        exif_raw,
        xmp,
        hash_json,
        apple_bridge_json,
        android_bridge_json,
        emit_crc32c,
    } = request;

    let manifest: ManifestV1 = serde_json::from_slice(
        &fs::read(&manifest).with_context(|| format!("read {}", manifest.display()))?,
    )
    .with_context(|| format!("parse {}", manifest.display()))?;
    let photo_bytes = fs::read(&photo).with_context(|| format!("read {}", photo.display()))?;
    let video_bytes = fs::read(&video).with_context(|| format!("read {}", video.display()))?;
    let optional_chunks = load_optional_chunks(
        thumbnail,
        exif_raw,
        xmp,
        hash_json,
        apple_bridge_json,
        android_bridge_json,
    )?;

    let asset = LivePhotoAsset {
        manifest,
        photo: photo_bytes,
        video: video_bytes,
        optional_chunks,
    };
    asset.write_to_path(
        &out,
        WriterOptions {
            emit_crc32c,
            strict_validation: true,
        },
    )?;

    Ok(PackResult { output_path: out })
}

pub fn inspect_livephoto(request: InspectRequest) -> Result<InspectResult> {
    let options = ReaderOptions {
        strictness: if request.recovery {
            Strictness::Recovery
        } else {
            Strictness::Strict
        },
        verify_checksums: request.verify_checksums,
        ..ReaderOptions::default()
    };
    let (parsed, report) = inspect_file(&request.file, options)?;
    Ok(InspectResult { parsed, report })
}

pub fn unpack_livephoto(request: UnpackRequest) -> Result<UnpackResult> {
    let (parsed, report) = inspect_file(&request.file, ReaderOptions::default())?;
    fs::create_dir_all(&request.out_dir)?;
    fs::write(
        request.out_dir.join("manifest.json"),
        serde_json::to_vec_pretty(&parsed.manifest)?,
    )?;
    if let Some(photo) = parsed.get_photo() {
        fs::write(request.out_dir.join("photo.bin"), &photo.payload)?;
    }
    if let Some(video) = parsed.get_video() {
        fs::write(request.out_dir.join("video.bin"), &video.payload)?;
    }
    let asset_like = parsed.to_asset();
    for (index, (kind, flags, payload)) in asset_like.optional_chunks.iter().enumerate() {
        let name = format!(
            "{index:02}_{}_flags_{flags:x}.bin",
            String::from_utf8_lossy(&kind.as_bytes())
        );
        fs::write(request.out_dir.join(name), payload)?;
    }

    Ok(UnpackResult {
        output_dir: request.out_dir,
        report,
    })
}

fn load_optional_chunks(
    thumbnail: Option<PathBuf>,
    exif_raw: Option<PathBuf>,
    xmp: Option<PathBuf>,
    hash_json: Option<PathBuf>,
    apple_bridge_json: Option<PathBuf>,
    android_bridge_json: Option<PathBuf>,
) -> Result<Vec<OptionalChunk>> {
    let mut optional_chunks = Vec::new();

    if let Some(path) = thumbnail {
        optional_chunks.push(OptionalChunk::Thumbnail {
            mime: "application/octet-stream".to_string(),
            bytes: fs::read(&path).with_context(|| format!("read {}", path.display()))?,
        });
    }
    if let Some(path) = exif_raw {
        optional_chunks.push(OptionalChunk::ExifRaw(
            fs::read(&path).with_context(|| format!("read {}", path.display()))?,
        ));
    }
    if let Some(path) = xmp {
        optional_chunks.push(OptionalChunk::Xmp(
            fs::read(&path).with_context(|| format!("read {}", path.display()))?,
        ));
    }
    if let Some(path) = hash_json {
        let value: HashPayloadV1 = serde_json::from_slice(
            &fs::read(&path).with_context(|| format!("read {}", path.display()))?,
        )?;
        optional_chunks.push(OptionalChunk::Hash(value));
    }
    if let Some(path) = apple_bridge_json {
        let value: AppleBridgeV1 = serde_json::from_slice(
            &fs::read(&path).with_context(|| format!("read {}", path.display()))?,
        )?;
        optional_chunks.push(OptionalChunk::AppleBridge(value));
    }
    if let Some(path) = android_bridge_json {
        let value: AndroidBridgeV1 = serde_json::from_slice(
            &fs::read(&path).with_context(|| format!("read {}", path.display()))?,
        )?;
        optional_chunks.push(OptionalChunk::AndroidBridge(value));
    }

    Ok(optional_chunks)
}
