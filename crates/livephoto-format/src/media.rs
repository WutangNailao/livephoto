#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhotoFormat {
    Jpeg,
    Heic,
    Heif,
    Avif,
    Png,
    Webp,
}

impl PhotoFormat {
    pub fn canonical_mime(self) -> &'static str {
        match self {
            Self::Jpeg => "image/jpeg",
            Self::Heic => "image/heic",
            Self::Heif => "image/heif",
            Self::Avif => "image/avif",
            Self::Png => "image/png",
            Self::Webp => "image/webp",
        }
    }

    pub fn canonical_extension(self) -> &'static str {
        match self {
            Self::Jpeg => "jpg",
            Self::Heic => "heic",
            Self::Heif => "heif",
            Self::Avif => "avif",
            Self::Png => "png",
            Self::Webp => "webp",
        }
    }

    pub fn from_mime(mime: &str) -> Option<Self> {
        let normalized = mime.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "image/jpeg" | "image/jpg" => Some(Self::Jpeg),
            "image/heic" => Some(Self::Heic),
            "image/heif" => Some(Self::Heif),
            "image/avif" => Some(Self::Avif),
            "image/png" => Some(Self::Png),
            "image/webp" => Some(Self::Webp),
            _ => None,
        }
    }

    pub fn from_extension(extension: &str) -> Option<Self> {
        let normalized = extension
            .trim()
            .trim_start_matches('.')
            .to_ascii_lowercase();
        match normalized.as_str() {
            "jpg" | "jpeg" => Some(Self::Jpeg),
            "heic" => Some(Self::Heic),
            "heif" => Some(Self::Heif),
            "avif" => Some(Self::Avif),
            "png" => Some(Self::Png),
            "webp" => Some(Self::Webp),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoFormat {
    Mp4,
    QuickTime,
    WebM,
}

impl VideoFormat {
    pub fn canonical_mime(self) -> &'static str {
        match self {
            Self::Mp4 => "video/mp4",
            Self::QuickTime => "video/quicktime",
            Self::WebM => "video/webm",
        }
    }

    pub fn canonical_extension(self) -> &'static str {
        match self {
            Self::Mp4 => "mp4",
            Self::QuickTime => "mov",
            Self::WebM => "webm",
        }
    }

    pub fn from_mime(mime: &str) -> Option<Self> {
        let normalized = mime.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "video/mp4" => Some(Self::Mp4),
            "video/quicktime" => Some(Self::QuickTime),
            "video/webm" => Some(Self::WebM),
            _ => None,
        }
    }

    pub fn from_extension(extension: &str) -> Option<Self> {
        let normalized = extension
            .trim()
            .trim_start_matches('.')
            .to_ascii_lowercase();
        match normalized.as_str() {
            "mp4" => Some(Self::Mp4),
            "mov" | "qt" => Some(Self::QuickTime),
            "webm" => Some(Self::WebM),
            _ => None,
        }
    }
}
