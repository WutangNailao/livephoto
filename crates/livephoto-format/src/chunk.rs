use std::io::{Read, Write};

use crate::error::{Error, Result};
use crate::types::{pad_len, read_u16, read_u32, read_u64, write_u16, write_u32, write_u64};

pub const CHUNK_HEADER_SIZE_V1: u16 = 48;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ChunkKind {
    Meta,
    Phot,
    Vide,
    Tocc,
    Thmb,
    Exif,
    Xmp,
    Appl,
    Andr,
    Vend,
    Unknown([u8; 4]),
}

impl ChunkKind {
    pub fn from_bytes(bytes: [u8; 4]) -> Self {
        match &bytes {
            b"META" => Self::Meta,
            b"PHOT" => Self::Phot,
            b"VIDE" => Self::Vide,
            b"TOCC" => Self::Tocc,
            b"THMB" => Self::Thmb,
            b"EXIF" => Self::Exif,
            b"XMP_" => Self::Xmp,
            b"APPL" => Self::Appl,
            b"ANDR" => Self::Andr,
            b"VEND" => Self::Vend,
            _ => Self::Unknown(bytes),
        }
    }

    pub fn as_bytes(self) -> [u8; 4] {
        match self {
            Self::Meta => *b"META",
            Self::Phot => *b"PHOT",
            Self::Vide => *b"VIDE",
            Self::Tocc => *b"TOCC",
            Self::Thmb => *b"THMB",
            Self::Exif => *b"EXIF",
            Self::Xmp => *b"XMP_",
            Self::Appl => *b"APPL",
            Self::Andr => *b"ANDR",
            Self::Vend => *b"VEND",
            Self::Unknown(bytes) => bytes,
        }
    }

    pub fn is_required_for_primary_playback(self) -> bool {
        matches!(self, Self::Meta | Self::Phot | Self::Vide)
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Meta => "META",
            Self::Phot => "PHOT",
            Self::Vide => "VIDE",
            Self::Tocc => "TOCC",
            Self::Thmb => "THMB",
            Self::Exif => "EXIF",
            Self::Xmp => "XMP_",
            Self::Appl => "APPL",
            Self::Andr => "ANDR",
            Self::Vend => "VEND",
            Self::Unknown(_) => "UNKN",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ChunkFlags(pub u64);

impl ChunkFlags {
    pub const REQUIRED_FOR_PRIMARY_PLAYBACK: u64 = 1 << 0;
    pub const COMPRESSED: u64 = 1 << 1;
    pub const ENCRYPTED: u64 = 1 << 2;
    pub const DETACHED_BRIDGE_METADATA: u64 = 1 << 4;

    pub fn contains(self, bit: u64) -> bool {
        self.0 & bit != 0
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LpChunkHeaderV1 {
    pub chunk_type: [u8; 4],
    pub chunk_version: u16,
    pub header_size: u16,
    pub chunk_id: u64,
    pub flags: u64,
    pub stored_length: u64,
    pub crc32c: u64,
    pub reserved: u64,
}

impl LpChunkHeaderV1 {
    pub fn new(
        kind: ChunkKind,
        chunk_id: u64,
        flags: u64,
        payload_len: u64,
        with_crc32c: bool,
    ) -> Self {
        let crc32c = if with_crc32c { 1 } else { 0 };
        Self {
            chunk_type: kind.as_bytes(),
            chunk_version: 1,
            header_size: CHUNK_HEADER_SIZE_V1,
            chunk_id,
            flags,
            stored_length: payload_len,
            crc32c,
            reserved: 0,
        }
    }

    pub fn kind(&self) -> ChunkKind {
        ChunkKind::from_bytes(self.chunk_type)
    }

    pub fn payload_padded_length(&self) -> u64 {
        self.stored_length + pad_len(self.stored_length)
    }

    pub fn total_length(&self) -> u64 {
        u64::from(self.header_size) + self.payload_padded_length()
    }

    pub fn read_from<R: Read>(reader: &mut R) -> Result<Self> {
        let mut chunk_type = [0u8; 4];
        reader.read_exact(&mut chunk_type)?;
        let chunk_version = read_u16(reader)?;
        let header_size = read_u16(reader)?;
        let chunk_id = read_u64(reader)?;
        let flags = read_u64(reader)?;
        let stored_length = read_u64(reader)?;
        let crc32c = read_u64(reader)?;
        let reserved = read_u64(reader)?;
        Ok(Self {
            chunk_type,
            chunk_version,
            header_size,
            chunk_id,
            flags,
            stored_length,
            crc32c,
            reserved,
        })
    }

    pub fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(&self.chunk_type)?;
        write_u16(writer, self.chunk_version)?;
        write_u16(writer, self.header_size)?;
        write_u64(writer, self.chunk_id)?;
        write_u64(writer, self.flags)?;
        write_u64(writer, self.stored_length)?;
        write_u64(writer, self.crc32c)?;
        write_u64(writer, self.reserved)?;
        Ok(())
    }

    pub fn validate(&self, strict: bool) -> Result<()> {
        if self.header_size != CHUNK_HEADER_SIZE_V1 {
            return Err(Error::InvalidOffsetOrLength(format!(
                "chunk {} header_size {} must equal {}",
                self.chunk_id, self.header_size, CHUNK_HEADER_SIZE_V1
            )));
        }
        if strict && self.reserved != 0 {
            return Err(Error::InvalidOffsetOrLength(format!(
                "chunk {} reserved field must be zero",
                self.chunk_id
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ChunkEnvelope {
    pub header: LpChunkHeaderV1,
    pub payload: Vec<u8>,
}

impl ChunkEnvelope {
    pub fn new(
        kind: ChunkKind,
        chunk_id: u64,
        flags: u64,
        payload: Vec<u8>,
        crc32c_enabled: bool,
    ) -> Self {
        let mut header =
            LpChunkHeaderV1::new(kind, chunk_id, flags, payload.len() as u64, crc32c_enabled);
        if crc32c_enabled {
            header.crc32c = u64::from(crc32c::crc32c(&payload));
        }
        Self { header, payload }
    }

    pub fn total_length(&self) -> u64 {
        self.header.total_length()
    }

    pub fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        self.header.write_to(writer)?;
        writer.write_all(&self.payload)?;
        let padding = pad_len(self.payload.len() as u64) as usize;
        if padding > 0 {
            writer.write_all(&vec![0u8; padding])?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkRecord {
    pub header: LpChunkHeaderV1,
    pub file_offset: u64,
    pub total_length: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TocEntryV1 {
    pub chunk_id: u64,
    pub chunk_type: [u8; 4],
    pub file_offset: u64,
    pub total_length: u64,
    pub stored_length: u64,
    pub flags: u64,
}

impl TocEntryV1 {
    pub fn read_from<R: Read>(reader: &mut R) -> Result<Self> {
        let chunk_id = read_u64(reader)?;
        let mut chunk_type = [0u8; 4];
        reader.read_exact(&mut chunk_type)?;
        let _reserved1 = read_u32(reader)?;
        let file_offset = read_u64(reader)?;
        let total_length = read_u64(reader)?;
        let stored_length = read_u64(reader)?;
        let flags = read_u64(reader)?;
        Ok(Self {
            chunk_id,
            chunk_type,
            file_offset,
            total_length,
            stored_length,
            flags,
        })
    }

    pub fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        write_u64(writer, self.chunk_id)?;
        writer.write_all(&self.chunk_type)?;
        write_u32(writer, 0)?;
        write_u64(writer, self.file_offset)?;
        write_u64(writer, self.total_length)?;
        write_u64(writer, self.stored_length)?;
        write_u64(writer, self.flags)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TocPayloadV1 {
    pub entries: Vec<TocEntryV1>,
}

impl TocPayloadV1 {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let mut cursor = std::io::Cursor::new(bytes);
        let entry_count = read_u32(&mut cursor)? as usize;
        let _reserved0 = read_u32(&mut cursor)?;
        let mut entries = Vec::with_capacity(entry_count);
        for _ in 0..entry_count {
            entries.push(TocEntryV1::read_from(&mut cursor)?);
        }
        Ok(Self { entries })
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::with_capacity(8 + self.entries.len() * 40);
        write_u32(&mut buf, self.entries.len() as u32)?;
        write_u32(&mut buf, 0)?;
        for entry in &self.entries {
            entry.write_to(&mut buf)?;
        }
        Ok(buf)
    }
}
