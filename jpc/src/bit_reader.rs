use std::{
    fmt,
    io::{self, Read},
};

pub struct BitReader<'a, R: Read> {
    reader: &'a mut R,
    last_byte: [u8; 1], // last byte read from reader
    offset: u8,         // bit offset into last_byte, msb to lsb
    bits_read: u32,
}

impl<R: Read> fmt::Debug for BitReader<'_, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BitReader")
            .field("last_byte", &format_args!("{:x?}", &self.last_byte))
            .field("offset", &self.offset)
            .finish()
    }
}

impl<'a, R: Read> BitReader<'a, R> {
    pub fn new<'b: 'a>(reader: &'b mut R) -> Result<BitReader<'a, R>, io::Error> {
        let mut buf = [0; 1];
        reader.read_exact(&mut buf)?;
        Ok(Self {
            reader,
            last_byte: buf,
            offset: 0,
            bits_read: 0,
        })
    }

    pub fn next_bit(&mut self) -> Result<bool, io::Error> {
        self.bits_read += 1;
        if self.offset == 8 {
            self.reader.read_exact(&mut self.last_byte)?;
            self.offset = 0;
        }
        assert!(self.offset < 8);
        let o = 7 - self.offset;
        self.offset += 1;
        Ok((self.last_byte[0] >> o) & 0x01 == 1)
    }

    pub fn take(&mut self, arg: u8) -> Result<u8, io::Error> {
        let mut out = 0;
        for _ in 0..arg {
            out *= 2;
            out += self.next_bit()? as u8;
        }
        Ok(out)
    }

    pub fn bits_read(&self) -> u32 {
        self.bits_read
    }
}
