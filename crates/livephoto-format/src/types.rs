use std::io::{Read, Write};

use crate::error::{Error, Result};

pub const MAGIC: [u8; 4] = *b"LPHF";
pub const FILE_HEADER_SIZE_V1: u32 = 68;
pub const MAJOR_VERSION_V1: u16 = 1;
pub const MINOR_VERSION_V1: u16 = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FileFlags(pub u64);

impl FileFlags {
    pub const HASHES_PRESENT: u64 = 1 << 0;
    pub const SIGNATURE_PRESENT: u64 = 1 << 1;
    pub const ENCRYPTED_CHUNKS_PRESENT: u64 = 1 << 2;
    pub const APPLE_BRIDGE_PRESENT: u64 = 1 << 3;
    pub const ANDROID_BRIDGE_PRESENT: u64 = 1 << 4;

    pub fn contains(self, bit: u64) -> bool {
        self.0 & bit != 0
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LpFileHeaderV1 {
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

impl LpFileHeaderV1 {
    pub fn new(primary_manifest_id: u64) -> Self {
        Self {
            magic: MAGIC,
            major_version: MAJOR_VERSION_V1,
            minor_version: MINOR_VERSION_V1,
            header_size: FILE_HEADER_SIZE_V1,
            flags: 0,
            toc_offset: 0,
            toc_length: 0,
            file_size: 0,
            primary_manifest_id,
            reserved: [0; 2],
        }
    }

    pub fn read_from<R: Read>(reader: &mut R) -> Result<Self> {
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;
        let major_version = read_u16(reader)?;
        let minor_version = read_u16(reader)?;
        let header_size = read_u32(reader)?;
        let flags = read_u64(reader)?;
        let toc_offset = read_u64(reader)?;
        let toc_length = read_u64(reader)?;
        let file_size = read_u64(reader)?;
        let primary_manifest_id = read_u64(reader)?;
        let reserved = [read_u64(reader)?, read_u64(reader)?];
        Ok(Self {
            magic,
            major_version,
            minor_version,
            header_size,
            flags,
            toc_offset,
            toc_length,
            file_size,
            primary_manifest_id,
            reserved,
        })
    }

    pub fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(&self.magic)?;
        write_u16(writer, self.major_version)?;
        write_u16(writer, self.minor_version)?;
        write_u32(writer, self.header_size)?;
        write_u64(writer, self.flags)?;
        write_u64(writer, self.toc_offset)?;
        write_u64(writer, self.toc_length)?;
        write_u64(writer, self.file_size)?;
        write_u64(writer, self.primary_manifest_id)?;
        write_u64(writer, self.reserved[0])?;
        write_u64(writer, self.reserved[1])?;
        Ok(())
    }

    pub fn validate(&self, strict: bool) -> Result<()> {
        if self.magic != MAGIC {
            return Err(Error::InvalidMagic);
        }
        if self.major_version != MAJOR_VERSION_V1 {
            return Err(Error::UnsupportedMajorVersion(self.major_version));
        }
        if self.header_size != FILE_HEADER_SIZE_V1 {
            return Err(Error::MalformedHeader(format!(
                "header_size {} must equal {}",
                self.header_size, FILE_HEADER_SIZE_V1
            )));
        }
        if self.toc_offset == 0 {
            return Err(Error::MalformedHeader(
                "toc_offset must not be zero".to_string(),
            ));
        }
        if self.file_size < u64::from(FILE_HEADER_SIZE_V1) {
            return Err(Error::MalformedHeader("file_size too small".to_string()));
        }
        if strict && self.reserved != [0, 0] {
            return Err(Error::MalformedHeader(
                "reserved fields must be zero in strict mode".to_string(),
            ));
        }
        Ok(())
    }
}

pub(crate) fn read_u16<R: Read>(reader: &mut R) -> Result<u16> {
    let mut buf = [0u8; 2];
    reader.read_exact(&mut buf)?;
    Ok(u16::from_le_bytes(buf))
}

pub(crate) fn read_u32<R: Read>(reader: &mut R) -> Result<u32> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

pub(crate) fn read_u64<R: Read>(reader: &mut R) -> Result<u64> {
    let mut buf = [0u8; 8];
    reader.read_exact(&mut buf)?;
    Ok(u64::from_le_bytes(buf))
}

pub(crate) fn write_u16<W: Write>(writer: &mut W, value: u16) -> Result<()> {
    writer.write_all(&value.to_le_bytes())?;
    Ok(())
}

pub(crate) fn write_u32<W: Write>(writer: &mut W, value: u32) -> Result<()> {
    writer.write_all(&value.to_le_bytes())?;
    Ok(())
}

pub(crate) fn write_u64<W: Write>(writer: &mut W, value: u64) -> Result<()> {
    writer.write_all(&value.to_le_bytes())?;
    Ok(())
}

pub(crate) fn pad_len(stored_length: u64) -> u64 {
    let rem = stored_length % 8;
    if rem == 0 { 0 } else { 8 - rem }
}
