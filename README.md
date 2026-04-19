# livephoto

`livephoto` is a cross-platform dynamic photo container project.

It defines a single-file `.livephoto` format for packaging:

- a cover image
- a short motion video
- manifest metadata
- optional bridge and extension metadata

The goal is not to reproduce Apple's private Live Photo representation. The goal is to provide an open, implementable, single-file container and tooling layer for dynamic photos across Web, mobile, desktop, and backend systems.

## Why

Existing dynamic photo solutions are fragmented.

- Apple Live Photo is powerful inside the Apple ecosystem, but it is fundamentally a paired-resource model.
- Android Motion Photo is closer to a single-file model, but ecosystem behavior and metadata still vary.
- Many applications end up treating dynamic photos as ad hoc "image + video" bundles instead of a formal media object.

This project exists to make dynamic photos easier to:

- store
- transfer
- inspect
- bridge across ecosystems
- play outside vendor-specific frameworks

## What This Repository Contains

- `docs/livephoto-format-v1.md`
  English format specification
- `docs/livephoto-format-v1.zh-CN.md`
  Chinese format specification
- `docs/project-introduction.md`
  English project introduction
- `docs/project-introduction.zh-CN.md`
  Chinese project introduction
- `crates/livephoto-format`
  Rust implementation of the container format
- `crates/livephoto-toolkit`
  Reusable high-level Rust APIs for packing, inspecting, and unpacking `.livephoto` files
- `crates/livephoto-cli`
  CLI for packing, inspecting, and unpacking `.livephoto` files
- `web/`
  Minimal Web player

## Current Capabilities

The current implementation supports:

- file header, chunk header, and TOC parsing/writing
- required `META`, `PHOT`, `VIDE`, and `TOCC` semantics
- optional chunks such as thumbnail, EXIF, XMP, hash, Apple bridge, Android bridge, signature, vendor extensions, and unknown chunks
- manifest validation
- checksum verification
- recovery-mode chunk scanning if TOC parsing fails
- lossless preservation of unknown optional chunks during rewrite
- a minimal browser-based player for local `.livephoto` files

## Quick Start

### Build and Check

```bash
cd /Users/nailao/Code/livephoto
cargo test
cargo clippy --workspace --all-targets -- -D warnings
```

### Pack a `.livephoto`

```bash
cd /Users/nailao/Code/livephoto

cargo run -p livephoto-cli -- \
  pack \
  --manifest /path/to/manifest.json \
  --photo /path/to/photo.heic \
  --video /path/to/video.mov \
  --out /path/to/output.livephoto
```

### Inspect a `.livephoto`

```bash
cd /Users/nailao/Code/livephoto

cargo run -p livephoto-cli -- inspect /path/to/output.livephoto
```

### Call from Rust Code

```rust
use livephoto_toolkit::{pack_livephoto, PackRequest};

pack_livephoto(PackRequest {
    manifest: "/path/to/manifest.json".into(),
    photo: "/path/to/photo.heic".into(),
    video: "/path/to/video.mov".into(),
    out: "/path/to/output.livephoto".into(),
    thumbnail: None,
    exif_raw: None,
    xmp: None,
    hash_json: None,
    apple_bridge_json: None,
    android_bridge_json: None,
    emit_crc32c: true,
})?;
```

### Unpack a `.livephoto`

```bash
cd /Users/nailao/Code/livephoto

cargo run -p livephoto-cli -- \
  unpack \
  /path/to/output.livephoto \
  --out-dir /path/to/unpacked
```

### Run the Minimal Web Player

```bash
cd /Users/nailao/Code/livephoto/web
python3 -m http.server 8080
```

Then open:

[http://localhost:8080](http://localhost:8080)

## Format Notes

The current format is chunk-based and uses:

- magic: `LPHF`
- little-endian integer encoding
- a fixed 68-byte file header
- a fixed 48-byte chunk header
- 8-byte chunk payload alignment
- JSON manifest payloads for `META`

See the full specification for details:

- [English spec](./docs/livephoto-format-v1.md)
- [中文规范](./docs/livephoto-format-v1.zh-CN.md)

## Project Positioning

This project should be understood as:

- a cross-platform dynamic photo container
- a tooling layer for parsing and generating dynamic photo assets
- a bridge layer between Apple Live Photo, Android Motion Photo, and custom playback environments

It should not be understood as:

- Apple's native Live Photo format
- a replacement for system photo libraries
- a custom image codec or video codec

## Roadmap Direction

Reasonable next steps for the project include:

- richer Web player controls
- iOS and Android SDKs
- Apple and Android import/export bridge tooling
- editing tools for cover replacement, trimming, and metadata updates
- backend-oriented transcoding and inspection workflows

## License

MIT. See [LICENSE](./LICENSE).
