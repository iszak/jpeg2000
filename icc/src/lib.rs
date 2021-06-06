#![allow(dead_code)]

use log::warn;
use std::error;
use std::io;

// Each tag signature in the tag table must be unique;
// a profile cannot contain more than one tag with the same signature.
struct Tag {
    signature: [u8; 4],
    offset: [u8; 4], // uInt32Number
    size: [u8; 4],   // uInt32Number
}

impl Tag {
    // A four byte value registered with the ICC
    fn signature(&self) -> [u8; 4] {
        self.signature
    }

    // An address within an ICC profile, relative to byte zero of the file.
    fn offset(&self) -> u32 {
        u32::from_be_bytes(self.offset)
    }

    // The number of bytes in the tag data element.
    fn size(&self) -> u32 {
        u32::from_be_bytes(self.size)
    }
}

#[derive(Debug)]
pub struct ICCProfile {}

pub fn decode_icc<R: io::Read + io::Seek>(
    reader: &mut R,
) -> Result<ICCProfile, Box<dyn error::Error>> {
    let icc_start_position = reader.stream_position()?;

    let mut header: [u8; 128] = [0; 128];
    reader.read_exact(&mut header)?;

    let mut tag_count: [u8; 4] = [0; 4];
    reader.read_exact(&mut tag_count)?;

    let tag_table_size = u32::from_be_bytes(tag_count) as usize;

    let mut tag_table: Vec<Tag> = Vec::with_capacity(tag_table_size as usize);

    let mut largest_offset: u32 = 0;
    let mut largest_size: u32 = 0;
    loop {
        let mut tag = Tag {
            signature: [0; 4],
            offset: [0; 4],
            size: [0; 4],
        };
        reader.read_exact(&mut tag.signature)?;
        reader.read_exact(&mut tag.offset)?;
        reader.read_exact(&mut tag.size)?;

        if tag.offset() > largest_offset {
            largest_offset = tag.offset();
            largest_size = tag.size();
        }

        tag_table.push(tag);

        if tag_table.len() == 6 {
            break;
        }
    }

    reader.seek(io::SeekFrom::Start(
        icc_start_position + largest_offset as u64 + largest_size as u64,
    ))?;

    let mut unknown: [u8; 4] = [0; 4];
    reader.read_exact(&mut unknown)?;
    warn!("unknown bytes 3 {:?}", unknown);

    Ok(ICCProfile {})
}
