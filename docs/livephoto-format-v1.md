# LivePhoto Container Format v1

Status: Draft

Document version: 0.1.0

Target implementation language: Rust

File extension: `.livephoto`

MIME type proposal: `application/x-livephoto`

Magic: `LPHF`

## 1. Scope

This document defines a single-file binary container format for dynamic photos.

The format is intended to support:

- single-file storage and transmission
- cross-platform parsing and playback
- encapsulation of a cover image and a short motion clip
- future-compatible metadata and extension blocks
- import/export bridges to Apple Live Photo and Android Motion Photo ecosystems

This format is not intended to be wire-compatible with Apple's native Live Photo representation. Apple export/import should be implemented as adapters.

## 2. Design Goals

The format is designed with the following priorities:

1. A `.livephoto` file must be self-contained.
2. A parser must be able to locate required resources without scanning the full file.
3. The cover image must have explicit semantic meaning.
4. The container must support progressive evolution without breaking v1 readers.
5. Implementations in Rust should be straightforward, explicit, and safe.
6. Non-media metadata must be decoupled from encoded media streams.

## 3. High-Level Model

A `.livephoto` file contains:

- one required manifest chunk
- one required primary cover image chunk
- one required primary motion video chunk
- zero or more optional chunks for thumbnails, hashes, EXIF, XMP, bridge metadata, alternate renditions, signatures, and vendor extensions

The semantic model is:

- the cover image is the default still representation
- the motion video is the dynamic representation
- the manifest defines timing, dimensions, playback defaults, and chunk relationships
- all binary resources are stored as opaque payloads with typed chunk headers

## 4. Binary Conventions

### 4.1 Endianness

All integer fields in the container header, table of contents, and chunk headers are little-endian.

### 4.2 Alignment

All chunks are 8-byte aligned.

Writers:

- must pad each chunk payload to the next 8-byte boundary
- must set the `stored_length` field to the unpadded payload byte length
- must write zero bytes in padding

Readers:

- must use `stored_length` for payload parsing
- must ignore padding bytes

### 4.3 Character Encoding

- fixed-size identifiers use ASCII
- JSON content uses UTF-8
- string fields in future binary structures use UTF-8 unless explicitly documented otherwise

### 4.4 Time Units

All manifest duration and timestamp values are in milliseconds unless explicitly stated otherwise.

### 4.5 UUID Representation

Any UUID represented in JSON must use canonical lowercase hyphenated text form, for example:

`"550e8400-e29b-41d4-a716-446655440000"`

This section defines the JSON text form for values that are UUIDs.
It does not imply that every identifier field in the format is a UUID.

## 5. File Layout

The top-level layout is:

```text
+-------------------+
| FileHeader        |
+-------------------+
| Chunk 0           |
+-------------------+
| Chunk 1           |
+-------------------+
| ...               |
+-------------------+
| Chunk N           |
+-------------------+
| TOC Chunk         |
+-------------------+
```

The TOC chunk is also a regular chunk, but its type is reserved and required exactly once.

The TOC should be placed at the end of the file in v1 so a writer can stream chunks first and finalize the index last.

## 6. File Header

The file begins with a fixed 68-byte header.

### 6.1 Binary Layout

```c
struct LPFileHeaderV1 {
    char     magic[4];             // "LPHF"
    uint16_t major_version;        // 1
    uint16_t minor_version;        // 0 for first stable v1 release
    uint32_t header_size;          // must be 68 in v1
    uint64_t flags;                // file-level flags
    uint64_t toc_offset;           // absolute byte offset of TOC chunk header
    uint64_t toc_length;           // full TOC chunk length including header and padding
    uint64_t file_size;            // total file size in bytes
    uint64_t primary_manifest_id;  // chunk id of primary META chunk
    uint64_t reserved[2];          // must be zero in v1
};
```

### 6.2 Header Rules

Writers:

- must write `magic = "LPHF"`
- must write `major_version = 1`
- should write `minor_version = 0`
- must write `header_size = 68`
- must set all unknown reserved fields to zero
- must set `toc_offset`, `toc_length`, and `file_size` consistently
- must set `primary_manifest_id` to the chunk id of the required `META` chunk

Readers:

- must reject invalid magic
- must reject `header_size != 68`
- must reject `toc_offset == 0`
- must reject `file_size < 68`
- should reject inconsistent offset or length fields
- may reject non-zero reserved fields in strict mode

### 6.3 File Flags

File flags are a `u64` bitset.

Defined bits in v1:

- bit 0: file contains integrity hashes
- bit 1: file contains a signature block
- bit 2: file contains encrypted chunks
- bit 3: file contains bridge metadata for Apple Live Photo
- bit 4: file contains bridge metadata for Android Motion Photo

All other bits are reserved for future use and must be zero in v1 writers.

## 7. Chunk Model

Each chunk consists of a fixed-size header and a variable-size payload.

### 7.1 Chunk Header

```c
struct LPChunkHeaderV1 {
    char     chunk_type[4];        // ASCII, examples: "META", "PHOT", "VIDE"
    uint16_t chunk_version;        // chunk schema version, usually 1
    uint16_t header_size;          // must be 48 in v1
    uint64_t chunk_id;             // unique within file
    uint64_t flags;                // chunk-level flags
    uint64_t stored_length;        // payload length excluding padding
    uint64_t crc32c;               // low 32 bits used, upper bits zero in v1
    uint64_t reserved;             // must be zero in v1
};
```

The chunk header size is fixed at 48 bytes in v1.

Writers:

- must write `header_size = 48`

Readers:

- must reject `header_size != 48`

### 7.2 Chunk Ordering

Chunk order is not semantically significant except:

- `file_header.toc_offset` must point to one canonical `TOCC` chunk
- `META`, `PHOT`, and `VIDE` chunks required by the primary asset must exist
- the `TOCC` chunk should be last

Strict readers must reject files containing any additional non-canonical `TOCC` chunks.
Recovery readers may ignore additional non-canonical `TOCC` chunks.

Recommended write order:

1. `META`
2. `PHOT`
3. `VIDE`
4. optional chunks
5. `TOCC`

### 7.3 Chunk Flags

Chunk flags are a `u64` bitset.

Defined bits in v1:

- bit 0: chunk is required for primary playback
- bit 1: chunk payload is compressed
- bit 2: chunk payload is encrypted
- bit 3: chunk is an alternate rendition
- bit 4: chunk is detached bridge metadata only

Unknown bits must be ignored by readers unless strict validation is enabled.

### 7.4 Integrity

`crc32c` is optional in v1.

Rules:

- if `crc32c == 0`, no per-chunk checksum is provided
- if non-zero, readers should verify it against the unpadded payload bytes
- writers should either provide valid CRC32C or write zero

## 8. Table of Contents Chunk

The TOC chunk type is `TOCC`.

Its payload is a binary index that allows direct lookup of chunk locations.

### 8.1 TOC Payload Layout

```c
struct LPToCPayloadV1 {
    uint32_t entry_count;
    uint32_t reserved0;
    LPToCEntryV1 entries[entry_count];
};

struct LPToCEntryV1 {
    uint64_t chunk_id;
    char     chunk_type[4];
    uint32_t reserved1;
    uint64_t file_offset;          // absolute offset of chunk header
    uint64_t total_length;         // chunk header + payload + padding
    uint64_t stored_length;        // payload only, no padding
    uint64_t flags;
};
```

### 8.2 TOC Requirements

Writers:

- must include every non-TOC chunk in the TOC
- must not include the canonical `TOCC` itself as an entry
- must ensure chunk ids are unique

Readers:

- should prefer TOC-based navigation
- must treat the `TOCC` referenced by `file_header.toc_offset` as the canonical TOC
- must reject canonical TOC entries whose `chunk_type` is `TOCC`
- may fall back to linear scan if TOC validation fails and recovery mode is enabled

Vendor-defined auxiliary indexes must use `VEND` or another non-`TOCC` chunk type.

## 9. Required Chunk Types

### 9.1 `META` Manifest Chunk

The `META` chunk payload is UTF-8 JSON.

It defines logical relationships, playback defaults, and metadata schema values.

Exactly one primary `META` chunk is required in v1.

#### 9.1.1 Required Manifest Fields

```json
{
  "schema": "livephoto/v1",
  "asset_id": "asset-demo-01",
  "created_at_ms": 1776038462000,
  "duration_ms": 1800,
  "width": 1440,
  "height": 1920,
  "cover_timestamp_ms": 900,
  "photo_chunk_id": 2,
  "video_chunk_id": 3,
  "photo_mime": "image/jpeg",
  "video_mime": "video/mp4",
  "has_audio": true,
  "playback": {
    "autoplay": false,
    "loop": false,
    "bounce": false,
    "muted_by_default": false,
    "return_to_cover": true
  }
}
```

#### 9.1.2 Manifest Schema

The following fields are defined in v1.

Required root fields:

- `schema`: must be `"livephoto/v1"`
- `asset_id`: opaque unique string identifying the logical asset; UUID recommended but not required; if a UUID is used, it must follow Section 4.5
- `created_at_ms`: logical creation timestamp
- `duration_ms`: total motion duration
- `width`: display width in pixels
- `height`: display height in pixels
- `cover_timestamp_ms`: timestamp in motion clip corresponding to the cover image
- `photo_chunk_id`: chunk id of primary cover image
- `video_chunk_id`: chunk id of primary motion video
- `photo_mime`: MIME type of cover image payload
- `video_mime`: MIME type of motion video payload
- `has_audio`: whether the primary motion video contains audible media
- `playback`: playback policy object

Optional root fields:

- `title`
- `description`
- `author`
- `timezone`
- `rotation_degrees`
- `thumbnail_chunk_id`
- `exif_chunk_id`
- `xmp_chunk_id`
- `hash_chunk_id`
- `apple_bridge_chunk_id`
- `android_bridge_chunk_id`
- `tags`
- `capture_device`
- `software`
- `color_space`
- `alpha_mode`
- `poster_strategy`
- `preferred_seek_pre_roll_ms`
- `extensions`

#### 9.1.3 Playback Object

Defined `playback` fields:

- `autoplay`: boolean
- `loop`: boolean
- `bounce`: boolean
- `muted_by_default`: boolean
- `return_to_cover`: boolean
- `hold_last_frame`: boolean, optional
- `interaction_hint`: optional string enum

Allowed `interaction_hint` values in v1:

- `"tap"`
- `"hover"`
- `"press"`
- `"viewport"`
- `"programmatic"`

#### 9.1.4 Manifest Validation Rules

Readers should validate:

- `duration_ms > 0`
- `width > 0`
- `height > 0`
- `cover_timestamp_ms <= duration_ms`
- `photo_chunk_id != video_chunk_id`
- `photo_mime` and `video_mime` match actual chunk payload formats when detectable

### 9.2 `PHOT` Primary Cover Image Chunk

The `PHOT` chunk stores the primary still image.

Payload:

- raw encoded image bytes

Supported MIME types in v1:

- `image/jpeg`
- `image/heic`
- `image/heif`
- `image/avif`
- `image/png`
- `image/webp`

Writers should prefer:

- `image/jpeg` for maximum compatibility
- `image/heic` for Apple bridge workflows

Readers must:

- treat `PHOT` payload as opaque encoded bytes
- not assume a specific codec beyond manifest metadata

### 9.3 `VIDE` Primary Motion Video Chunk

The `VIDE` chunk stores the primary motion clip.

Payload:

- raw encoded video container bytes

Supported MIME types in v1:

- `video/mp4`
- `video/quicktime`
- `video/webm`

Writers should prefer:

- `video/mp4` with H.264/AAC for broadest compatibility

Readers must:

- treat `VIDE` payload as opaque encoded bytes
- not assume sample-level structure from the `.livephoto` container alone

## 10. Optional Chunk Types

### 10.1 `THMB` Thumbnail Chunk

Stores a low-cost preview image.

Payload:

- raw encoded image bytes

Recommended use:

- list views
- quick previews
- server-side indexing

### 10.2 `EXIF` Metadata Chunk

Stores EXIF metadata in one of the following representations:

- raw EXIF block
- UTF-8 JSON representation

The manifest should define the representation via:

```json
{
  "extensions": {
    "exif_format": "raw"
  }
}
```

or

```json
{
  "extensions": {
    "exif_format": "json"
  }
}
```

Writers should prefer raw retention if preserving source fidelity matters.

### 10.3 `XMP_` XMP Metadata Chunk

Stores XMP packet bytes as UTF-8 XML.

The chunk type is `XMP_` because chunk types are fixed 4-byte ASCII codes.

### 10.4 `HASH` Integrity Hashes Chunk

Stores cryptographic hashes for selected chunks or the whole file.

Payload format in v1 is UTF-8 JSON:

```json
{
  "alg": "sha256",
  "file": "hex...",
  "chunks": {
    "2": "hex...",
    "3": "hex..."
  }
}
```

### 10.5 `APPL` Apple Bridge Chunk

Stores metadata needed to reconstruct an Apple-compatible Live Photo pair.

Recommended fields:

```json
{
  "asset_identifier": "550e8400-e29b-41d4-a716-446655440000",
  "still_image_time_ms": 900,
  "photo_codec_hint": "image/heic",
  "video_codec_hint": "video/quicktime",
  "maker_apple_key_17": "550e8400-e29b-41d4-a716-446655440000",
  "quicktime_content_identifier": "550e8400-e29b-41d4-a716-446655440000"
}
```

This chunk does not make the file natively readable by Apple Photos.
It only preserves bridge data for export.

### 10.6 `ANDR` Android Bridge Chunk

Stores metadata needed to reconstruct Android Motion Photo compatible outputs.

Recommended fields:

```json
{
  "presentation_timestamp_us": 900000,
  "xmp_format": "container",
  "primary_image_role": "display",
  "embedded_video_role": "motion"
}
```

### 10.7 `ALTV` Alternate Video Chunk

Stores an alternate motion video rendition.

Typical use:

- lower bitrate variant
- HEVC variant
- WebM variant

Its relationship to primary playback must be declared in `META.extensions` or a future structured manifest field.

### 10.8 `ALTP` Alternate Photo Chunk

Stores an alternate still image rendition.

Typical use:

- JPEG fallback for HEIC primary
- lower resolution preview
- transparent PNG asset

### 10.9 `SIGN` Signature Chunk

Stores a detached signature or certificate chain.

The exact cryptographic schema is out of scope for v1 but the chunk type is reserved.

### 10.10 `VEND` Vendor Extension Chunk

Stores implementation-specific data.

Writers using `VEND` should namespace the payload with a top-level vendor id if JSON is used.

Readers may ignore unknown optional chunks.

## 11. MIME and Codec Rules

The container does not define image or video codecs itself.
It only transports encoded payloads.

Rules:

- the manifest must declare `photo_mime` and `video_mime`
- optional alternate renditions should declare their MIME in manifest extensions
- readers should sniff payloads when possible and warn on mismatches
- writers should not rely on file extension inside payloads

## 12. Playback Semantics

The format distinguishes storage from playback semantics.

### 12.1 Default Rendering

Default state:

- render the cover image
- do not autoplay unless `playback.autoplay == true`
- honor `muted_by_default` if video audio exists

### 12.2 Cover Semantics

`cover_timestamp_ms` defines the exact semantic relationship between still and motion.

Usage:

- if the cover image was extracted from the video, this points to the source frame
- if the cover image was independently captured, this points to the nearest representative frame
- players should seek to this time when transitioning back to cover semantics

### 12.3 End-of-Playback Policy

If `return_to_cover == true`:

- the player should restore the cover representation after playback ends

If `hold_last_frame == true` and `return_to_cover == false`:

- the player may remain on the final decoded frame

If `bounce == true`:

- a player may play forward then reverse
- if reverse playback is unsupported, the player may ignore `bounce`

## 13. Compatibility Policy

### 13.1 Forward Compatibility

Readers should:

- reject unknown `major_version`
- ignore unknown optional chunk types
- ignore unknown manifest fields
- preserve unknown chunks during lossless rewrite when possible

### 13.2 Backward Compatibility

Future v1.x revisions must:

- preserve the 68-byte file header contract
- preserve the 48-byte chunk header contract
- not use larger `header_size` values to introduce header extensions within v1.x
- avoid changing semantics of existing required fields
- add only optional fields or new optional chunk types

## 14. Error Handling

Suggested Rust error categories:

- invalid magic
- unsupported major version
- malformed header
- malformed TOC
- duplicate or non-canonical `TOCC`
- duplicate chunk id
- required chunk missing
- manifest parse failure
- manifest validation failure
- checksum mismatch
- invalid offset or length
- unsupported codec
- I/O failure

Strict mode should fail on:

- invalid header fields
- missing required chunks
- manifest inconsistencies
- overlapping chunk offsets
- additional non-canonical `TOCC` chunks
- canonical TOC entries that reference `TOCC`

Recovery mode may:

- scan linearly if TOC is corrupted
- ignore invalid optional chunks
- ignore checksum absence

## 15. Streaming and Random Access

v1 is optimized for file-based access, not live streaming.

However:

- the header provides direct access to the TOC
- the TOC provides direct offsets to chunk headers
- a parser can memory-map the file and borrow payload slices

Writers should not assume payloads are small enough to duplicate in memory.

## 16. Security Considerations

Readers must:

- validate all lengths before allocation
- validate all offsets against file bounds
- reject overlapping or out-of-range chunk regions
- impose maximum JSON manifest size limits
- avoid trusting MIME declarations without optional sniffing
- treat embedded media codecs as untrusted input

Suggested implementation limits for v1:

- maximum manifest size: 1 MiB
- maximum thumbnail size: 32 MiB
- maximum file size: implementation-defined, but use checked arithmetic everywhere

## 17. Rust Mapping

The following Rust types are recommended for a reference implementation.

```rust
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FileHeaderV1 {
    pub magic: [u8; 4],
    pub major_version: u16,
    pub minor_version: u16,
    pub header_size: u32,
    pub flags: u64,
    pub toc_offset: u64,
    pub toc_length: u64,
    pub file_size: u64,
    pub primary_manifest_id: u64,
    pub reserved: [u64; 2],
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ChunkHeaderV1 {
    pub chunk_type: [u8; 4],
    pub chunk_version: u16,
    pub header_size: u16,
    pub chunk_id: u64,
    pub flags: u64,
    pub stored_length: u64,
    pub crc32c: u64,
    pub reserved: u64,
}
```

Suggested parsed model:

```rust
#[derive(Debug, Clone)]
pub struct LivePhotoFile {
    pub header: FileHeaderV1,
    pub chunks: Vec<ChunkRecord>,
    pub manifest: ManifestV1,
}

#[derive(Debug, Clone)]
pub struct ChunkRecord {
    pub header: ChunkHeaderV1,
    pub file_offset: u64,
    pub total_length: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
    #[serde(default)]
    pub extensions: serde_json::Value,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlaybackPolicyV1 {
    pub autoplay: bool,
    pub loop: bool,
    pub bounce: bool,
    pub muted_by_default: bool,
    pub return_to_cover: bool,
    #[serde(default)]
    pub hold_last_frame: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interaction_hint: Option<String>,
}
```

## 18. Writer Algorithm

Recommended write procedure:

1. Validate input image and video assets.
2. Assign stable chunk ids.
3. Build the manifest JSON with resolved chunk references.
4. Write the fixed file header with placeholder TOC and file size fields.
5. Write `META`, `PHOT`, `VIDE`, and optional chunks sequentially.
6. Record each chunk offset and total length.
7. Build and write the canonical `TOCC` chunk containing only non-`TOCC` entries.
8. Seek back and finalize the file header.
9. Optionally compute and write hashes before finalization if your pipeline supports it.

## 19. Reader Algorithm

Recommended read procedure:

1. Read and validate the fixed header.
2. Seek to `toc_offset`.
3. Parse and validate the canonical `TOCC`.
4. Resolve the primary `META` chunk using `primary_manifest_id`.
5. Parse manifest JSON.
6. Resolve `photo_chunk_id` and `video_chunk_id`.
7. Expose borrowed payload slices or stream readers for image and video data.

Strict readers should then linearly scan the file to confirm that no additional `TOCC` chunks exist.

## 20. Minimal Valid File

A minimal valid `.livephoto` file contains:

- valid file header
- one `META` chunk
- one `PHOT` chunk
- one `VIDE` chunk
- one `TOCC` chunk

No thumbnail, hash, EXIF, XMP, or bridge chunk is required.

## 21. Example Manifest

```json
{
  "schema": "livephoto/v1",
  "asset_id": "asset-demo-01",
  "created_at_ms": 1776070800000,
  "duration_ms": 1500,
  "width": 1080,
  "height": 1440,
  "cover_timestamp_ms": 800,
  "photo_chunk_id": 2,
  "video_chunk_id": 3,
  "photo_mime": "image/jpeg",
  "video_mime": "video/mp4",
  "has_audio": true,
  "playback": {
    "autoplay": false,
    "loop": false,
    "bounce": false,
    "muted_by_default": false,
    "return_to_cover": true,
    "hold_last_frame": false,
    "interaction_hint": "press"
  },
  "title": "sample asset",
  "software": "livephoto-rs/0.1.0",
  "extensions": {
    "thumbnail_mime": "image/jpeg"
  }
}
```

## 22. Recommended Future Extensions

The following are intentionally left for future revisions:

- chunk-level compression envelopes
- chunk-level encryption envelopes
- multiple audio tracks
- subtitle or caption tracks
- embedded image sequences instead of only a single video
- edit decision lists
- binary manifest encoding such as CBOR
- delta-friendly chunk packing for sync systems

## 23. Practical Guidance

For a first Rust implementation, use:

- JSON for manifest payloads
- `image/jpeg` or `image/heic` for `PHOT`
- `video/mp4` for `VIDE`
- `serde_json` for manifest parsing
- `crc32c` only if integrity checks are needed immediately

Do not implement in v1 initial milestone:

- encryption
- signature validation
- multi-rendition selection logic
- lossy rewrite of unknown chunks

## 24. Suggested Repository Layout

If you implement this in Rust, a clean starting structure would be:

```text
livephoto/
  crates/
    livephoto-format/     # binary structs, parser, writer
    livephoto-codec/      # bridge and media helpers
    livephoto-player/     # playback abstraction
    livephoto-cli/        # pack, unpack, inspect
  docs/
    livephoto-format-v1.md
```

## 25. Summary

`.livephoto` v1 is a chunk-based binary container with:

- a fixed 68-byte file header
- a fixed 48-byte chunk header
- required `META`, `PHOT`, `VIDE`, and `TOCC` semantics
- JSON manifest-driven relationships
- opaque media payloads
- strong forward-compatibility posture

It is designed to be simple enough for an initial Rust implementation while leaving a clean migration path toward a richer long-term media container.
