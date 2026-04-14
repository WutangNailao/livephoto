use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use livephoto_format::{
    inspect_file, AndroidBridgeV1, AppleBridgeV1, HashPayloadV1, LivePhotoAsset, ManifestV1,
    OptionalChunk, ReaderOptions, Strictness, WriterOptions,
};

#[derive(Debug, Parser)]
#[command(name = "livephoto")]
#[command(about = "Pack and inspect .livephoto files")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Pack(Box<PackArgs>),
    Inspect(Box<InspectArgs>),
    Unpack(Box<UnpackArgs>),
}

#[derive(Debug, clap::Args)]
struct PackArgs {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        photo: PathBuf,
        #[arg(long)]
        video: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        thumbnail: Option<PathBuf>,
        #[arg(long)]
        exif_raw: Option<PathBuf>,
        #[arg(long)]
        xmp: Option<PathBuf>,
        #[arg(long)]
        hash_json: Option<PathBuf>,
        #[arg(long)]
        apple_bridge_json: Option<PathBuf>,
        #[arg(long)]
        android_bridge_json: Option<PathBuf>,
        #[arg(long)]
        no_crc32c: bool,
}

#[derive(Debug, clap::Args)]
struct InspectArgs {
        file: PathBuf,
        #[arg(long)]
        recovery: bool,
        #[arg(long)]
        no_verify_checksums: bool,
}

#[derive(Debug, clap::Args)]
struct UnpackArgs {
    file: PathBuf,
    #[arg(long)]
    out_dir: PathBuf,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Pack(args) => {
            let PackArgs {
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
            no_crc32c,
            } = *args;
            let manifest: ManifestV1 =
                serde_json::from_slice(&fs::read(&manifest).with_context(|| format!("read {}", manifest.display()))?)
                    .with_context(|| format!("parse {}", manifest.display()))?;
            let photo_bytes =
                fs::read(&photo).with_context(|| format!("read {}", photo.display()))?;
            let video_bytes =
                fs::read(&video).with_context(|| format!("read {}", video.display()))?;
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
                let value: HashPayloadV1 =
                    serde_json::from_slice(&fs::read(&path).with_context(|| format!("read {}", path.display()))?)?;
                optional_chunks.push(OptionalChunk::Hash(value));
            }
            if let Some(path) = apple_bridge_json {
                let value: AppleBridgeV1 =
                    serde_json::from_slice(&fs::read(&path).with_context(|| format!("read {}", path.display()))?)?;
                optional_chunks.push(OptionalChunk::AppleBridge(value));
            }
            if let Some(path) = android_bridge_json {
                let value: AndroidBridgeV1 =
                    serde_json::from_slice(&fs::read(&path).with_context(|| format!("read {}", path.display()))?)?;
                optional_chunks.push(OptionalChunk::AndroidBridge(value));
            }
            let asset = LivePhotoAsset {
                manifest,
                photo: photo_bytes,
                video: video_bytes,
                optional_chunks,
            };
            asset.write_to_path(
                &out,
                WriterOptions {
                    emit_crc32c: !no_crc32c,
                    strict_validation: true,
                },
            )?;
            println!("wrote {}", out.display());
        }
        Command::Inspect(args) => {
            let InspectArgs {
                file,
                recovery,
                no_verify_checksums,
            } = *args;
            let options = ReaderOptions {
                strictness: if recovery {
                    Strictness::Recovery
                } else {
                    Strictness::Strict
                },
                verify_checksums: !no_verify_checksums,
                ..ReaderOptions::default()
            };
            let (parsed, report) = inspect_file(&file, options)?;
            println!("file: {}", file.display());
            println!("header: {:?}", parsed.header);
            println!("manifest:");
            println!("{}", serde_json::to_string_pretty(&parsed.manifest)?);
            println!("chunks:");
            for chunk in &parsed.chunks {
                println!(
                    "  id={} type={} offset={} stored_length={} flags={:#x}",
                    chunk.header.chunk_id,
                    String::from_utf8_lossy(&chunk.header.chunk_type),
                    chunk.file_offset,
                    chunk.header.stored_length,
                    chunk.header.flags
                );
            }
            if report.is_clean() {
                println!("conformance: clean");
            } else {
                println!("conformance issues:");
                for issue in report.issues {
                    println!("  {}: {}", issue.code, issue.message);
                }
            }
        }
        Command::Unpack(args) => {
            let UnpackArgs { file, out_dir } = *args;
            let (parsed, report) = inspect_file(&file, ReaderOptions::default())?;
            fs::create_dir_all(&out_dir)?;
            fs::write(out_dir.join("manifest.json"), serde_json::to_vec_pretty(&parsed.manifest)?)?;
            if let Some(photo) = parsed.get_photo() {
                fs::write(out_dir.join("photo.bin"), &photo.payload)?;
            }
            if let Some(video) = parsed.get_video() {
                fs::write(out_dir.join("video.bin"), &video.payload)?;
            }
            let asset_like = parsed.to_asset();
            for (index, (kind, flags, payload)) in asset_like.optional_chunks.iter().enumerate() {
                let name = format!(
                    "{index:02}_{}_flags_{flags:x}.bin",
                    String::from_utf8_lossy(&kind.as_bytes())
                );
                fs::write(out_dir.join(name), payload)?;
            }
            if report.is_clean() {
                println!("unpacked {} to {}", file.display(), out_dir.display());
            } else {
                println!(
                    "unpacked {} to {} with {} conformance issue(s)",
                    file.display(),
                    out_dir.display(),
                    report.issues.len()
                );
            }
        }
    }
    Ok(())
}
