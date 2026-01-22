//! Packets
//!
//! All compressed image data representing a specific tile, layer, component, resolution level and
//! precinct appears in the codestream in a contiguous segment called a packet.
//!
//! This means decoding image data takes place packet by packet.
//!
//! To decode a packet requires the context of what the packet is associated to, the specific tile,
//! layer, component, resolution level, and precinct.
//!
//! Each packet decoder is for a specific tile, component, resolution level, and precinct. The
//! packet decoder consumes packets, layer by layer.
use core::{error, fmt};
use log::{debug, info, warn};
use std::io::{self, Read};

use crate::code_block::CodeBlockDecodeError;
use crate::coder::standard_decoder;
use crate::shared::{Array2D, Bounds, SubBandGroup, SubBandType, I2};
use crate::tag_tree::{InclusionTagTree, ZeroPlaneTagTree};
use crate::{bit_reader::BitReader, code_block::CodeBlockDecoder};

/// contains information from the relevant header
#[derive(Debug, Default)]
struct HeaderInfo {
    _length: usize,
    packet_info: Vec<Vec<CodeBlockHeaderInformation>>,
}

#[derive(Debug)]
struct CodeBlockHeaderInformation {
    _index: I2,
    code_pass_count: u8,
    coded_bytes: u32,
}

/// Result wrapper for our errors
type PacketResult<T> = Result<T, PacketDecodeError>;

#[derive(Debug)]
struct SubBandBounds {
    bounds: Bounds,
    sub_band_type: SubBandType,
}

/// Bounds for a specific Tile Component Resolution
pub type TileComponentResolutionBounds = Bounds;

/// Calculations based on section B.5
impl TileComponentResolutionBounds {
    fn sub_bands_ll(&self) -> SubBandBounds {
        SubBandBounds {
            sub_band_type: SubBandType::LL,
            // for LL we use the whole resolution level, there is no decomposition
            bounds: Bounds {
                x0: self.x0,
                x1: self.x1,
                y0: self.y0,
                y1: self.y1,
            },
        }
    }
    fn sub_bands_hl(&self) -> SubBandBounds {
        SubBandBounds {
            sub_band_type: SubBandType::HL,
            bounds: Bounds {
                x0: self.x0 / 2,
                x1: self.x1 / 2,
                y0: self.y0.div_ceil(2),
                y1: self.y1.div_ceil(2),
            },
        }
    }
    fn sub_bands_lh(&self) -> SubBandBounds {
        SubBandBounds {
            sub_band_type: SubBandType::LH,
            bounds: Bounds {
                x0: self.x0.div_ceil(2),
                x1: self.x1.div_ceil(2),
                y0: self.y0 / 2,
                y1: self.y1 / 2,
            },
        }
    }
    fn sub_bands_hh(&self) -> SubBandBounds {
        SubBandBounds {
            sub_band_type: SubBandType::HH,
            bounds: Bounds {
                x0: self.x0 / 2,
                x1: self.x1 / 2,
                y0: self.y0 / 2,
                y1: self.y1 / 2,
            },
        }
    }
}

#[derive(Debug)]
pub enum PacketDecodeError {
    IO(io::Error),
    CodeBlock(CodeBlockDecodeError),
}

impl From<io::Error> for PacketDecodeError {
    fn from(value: io::Error) -> Self {
        PacketDecodeError::IO(value)
    }
}

impl From<CodeBlockDecodeError> for PacketDecodeError {
    fn from(value: CodeBlockDecodeError) -> Self {
        PacketDecodeError::CodeBlock(value)
    }
}

impl error::Error for PacketDecodeError {}
impl fmt::Display for PacketDecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PacketDecodeError::IO(error) => error.fmt(f),
            PacketDecodeError::CodeBlock(error) => error.fmt(f),
        }
    }
}

/// Holds the context for a precinct packet decoder
#[derive(Debug)]
struct DecoderContext {
    sub_bands: Vec<SubBandContext>,
    layer: u8,
}

/// Holds context for a specific sub-band in a precinct
#[derive(Debug)]
struct SubBandContext {
    inclusion_tree: InclusionTagTree,
    zeros_tree: ZeroPlaneTagTree,
    cbs: Vec<CodeBlockDecoder>,
}

impl SubBandContext {
    /// Determines the inclusion decision for the specific code block for the given layer by
    /// updating the tag tree with bits from the BitReader
    fn inclusion_decision<R: Read>(
        &mut self,
        dim_idx: I2,
        layer: u8,
        bit_reader: &mut BitReader<'_, R>,
    ) -> PacketResult<bool> {
        Ok(self
            .inclusion_tree
            .read_for_inclusion(dim_idx, layer as u32, bit_reader)?)
    }

    /// Determines the number of zero bit planes for a given code block based on tag tree possibly
    /// reading bits.
    fn zero_planes<R: Read>(
        &mut self,
        dim_idx: I2,
        bit_reader: &mut BitReader<'_, R>,
    ) -> PacketResult<u8> {
        Ok(self.zeros_tree.read(dim_idx, bit_reader)?)
    }

    /// Tests if a code block was included in a previous layer
    fn included(&self, dim_idx: I2, layer: u8) -> bool {
        self.inclusion_tree.query_inclusion(dim_idx, layer as u32)
    }
}

/// The PrecinctDecoder is responsible for decoding packet headers and packets for a single
/// precinct.
#[derive(Debug)]
pub struct PrecinctDecoder {
    ctx: DecoderContext,
    header: Option<HeaderInfo>,
    _bounds: TileComponentResolutionBounds,
}

impl PrecinctDecoder {
    pub fn new(
        cbx: u8,
        cby: u8,
        exponents: &[u8],
        bounds: TileComponentResolutionBounds,
        is_res_0: bool,
    ) -> PrecinctDecoder {
        let pcbx = 2u32.pow(cbx as u32);
        let pcby = 2u32.pow(cby as u32);
        let sbs = if is_res_0 {
            vec![bounds.sub_bands_ll()]
        } else {
            vec![
                bounds.sub_bands_hl(),
                bounds.sub_bands_lh(),
                bounds.sub_bands_hh(),
            ]
        };
        let mut sub_band_ctxs = Vec::new();
        for (sub_band_bounds, mb) in sbs.iter().zip(exponents) {
            let sb_type = sub_band_bounds.sub_band_type;
            let b = sub_band_bounds.bounds;

            let width = b.x1 - b.x0;
            let height = b.y1 - b.y0;

            let num_cb_wide = width.div_ceil(pcbx) as usize;
            let num_cb_tall = height.div_ceil(pcby) as usize;
            info!(
                "Createing packet decoder for sub_band {:?} with {}x{} codeblocks",
                sb_type, num_cb_wide, num_cb_tall
            );

            if num_cb_tall == 0 || num_cb_wide == 0 {
                continue; // skip this sub_band
            }
            if num_cb_tall > 1 || num_cb_wide > 1 {
                todo!("Not handling multiple codeblocks yet");
            }
            let mut cbs = Vec::new();
            for _ in 0..num_cb_tall {
                for _ in 0..num_cb_tall {
                    let cb_bounds = sub_band_bounds.bounds;
                    cbs.push(CodeBlockDecoder::new(
                        (cb_bounds.x1 - cb_bounds.x0) as i32,
                        (cb_bounds.y1 - cb_bounds.y0) as i32,
                        sb_type,
                        *mb,
                    ));
                }
            }
            let sb_ctx = SubBandContext {
                inclusion_tree: InclusionTagTree::new(num_cb_wide, num_cb_tall),
                zeros_tree: ZeroPlaneTagTree::new(num_cb_wide, num_cb_tall),
                cbs,
            };
            sub_band_ctxs.push(sb_ctx);
        }
        let sub_bands = sub_band_ctxs;

        PrecinctDecoder {
            ctx: DecoderContext {
                layer: 0,
                sub_bands,
            },
            header: None,
            _bounds: bounds,
        }
    }

    /// Grab sub band information for this precinct
    pub fn grab_precinct_subbands(&self) -> SubBandGroup<Array2D<i32>> {
        let mut ll = None;
        let mut hl = None;
        let mut lh = None;
        let mut hh = None;

        for sb in &self.ctx.sub_bands {
            if 1 != sb.cbs.len() {
                todo!("combining code blocks not implemented ");
            }
            for code_block in &sb.cbs {
                let sbt = code_block.sub_band();
                let _ = (match sbt {
                    SubBandType::LL => &mut ll,
                    SubBandType::HL => &mut hl,
                    SubBandType::LH => &mut lh,
                    SubBandType::HH => &mut hh,
                })
                .insert(code_block.coefficients());
            }
        }

        if let Some(ll) = ll {
            SubBandGroup::LL(ll)
        } else {
            SubBandGroup::Partial {
                hl: hl.unwrap_or_else(|| Array2D::new(0, 0)),
                lh: lh.unwrap_or_else(|| Array2D::new(0, 0)),
                hh: hh.unwrap_or_else(|| Array2D::new(0, 0)),
            }
        }
    }

    /// Consume a packet header pointed to by the reader
    pub fn consume_packet_header<R: Read>(mut self, reader: &mut R) -> PacketResult<Self> {
        let sub_bands = &mut self.ctx.sub_bands;
        let layer = self.ctx.layer;
        self.header = Some(Self::consume_header(sub_bands, layer, reader)?);
        Ok(self)
    }

    /// Consume and return packet header pointed to by the reader
    fn consume_header<R: Read>(
        sub_bands: &mut [SubBandContext],
        layer: u8,
        reader: &mut R,
    ) -> PacketResult<HeaderInfo> {
        // Packets are byte aligned, so we can parse at the byte boundary
        let mut bit_r = BitReader::new(reader)?;
        let zl_mark = bit_r.next_bit()?;
        if !zl_mark {
            // not sure if our handling works, warn in case
            warn!("Zero length packet");
            return Ok(HeaderInfo {
                _length: 0,
                ..Default::default()
            });
        }

        let mut total_to_read = 0;
        let mut packet_info = vec![];
        for sub_band_ctx in sub_bands.iter_mut() {
            let mut cb_info = vec![];
            // Walk code-blocks in sub band / precinct
            let code_blocks = vec![0];

            'for_code_blocks: for cb in code_blocks {
                let cb_idx = I2 { x: 0, y: 0 };
                let to_include: bool = if sub_band_ctx.included(cb_idx, layer) {
                    // If a code-block has been previously encoded, check 1 bit for inclusion/exclusion status
                    bit_r.next_bit()?
                } else {
                    // If a code-block inclusion level has not been encoded, update tag tree until we know
                    let decision = sub_band_ctx.inclusion_decision(cb_idx, layer, &mut bit_r)?;
                    if decision {
                        // initialize zero plane information
                        let zero_planes = sub_band_ctx.zero_planes(cb_idx, &mut bit_r)?;
                        debug!("Initializing code block with {zero_planes} zero planes");
                        sub_band_ctx.cbs[cb as usize].num_zero_bit_planes(zero_planes);
                    }
                    decision
                };
                // If not including this code block, not much to do
                if !to_include {
                    continue 'for_code_blocks;
                }

                // Ok code block included and zero planes initialized
                let code_pass_count = parse_coding_pass(&mut bit_r)?;
                let mut to_inc = 0;
                while bit_r.next_bit()? {
                    to_inc += 1;
                }
                // TODO sub_band_ctx.code_block(cb).increment_lblock(to_inc);
                let lblock = 3 + to_inc;
                let count_read = lblock + code_pass_count.ilog2() as u8;

                let coded_bytes = bit_r.take(count_read)?;
                cb_info.push(CodeBlockHeaderInformation {
                    _index: cb_idx,
                    code_pass_count,
                    coded_bytes,
                });
                total_to_read += coded_bytes as usize;
            }
            packet_info.push(cb_info);
        }
        Ok(HeaderInfo {
            _length: total_to_read,
            packet_info,
        })
    }

    /// Consume a packet pointed to by the reader. The previous call must be to
    /// consume_packet_header to prime the handlers.
    pub fn consume_packet<R: Read>(mut self, reader: &mut R) -> PacketResult<Self> {
        let Some(HeaderInfo { packet_info, .. }) = self.header.take() else {
            panic!("Invalid consume_packet call");
        };

        for (sb, header_info) in self.ctx.sub_bands.iter_mut().zip(packet_info) {
            if header_info.len() > 1 {
                todo!("Unable to handle multiple code blocks");
            }
            for CodeBlockHeaderInformation {
                _index,
                code_pass_count,
                coded_bytes,
            } in header_info
            {
                let mut buf = vec![0u8; coded_bytes as usize];
                reader.read_exact(&mut buf)?;
                let mut coder = standard_decoder(&buf);
                // TODO handle different mq coder style
                let cb = &mut sb.cbs[0];
                cb.decode(code_pass_count, &mut coder)?;
            }
        }
        Ok(self)
    }
}

fn parse_coding_pass<R: Read>(br: &mut BitReader<'_, R>) -> PacketResult<u8> {
    if !br.next_bit()? {
        // 0b0
        return Ok(1);
    }
    if !br.next_bit()? {
        // 0b10
        return Ok(2);
    }
    // 0b11 ?
    let r = br.take_u8(2)?;
    if r != 0b11 {
        // 0b 11 xx
        return Ok(3 + r);
    }
    // 0b 1111 ?
    let r = br.take_u8(5)?;
    if r != 0b11111 {
        return Ok(6 + r);
    }
    // 0b 1111 11111 ?
    let r = br.take_u8(7)?;
    Ok(37 + r)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::shared::Bounds;

    use super::*;

    type E = Box<dyn error::Error>;

    #[test]
    fn test_create() {
        let bounds = Bounds {
            x0: 0,
            x1: 32,
            y0: 0,
            y1: 32,
        };
        let _ = PrecinctDecoder::new(5, 5, &[9], bounds, true);
    }

    #[test]
    fn test_packet_decode_consume_01() -> Result<(), E> {
        let ba = b"\xC7\xD4\x0C\x01\x8f\x0D\xC8\x75\x5D\x00\x00\x00";
        let mut reader = Cursor::new(ba);

        // TODO this new method sucks
        let tcr = TileComponentResolutionBounds {
            x0: 0,
            x1: 1,
            y0: 0,
            y1: 5,
        };
        let decoder = PrecinctDecoder::new(5, 5, &[9], tcr, true);
        let decoder = decoder.consume_packet_header(&mut reader)?;
        assert_eq!(reader.position(), 3, "Header was 3 bytes");
        let decoder = decoder.consume_packet(&mut reader)?;
        assert_eq!(reader.position(), 9, "expected to consume 9 bytes");

        let sb = &decoder.ctx.sub_bands[0];
        let cb = &sb.cbs[0];
        let coeffs = cb.coefficients();
        let exp = Array2D::from_data(vec![-26, -22, -30, -32, -19], 1, 5);
        assert_eq!(coeffs, exp);

        Ok(())
    }

    #[test]
    fn test_packet_decode_consume_02() -> Result<(), E> {
        let ba = b"\xC0\x7C\x21\x80\x0F\xB1\x76";
        let mut reader = Cursor::new(ba);

        // TODO this new method sucks
        let tcr = TileComponentResolutionBounds {
            x0: 0,
            x1: 1,
            y0: 0,
            y1: 9,
        };
        let decoder = PrecinctDecoder::new(5, 5, &[10, 10, 10], tcr, false);
        let decoder = decoder.consume_packet_header(&mut reader)?;
        assert_eq!(reader.position(), 4, "Header was 4 bytes");
        let decoder = decoder.consume_packet(&mut reader)?;
        assert_eq!(reader.position(), 7, "expected to consume 7 bytes");

        let sb = decoder.ctx.sub_bands.last().expect("Expected to grab LH");
        let cb = &sb.cbs[0];
        let coeffs = cb.coefficients();
        assert_eq!(coeffs, Array2D::from_data(vec![1, 5, 1, 0], 1, 4));

        Ok(())
    }

    #[test]
    fn test_decode_zl_packet() -> Result<(), E> {
        let ba = b"\x00";
        let mut reader = Cursor::new(ba);

        let tcr = TileComponentResolutionBounds {
            x0: 0,
            x1: 1,
            y0: 0,
            y1: 9,
        };
        let decoder = PrecinctDecoder::new(5, 5, &[10, 10, 10], tcr, false);
        let decoder = decoder.consume_packet_header(&mut reader)?;
        let state = decoder.header;

        assert_eq!(
            0,
            state.expect("expected header")._length,
            "zero length packet header"
        );
        assert_eq!(reader.position(), 1, "expected to consume 1 bytes");
        Ok(())
    }

    /// from B.10 packet header
    #[test]
    fn test_parse_pass_count() {
        let vals: Vec<(u8, &[u8])> = vec![
            (1, b"\x00"),
            (2, b"\x80"),
            (4, b"\xD0"),
            (6, b"\xF0\x00"),
            (37, b"\xFF\x80"),
            (37 + 4, b"\xFF\x84"),
            (37 + 112, b"\xFF\xF0"),
        ];
        for (exp, bs) in vals {
            let mut cursor = Cursor::new(bs);
            let mut br = BitReader::new(&mut cursor).expect("unable to create reader");
            assert_eq!(exp, parse_coding_pass(&mut br).expect("didn't expect fail"));
        }
    }
}
