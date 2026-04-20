# livephoto-media

`livephoto-media` is the workspace crate for image and video output format conversion planning.

This crate is intentionally platform-neutral. It is meant for cases such as:

- photo output conversion like `heic -> jpg`
- video output conversion like `mov -> mp4`
- comparing input and target formats before transcoding

It is not responsible for Apple Live Photo packaging, Android Motion Photo packaging, or any other platform-specific bridge logic.

The input/output format enums are reused from `livephoto-format`, so supported media types stay aligned with what `.livephoto` itself accepts.

## Scope

Current responsibilities:

- image format detection from MIME type and/or file extension
- video format detection from MIME type and/or file extension
- output planning for target image and video formats
- a shared place for future transcoding backends

Typical layering:

- `livephoto-format`: `.livephoto` container format
- `livephoto-toolkit`: asset-level workflows
- `livephoto-media`: media output conversion
- `bridges/apple`, `bridges/android`: platform-specific import/export

## Example

```rust
use livephoto_media::{PhotoFormat, PhotoInput, VideoFormat, VideoInput, plan_photo_output, plan_video_output};

let photo_plan = plan_photo_output(
    &PhotoInput {
        mime: Some("image/heic".into()),
        extension: Some("heic".into()),
    },
    Some(PhotoFormat::Jpeg),
)?;

let video_plan = plan_video_output(
    &VideoInput {
        mime: Some("video/quicktime".into()),
        extension: Some("mov".into()),
    },
    Some(VideoFormat::Mp4),
)?;
```

Callers can decide whether transcoding is needed by comparing `photo_plan.input` with `photo_plan.output`, and `video_plan.input` with `video_plan.output`.
