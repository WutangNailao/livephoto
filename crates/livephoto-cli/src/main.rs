use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use livephoto_toolkit::{
    InspectRequest, PackRequest, UnpackRequest, inspect_livephoto, pack_livephoto, unpack_livephoto,
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
                apple_bridge_json,
                android_bridge_json,
                no_crc32c,
            } = *args;
            let result = pack_livephoto(PackRequest {
                manifest,
                photo,
                video,
                out,
                thumbnail,
                exif_raw,
                xmp,
                apple_bridge_json,
                android_bridge_json,
                emit_crc32c: !no_crc32c,
            })?;
            println!("wrote {}", result.output_path.display());
        }
        Command::Inspect(args) => {
            let InspectArgs {
                file,
                recovery,
                no_verify_checksums,
            } = *args;
            let display_file = file.clone();
            let result = inspect_livephoto(InspectRequest {
                file,
                recovery,
                verify_checksums: !no_verify_checksums,
            })?;
            let parsed = result.parsed;
            let report = result.report;
            println!("file: {}", display_file.display());
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
            let display_file = file.clone();
            let result = unpack_livephoto(UnpackRequest { file, out_dir })?;
            if result.report.is_clean() {
                println!(
                    "unpacked {} to {}",
                    display_file.display(),
                    result.output_dir.display()
                );
            } else {
                println!(
                    "unpacked {} to {} with {} conformance issue(s)",
                    display_file.display(),
                    result.output_dir.display(),
                    result.report.issues.len()
                );
            }
        }
    }
    Ok(())
}
