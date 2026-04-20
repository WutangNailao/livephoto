use std::path::Path;

pub use livephoto_format::{PhotoFormat, VideoFormat};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PhotoInput {
    pub mime: Option<String>,
    pub extension: Option<String>,
}

impl PhotoInput {
    pub fn from_path(path: impl AsRef<Path>) -> Self {
        let extension = path
            .as_ref()
            .extension()
            .and_then(|value| value.to_str())
            .map(ToOwned::to_owned);
        Self {
            mime: None,
            extension,
        }
    }

    pub fn detect_format(&self) -> Result<PhotoFormat, MediaError> {
        detect_photo_format(self.mime.as_deref(), self.extension.as_deref())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VideoInput {
    pub mime: Option<String>,
    pub extension: Option<String>,
}

impl VideoInput {
    pub fn from_path(path: impl AsRef<Path>) -> Self {
        let extension = path
            .as_ref()
            .extension()
            .and_then(|value| value.to_str())
            .map(ToOwned::to_owned);
        Self {
            mime: None,
            extension,
        }
    }

    pub fn detect_format(&self) -> Result<VideoFormat, MediaError> {
        detect_video_format(self.mime.as_deref(), self.extension.as_deref())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhotoOutputPlan {
    pub input: PhotoFormat,
    pub output: PhotoFormat,
}

impl PhotoOutputPlan {
    pub fn target_extension(self) -> &'static str {
        self.output.canonical_extension()
    }

    pub fn target_mime(self) -> &'static str {
        self.output.canonical_mime()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VideoOutputPlan {
    pub input: VideoFormat,
    pub output: VideoFormat,
}

impl VideoOutputPlan {
    pub fn target_extension(self) -> &'static str {
        self.output.canonical_extension()
    }

    pub fn target_mime(self) -> &'static str {
        self.output.canonical_mime()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OutputProfile {
    pub photo: Option<PhotoFormat>,
    pub video: Option<VideoFormat>,
}

pub fn plan_photo_output(
    input: &PhotoInput,
    requested_output: Option<PhotoFormat>,
) -> Result<PhotoOutputPlan, MediaError> {
    let detected = input.detect_format()?;
    let output = requested_output.unwrap_or(detected);
    Ok(PhotoOutputPlan {
        input: detected,
        output,
    })
}

pub fn plan_video_output(
    input: &VideoInput,
    requested_output: Option<VideoFormat>,
) -> Result<VideoOutputPlan, MediaError> {
    let detected = input.detect_format()?;
    let output = requested_output.unwrap_or(detected);
    Ok(VideoOutputPlan {
        input: detected,
        output,
    })
}

fn detect_photo_format(
    mime: Option<&str>,
    extension: Option<&str>,
) -> Result<PhotoFormat, MediaError> {
    match (mime, extension) {
        (Some(mime), Some(extension)) => {
            let from_mime = PhotoFormat::from_mime(mime)
                .ok_or_else(|| MediaError::UnsupportedPhotoFormat(mime.to_string()))?;
            let from_extension = PhotoFormat::from_extension(extension)
                .ok_or_else(|| MediaError::UnsupportedPhotoFormat(extension.to_string()))?;
            if from_mime == from_extension {
                Ok(from_mime)
            } else {
                Err(MediaError::ConflictingPhotoHints {
                    mime: mime.to_string(),
                    extension: extension.to_string(),
                })
            }
        }
        (Some(mime), None) => PhotoFormat::from_mime(mime)
            .ok_or_else(|| MediaError::UnsupportedPhotoFormat(mime.to_string())),
        (None, Some(extension)) => PhotoFormat::from_extension(extension)
            .ok_or_else(|| MediaError::UnsupportedPhotoFormat(extension.to_string())),
        (None, None) => Err(MediaError::MissingPhotoFormatHint),
    }
}

fn detect_video_format(
    mime: Option<&str>,
    extension: Option<&str>,
) -> Result<VideoFormat, MediaError> {
    match (mime, extension) {
        (Some(mime), Some(extension)) => {
            let from_mime = VideoFormat::from_mime(mime)
                .ok_or_else(|| MediaError::UnsupportedVideoFormat(mime.to_string()))?;
            let from_extension = VideoFormat::from_extension(extension)
                .ok_or_else(|| MediaError::UnsupportedVideoFormat(extension.to_string()))?;
            if from_mime == from_extension {
                Ok(from_mime)
            } else {
                Err(MediaError::ConflictingVideoHints {
                    mime: mime.to_string(),
                    extension: extension.to_string(),
                })
            }
        }
        (Some(mime), None) => VideoFormat::from_mime(mime)
            .ok_or_else(|| MediaError::UnsupportedVideoFormat(mime.to_string())),
        (None, Some(extension)) => VideoFormat::from_extension(extension)
            .ok_or_else(|| MediaError::UnsupportedVideoFormat(extension.to_string())),
        (None, None) => Err(MediaError::MissingVideoFormatHint),
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MediaError {
    #[error("unsupported photo format hint: {0}")]
    UnsupportedPhotoFormat(String),
    #[error("unsupported video format hint: {0}")]
    UnsupportedVideoFormat(String),
    #[error("missing photo format hint")]
    MissingPhotoFormatHint,
    #[error("missing video format hint")]
    MissingVideoFormatHint,
    #[error("photo mime `{mime}` conflicts with extension `{extension}`")]
    ConflictingPhotoHints { mime: String, extension: String },
    #[error("video mime `{mime}` conflicts with extension `{extension}`")]
    ConflictingVideoHints { mime: String, extension: String },
}

#[cfg(test)]
mod tests {
    use super::{
        MediaError, PhotoFormat, PhotoInput, VideoFormat, VideoInput, plan_photo_output,
        plan_video_output,
    };

    #[test]
    fn plan_passthrough_photo_output_when_format_matches() {
        let plan = plan_photo_output(
            &PhotoInput {
                mime: Some("image/heic".to_string()),
                extension: Some("heic".to_string()),
            },
            None,
        )
        .unwrap();
        assert_eq!(plan.input, PhotoFormat::Heic);
        assert_eq!(plan.output, PhotoFormat::Heic);
    }

    #[test]
    fn plan_transcoded_photo_output_when_format_changes() {
        let plan = plan_photo_output(
            &PhotoInput {
                mime: Some("image/heic".to_string()),
                extension: Some("heic".to_string()),
            },
            Some(PhotoFormat::Jpeg),
        )
        .unwrap();
        assert_eq!(plan.input, PhotoFormat::Heic);
        assert_eq!(plan.output, PhotoFormat::Jpeg);
        assert_eq!(plan.target_extension(), "jpg");
        assert_eq!(plan.target_mime(), "image/jpeg");
    }

    #[test]
    fn reject_conflicting_photo_hints() {
        let error = plan_photo_output(
            &PhotoInput {
                mime: Some("image/heic".to_string()),
                extension: Some("jpg".to_string()),
            },
            None,
        )
        .unwrap_err();
        assert_eq!(
            error,
            MediaError::ConflictingPhotoHints {
                mime: "image/heic".to_string(),
                extension: "jpg".to_string(),
            }
        );
    }

    #[test]
    fn plan_transcoded_video_output_when_format_changes() {
        let plan = plan_video_output(
            &VideoInput {
                mime: Some("video/quicktime".to_string()),
                extension: Some("mov".to_string()),
            },
            Some(VideoFormat::Mp4),
        )
        .unwrap();
        assert_eq!(plan.input, VideoFormat::QuickTime);
        assert_eq!(plan.output, VideoFormat::Mp4);
        assert_eq!(plan.target_extension(), "mp4");
        assert_eq!(plan.target_mime(), "video/mp4");
    }
}
