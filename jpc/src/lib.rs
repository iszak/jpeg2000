#![allow(dead_code)]

use log::info;
use std::error;
use std::fmt;
use std::io;
use std::str;

#[derive(Debug)]
enum CodestreamError {
    MarkerMissing { marker: MarkerSymbol, offset: u64 },
    MarkerUnexpected { marker: MarkerSymbol, offset: u64 },
}

impl error::Error for CodestreamError {}
impl fmt::Display for CodestreamError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::MarkerMissing { marker, offset } => {
                write!(f, "missing marker {:?} at offset {}", marker, offset)
            }
            Self::MarkerUnexpected { marker, offset } => {
                write!(f, "unexpected marker {:?} at offset {}", marker, offset)
            }
        }
    }
}

const COMPRESSION_TYPE_WAVELET: u8 = 7;

type MarkerSymbol = [u8; 2];

// Delimiting markers and marker segments
const MARKER_SYMBOL_SOC: MarkerSymbol = [255, 79]; // Start of code stream
const MARKER_SYMBOL_SOT: MarkerSymbol = [255, 144]; // Start of tile-part
const MARKER_SYMBOL_SOD: MarkerSymbol = [255, 147]; // Start of data
const MARKER_SYMBOL_EOC: MarkerSymbol = [255, 217]; // End of codestream

// Fixed information marker segments
const MARKER_SYMBOL_SIZ: MarkerSymbol = [255, 81]; // Image and tile size

// Functional marker segments
const MARKER_SYMBOL_COD: MarkerSymbol = [255, 82]; // Coding style default
const MARKER_SYMBOL_COC: MarkerSymbol = [255, 83]; // Coding style component
const MARKER_SYMBOL_RGN: MarkerSymbol = [255, 94]; // Region-of-interest
const MARKER_SYMBOL_QCD: MarkerSymbol = [255, 92]; // Quantization default
const MARKER_SYMBOL_QCC: MarkerSymbol = [255, 93]; // Quantization component
const MARKER_SYMBOL_POC: MarkerSymbol = [255, 95]; // Progression order change

// Pointer marker segments
const MARKER_SYMBOL_TLM: MarkerSymbol = [255, 85]; // Tile-part lengths
const MARKER_SYMBOL_PLM: MarkerSymbol = [255, 87]; // Packet length, main header
const MARKER_SYMBOL_PLT: MarkerSymbol = [255, 88]; // Packet length, tile-part header
const MARKER_SYMBOL_PPM: MarkerSymbol = [255, 96]; // Packed packet headers, main header
const MARKER_SYMBOL_PPT: MarkerSymbol = [255, 97]; // Packed packet headers, tile-part header

// In bit stream markers and marker segments
const MARKER_SYMBOL_SOP: MarkerSymbol = [255, 145]; // Start of packet
const MARKER_SYMBOL_EPH: MarkerSymbol = [255, 146]; // End of packet header

// Informational marker segments
const MARKER_SYMBOL_CRG: MarkerSymbol = [255, 99]; // Component registration
const MARKER_SYMBOL_COM: MarkerSymbol = [255, 100]; // Comment

#[derive(Debug)]
enum ProgressionOrder {
    // 0000 0000 Layer-resolution level-component-position progression
    LRLCPP,

    // 0000 0001 Resolution level-layer-component-position progression
    RLLCPP,

    // 0000 0010 Resolution level-position-component-layer progression
    RLPCLP,

    // 0000 0011 Position-component-resolution level-layer progression
    PCRLLP,

    // 0000 0100 Component-position-resolution level-layer progression
    CPRLLP,

    // All other values reserved
    Reserved,
}

#[derive(Debug)]
enum CodingBlockStyle {
    // xxxx xxx0 No selective arithmetic coding bypass
    NoSelectiveArithmeticCodingBypass,

    // xxxx xxx1 - Selective arithmetic coding bypass
    SelectiveArithmeticCodingBypass,

    // xxxx xx0x - No reset of context probabilities on coding pass boundaries
    NoResetOfContextProbabilities,

    // xxxx xx1x - Reset context probabilities on coding pass boundaries
    ResetContextProbabilities,

    // xxxx x0xx - No termination on each coding pass
    NoTerminationOnEachCodingPass,

    // xxxx x1xx - Termination on each coding pass
    TerminationOnEachCodingPass,

    // xxxx 0xxx - No vertically causal context
    NoVerticallyCausalContext,

    // xxxx 1xxx - Vertically causal context
    VerticallyCausalContext,

    // xxx0 xxxx - No predictable termination
    NoPredictableTermination,

    // xxx1 xxxx - Predictable termination
    PredictableTermination,

    // xx0x xxxx - No segmentation symbols are used
    NoSegmentationSymbolsAreUsed,

    // xx1x xxxx - Segmentation symbols are used
    SegmentationSymbolsAreUsed,

    // All other values reserved
    Reserved,
}

#[derive(Debug, PartialEq)]
enum CodingStyle {
    // xxxx xxx0 Entropy coder, precincts with PPx = 15 and PPy = 15
    EntropyCoderWithPrecincts,

    // xxxx xxx1 Entropy coder with precincts defined below
    EntropyCoderWithPrecinctsDefined,

    // xxxx xx0x No SOP marker segments used
    NoSOP,

    // xxxx xx1x SOP marker segments may be used
    SOP,

    // xxxx x0xx No EPH marker used
    NoEPH,

    // xxxx x1xx EPH marker may be used
    EPH,

    // All other values reserved
    Reserved,
}

impl CodingStyle {
    fn new(value: u8) -> Vec<CodingStyle> {
        let mut coding_styles: Vec<CodingStyle> = vec![];

        if value & 0b_0000_0001 > 0 {
            coding_styles.push(CodingStyle::EntropyCoderWithPrecincts);
        } else {
            coding_styles.push(CodingStyle::EntropyCoderWithPrecinctsDefined);
        }

        if value & 0b_0000_0010 > 0 {
            coding_styles.push(CodingStyle::SOP);
        } else {
            coding_styles.push(CodingStyle::NoSOP);
        }

        if value & 0b_0000_0100 > 0 {
            coding_styles.push(CodingStyle::EPH);
        } else {
            coding_styles.push(CodingStyle::NoEPH);
        }

        coding_styles
    }
}

#[derive(Debug)]
enum MultipleComponentTransformation {
    // No multiple component transformation specified.
    None,

    // Component transformation used on components 0, 1, 2 for coding efficiency.
    // Irreversible component transformation used with the 9-7 irreversible filter.
    // Reversible component transformation used with the 5-3 reversible filter.
    Multiple,

    // All other values reserved
    Reserved,
}

const TRANSFORMATION_FILTER_IRREVERSIBLE: [u8; 1] = [0];
const TRANSFORMATION_FILTER_REVERSIBLE: [u8; 1] = [1];

#[derive(Debug)]
enum TransformationFilter {
    // 9-7 irreversible filter
    Irreversible,
    // 5-3 reversible filter
    Reversible,

    // All other values reserved
    Reserved { value: [u8; 1] },
}

impl TransformationFilter {
    fn new(value: [u8; 1]) -> TransformationFilter {
        match value {
            TRANSFORMATION_FILTER_IRREVERSIBLE => TransformationFilter::Irreversible,
            TRANSFORMATION_FILTER_REVERSIBLE => TransformationFilter::Reversible,
            _ => TransformationFilter::Reserved { value },
        }
    }
}

// Coding style default (COD)
//
// Function: Describes the coding style, number of decomposition levels,
// and layering that is the default used forcompressing all components of
// an image (if in the main header) or a tile (if in the tile-part header).
//
// The parameter values can be overridden for an individual component by a
// COC marker segment in either the main or tile-part header.
#[derive(Debug, Default)]
pub struct CodingStyleMarkerSegment {
    coding_style: [u8; 1],

    // Progression order
    progression_order: [u8; 1],

    // Number of layers
    no_layers: [u8; 2],

    // Multiple component transformation
    multiple_component_transformation: [u8; 1],

    // Number of decomposition levels, NL, Zero implies no transformation.
    no_decomposition_levels: [u8; 1],

    // Code-block width exponent offset value, xcb
    code_block_width: [u8; 1],

    // Code-block height exponent offset value, ycb
    code_block_height: [u8; 1],

    // Style of the code-block coding passes
    code_block_style: [u8; 1],

    // Wavelet transformation used.
    transformation: [u8; 1],
}

impl CodingStyleMarkerSegment {
    fn coding_styles(&self) -> Vec<CodingStyle> {
        CodingStyle::new(self.coding_style[0])
    }

    fn no_layers(&self) -> u16 {
        u16::from_be_bytes(self.no_layers)
    }

    fn progression_order(&self) -> ProgressionOrder {
        todo!();
    }

    fn multiple_component_transformation(&self) -> MultipleComponentTransformation {
        todo!();
    }

    fn coding_block_styles(&self) -> Vec<CodingBlockStyle> {
        let mut coding_block_styles: Vec<CodingBlockStyle> = vec![];

        if self.coding_style[0] & 0b_0000_0001 == 1 {
            coding_block_styles.push(CodingBlockStyle::SelectiveArithmeticCodingBypass);
        } else {
            coding_block_styles.push(CodingBlockStyle::NoSelectiveArithmeticCodingBypass);
        }

        if self.coding_style[0] & 0b_0000_0010 == 1 {
            coding_block_styles.push(CodingBlockStyle::ResetContextProbabilities);
        } else {
            coding_block_styles.push(CodingBlockStyle::NoResetOfContextProbabilities);
        }

        if self.coding_style[0] & 0b_0000_0100 == 1 {
            coding_block_styles.push(CodingBlockStyle::TerminationOnEachCodingPass);
        } else {
            coding_block_styles.push(CodingBlockStyle::NoTerminationOnEachCodingPass);
        }

        if self.coding_style[0] & 0b_0000_1000 == 1 {
            coding_block_styles.push(CodingBlockStyle::VerticallyCausalContext);
        } else {
            coding_block_styles.push(CodingBlockStyle::NoVerticallyCausalContext);
        }

        if self.coding_style[0] & 0b_0001_0000 == 1 {
            coding_block_styles.push(CodingBlockStyle::PredictableTermination);
        } else {
            coding_block_styles.push(CodingBlockStyle::NoPredictableTermination);
        }

        if self.coding_style[0] & 0b_0010_0000 == 1 {
            coding_block_styles.push(CodingBlockStyle::SegmentationSymbolsAreUsed);
        } else {
            coding_block_styles.push(CodingBlockStyle::NoSegmentationSymbolsAreUsed);
        }

        coding_block_styles
    }

    fn transformation(&self) -> TransformationFilter {
        TransformationFilter::new(self.transformation)
    }
}

// Image and tile size (SIZ)
//
// Function: Provides information about the uncompressed image such as the
// width and height of the reference grid, the width and height of the tiles,
// the number of components, component bit depth, and the separation of
// component samples with respect to the reference grid.
#[derive(Debug, Default)]
pub struct ImageAndTileSizeMarkerSegment {
    // Denotes capabilities that a decoder needs to properly decode the codestream.
    decoder_capabilities: [u8; 2],

    // XSiz: Width of the reference grid.
    reference_grid_width: [u8; 4],

    // YSiz: Height of the reference grid.
    reference_grid_height: [u8; 4],

    // XOsiz: Horizontal offset from the origin of the reference grid to the top
    // side of the image area.
    image_horizontal_offset: [u8; 4],

    // YOsiz: Vertical offset from the origin of the reference grid to the top
    // side of the image area.
    image_vertical_offset: [u8; 4],

    // XTsiz: Width of one reference tile with respect to the reference grid
    reference_tile_width: [u8; 4],

    // YTsiz: Height of one reference tile with respect to the reference grid.
    reference_tile_height: [u8; 4],

    // XTOsiz: Horizontal offset from the origin of the reference grid to the left side of the
    // first tile.
    tile_horizontal_offset: [u8; 4],

    // YTOsiz: Vertical offset from the origin of the reference grid to the
    // top side of the first tile.
    tile_vertical_offset: [u8; 4],

    // Csiz: Number of components in the image.
    no_components: [u8; 2],

    // Ssiz: Precision (depth) in bits and sign of the ith component samples.
    //
    // The precision is the precision of the component samples before DC
    // level shifting is performed (i.e., the precision of the original
    // component samples before any processing is performed).
    //
    // If the component sample values are signed, then the range of
    // component sample values is -2^(Ssiz AND 0x7F)-1 ≤ component sample value ≤ 2^(Ssiz AND 0x7F)-1 - 1.
    //
    // There is one occurrence of this parameter for each component.
    // The order corresponds to thecomponent’s index, starting with zero.
    precision: Vec<[u8; 1]>,

    // XRsiz: Horizontal separation of a sample of ith component
    // with respect to the reference grid.
    //
    // There is one occurrence of this parameter for each component.
    horizontal_separation: Vec<[u8; 1]>,

    // YRsiz: Vertical separation of a sample of ith component
    // with respect to the reference grid.
    //
    // There is one occurrence of this parameter for each component.
    vertical_separation: Vec<[u8; 1]>,
}

impl ImageAndTileSizeMarkerSegment {
    fn no_components(&self) -> u16 {
        u16::from_be_bytes(self.no_components)
    }
}

#[derive(Debug)]
enum CommentRegistrationValue {
    // General use (binary values)
    Binary,

    // General use (IS 8859-15:1999 (Latin) values)
    Latin,

    // All other values reserved
    Reserved,
}

// Comment (COM)
//
// Allows unstructured data in the main and tile-part header.
#[derive(Debug, Default)]
pub struct CommentMarkerSegment {
    // RCom: Registration value of the marker segment
    registration_value: [u8; 2],

    // Ccomi: Byte of unstructured data
    comment: Vec<u8>,
}

impl CommentMarkerSegment {
    fn registration_value(&self) -> CommentRegistrationValue {
        match i16::from_be_bytes(self.registration_value) {
            1 => CommentRegistrationValue::Binary,
            2 => CommentRegistrationValue::Latin,
            _ => CommentRegistrationValue::Reserved,
        }
    }

    fn comment_utf8(&self) -> Result<&str, str::Utf8Error> {
        str::from_utf8(&self.comment)
    }
}

#[derive(Debug)]
pub enum QuantizationStyle {
    No,
    ScalarDerived,
    ScalarExpounded,
    Reserved { value: u8 },
}

impl QuantizationStyle {
    fn new(byte: u8) -> QuantizationStyle {
        let value = byte << 3 >> 3;

        match value {
            // xxx0 0000
            0b0000_0000 => QuantizationStyle::No,
            // xxx0 0001
            0b0000_0001 => QuantizationStyle::ScalarDerived,
            // xxx0 0010
            0b0000_0010 => QuantizationStyle::ScalarExpounded,
            _ => QuantizationStyle::Reserved { value },
        }
    }
}

// Quantization default (QCD)
//
// Function: Describes the quantization default used for compressing all
// components not defined by a QCC marker segment. The parameter values can be
// overridden for an individual component by a QCC marker segment in either the
// main or tile-part header.
#[derive(Debug, Default)]
pub struct QuantizationDefaultMarkerSegment {
    // Length of marker segment in bytes (not including the marker).
    length: [u8; 2],

    // Sqcd: Quantization style for all components
    style: [u8; 1],

    // SPqcd^i: Quantization step size value for the ith subband in the defined
    // order. The number of parameters is the same as the number of sub bands in
    // the tile-component with the greatest number of decomposition levels.
    step_size_values: Vec<u8>,
}

impl QuantizationDefaultMarkerSegment {
    // no_quantization               = 4 + 3 · number_decomposition_levels
    // scalar_quantization_derived   = 5
    // scalar_quantization_expounded = 5 + 6 · scalar_quantization_expounded
    //
    // where number_decomposition_levels is defined in the COD and COC marker
    // segments, and no_quantization, scalar_quantization_derived, or
    // scalar_quantization_expounded is signalled in the Sqcd parameter.
    pub fn length(&self) -> u16 {
        todo!();
    }

    pub fn style(&self) -> QuantizationStyle {
        QuantizationStyle::new(self.style[0])
    }

    pub fn step_size_values(&self) {
        todo!();
    }

    pub fn no_guard_bits(&self) -> u8 {
        self.style[0] >> 5
    }
}

// Contiguous Codestream box
//
// The Contiguous Codestream box contains a valid and complete JPEG 2000
// codestream. When displaying the image, a conforming reader shall ignore all
// codestreams after the first codestream found in the file.
//
// Contiguous Codestream boxes may be found anywhere in the file
// except before the JP2 Header box.
#[derive(Debug, Default)]
pub struct ContiguousCodestream {
    length: u64,
    offset: u64,
    header: Header,
    tiles: Vec<Tile>,
}

impl ContiguousCodestream {
    pub fn header(&self) -> &Header {
        &self.header
    }

    fn decode_siz<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<ImageAndTileSizeMarkerSegment, Box<dyn error::Error>> {
        let mut segment = ImageAndTileSizeMarkerSegment::default();

        let mut marker_segment_length: [u8; 2] = [0; 2];
        reader.read_exact(&mut marker_segment_length)?;

        reader.read_exact(&mut segment.decoder_capabilities)?;
        reader.read_exact(&mut segment.reference_grid_width)?;
        reader.read_exact(&mut segment.reference_grid_height)?;
        reader.read_exact(&mut segment.image_horizontal_offset)?;
        reader.read_exact(&mut segment.image_vertical_offset)?;
        reader.read_exact(&mut segment.reference_tile_width)?;
        reader.read_exact(&mut segment.reference_tile_height)?;
        reader.read_exact(&mut segment.tile_horizontal_offset)?;
        reader.read_exact(&mut segment.tile_vertical_offset)?;
        reader.read_exact(&mut segment.no_components)?;

        let no_components = segment.no_components();

        let mut i = 0;
        loop {
            let mut precision: [u8; 1] = [0; 1];
            reader.read_exact(&mut precision)?;
            segment.precision.push(precision);

            let mut horizontal_separation: [u8; 1] = [0; 1];
            reader.read_exact(&mut horizontal_separation)?;
            segment.horizontal_separation.push(horizontal_separation);

            let mut vertical_separation: [u8; 1] = [0; 1];
            reader.read_exact(&mut vertical_separation)?;
            segment.vertical_separation.push(vertical_separation);

            i += 1;
            if i == no_components {
                break;
            }
        }

        Ok(segment)
    }

    fn decode_cod<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<CodingStyleMarkerSegment, Box<dyn error::Error>> {
        let mut segment = CodingStyleMarkerSegment::default();

        // Length of marker segment in bytes (not including the marker).
        let mut marker_segment_length: [u8; 2] = [0; 2];
        reader.read_exact(&mut marker_segment_length)?;

        reader.read_exact(&mut segment.coding_style)?;
        reader.read_exact(&mut segment.progression_order)?;
        reader.read_exact(&mut segment.no_layers)?;
        reader.read_exact(&mut segment.multiple_component_transformation)?;
        reader.read_exact(&mut segment.no_decomposition_levels)?;
        reader.read_exact(&mut segment.code_block_width)?;
        reader.read_exact(&mut segment.code_block_height)?;
        reader.read_exact(&mut segment.code_block_style)?;
        reader.read_exact(&mut segment.transformation)?;

        // If Scod or Scoc = xxxx xxx0, this parameter is not present, otherwise this indicates
        // precinct width and height.
        //
        // The first parameter (8 bits) corresponds to the NLLL sub band.
        // Each successive parameter corresponds to each successive resolution level in order
        if segment.coding_style[0] & 1 == 1 {
            todo!();
        }

        Ok(segment)
    }

    fn decode_qcd<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<QuantizationDefaultMarkerSegment, Box<dyn error::Error>> {
        let mut segment = QuantizationDefaultMarkerSegment::default();

        reader.read_exact(&mut segment.length)?;
        reader.read_exact(&mut segment.style)?;

        // Skip
        let step_size_length = u16::from_be_bytes(segment.length)
            - segment.length.len() as u16
            - segment.style.len() as u16;

        let mut step_size_values: Vec<u8> = Vec::with_capacity(step_size_length as usize);

        let mut step_size_value: [u8; 1] = [0; 1];

        let mut index = 0;
        while index < step_size_length {
            reader.read_exact(&mut step_size_value)?;
            step_size_values.push(step_size_value[0]);
            index = index + 1;
        }
        segment.step_size_values = step_size_values;

        Ok(segment)
    }

    fn decode_com<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<CommentMarkerSegment, Box<dyn error::Error>> {
        let mut segment = CommentMarkerSegment::default();

        // Length of marker segment in bytes (not including the marker).
        let mut marker_segment_length: [u8; 2] = [0; 2];
        reader.read_exact(&mut marker_segment_length)?;
        reader.read_exact(&mut segment.registration_value)?;

        let comment_length = u16::from_be_bytes(marker_segment_length) as usize
            - marker_segment_length.len()
            - segment.registration_value.len();

        segment.comment = vec![0; comment_length];

        reader.read_exact(&mut segment.comment)?;

        Ok(segment)
    }
}

#[derive(Debug, Default)]
pub struct Header {
    // SIZ (Required)
    image_and_tile_size_marker_segment: ImageAndTileSizeMarkerSegment,

    // COD (Required)
    coding_style_marker_segment: CodingStyleMarkerSegment,

    // QCD (Required)
    quantization_default_marker_segment: QuantizationDefaultMarkerSegment,

    // COM (Optional)
    comment_marker_segment: Option<CommentMarkerSegment>,
}
impl Header {
    pub fn image_and_tile_size_marker_segment(&self) -> &ImageAndTileSizeMarkerSegment {
        &self.image_and_tile_size_marker_segment
    }
    pub fn coding_style_marker_segment(&self) -> &CodingStyleMarkerSegment {
        &self.coding_style_marker_segment
    }
    pub fn quantization_default_marker_segment(&self) -> &QuantizationDefaultMarkerSegment {
        &self.quantization_default_marker_segment
    }
    pub fn comment_marker_segment(&self) -> &Option<CommentMarkerSegment> {
        &self.comment_marker_segment
    }
}

#[derive(Debug, Default)]
struct Tile {
    header: TileHeader,
    parts: Vec<u8>,
}

#[derive(Debug, Default)]
struct TileHeader {
    // Isot: Tile index.
    //
    // This number refers to the tiles in raster order starting at the number 0
    tile_index: [u8; 2],

    // Psot: Length, in bytes, from the beginning of the first byte of this SOT
    // marker segment of the tile-part to the end of the data of that tile-part.
    //
    // Only the last tile-part in the codestream may contain a 0 for Psot.
    //
    // If the Psot is 0, this tile-part is assumed to contain all data until the
    // EOC marker.
    tile_length: [u8; 2],

    // TPsot: Tile-part index.
    //
    // There is a specific order required for decoding tile-parts; this index
    // denotes the order from 0.
    //
    // If there is only one tile-part for a tile then this value is zero.
    //
    // The tile-parts of this tile shall appear in the codestream in this order,
    // although not necessarily consecutively.
    tile_part_index: [u8; 2],

    // TNsot: Number of tile-parts of a tile in the codestream.
    //
    // Two values are allowed: the correct number of tile-parts for that tile
    // and zero. A zero value indicates that the number of tile-parts of this
    // tile is not specified inthis tile-part.
    no_tile_parts: [u8; 2],
}

impl ContiguousCodestream {
    fn length(&self) -> u64 {
        self.length
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn decode_main_header<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<Header, Box<dyn error::Error>> {
        let mut header = Header::default();

        let mut marker_type: MarkerSymbol = [0; 2];
        reader.read_exact(&mut marker_type)?;

        // SOC (Required as the first marker)
        if marker_type != MARKER_SYMBOL_SOC {
            return Err(CodestreamError::MarkerMissing {
                marker: MARKER_SYMBOL_SOC,
                offset: reader.stream_position()?,
            }
            .into());
        }

        // SIZ (Required as the second marker segment)
        reader.read_exact(&mut marker_type)?;
        if marker_type != MARKER_SYMBOL_SIZ {
            return Err(CodestreamError::MarkerMissing {
                marker: MARKER_SYMBOL_SOT,
                offset: reader.stream_position()?,
            }
            .into());
        }

        info!("SIZ Start {:?}", reader.stream_position());
        header.image_and_tile_size_marker_segment = self.decode_siz(reader)?;
        info!("SIZ Finish {:?}", reader.stream_position());

        // COD (Required)
        reader.read_exact(&mut marker_type)?;
        if marker_type != MARKER_SYMBOL_COD {
            return Err(CodestreamError::MarkerMissing {
                marker: MARKER_SYMBOL_COD,
                offset: reader.stream_position()?,
            }
            .into());
        }
        info!("COD Start {:?}", reader.stream_position());
        header.coding_style_marker_segment = self.decode_cod(reader)?;
        info!("COD Finish {:?}", reader.stream_position());

        // COC (Optional, no more than one COC per component)
        reader.read_exact(&mut marker_type)?;
        if marker_type == MARKER_SYMBOL_COC {
            todo!();
        }

        // QCD (Required)
        if marker_type != MARKER_SYMBOL_QCD {
            return Err(CodestreamError::MarkerMissing {
                marker: MARKER_SYMBOL_QCD,
                offset: reader.stream_position()?,
            }
            .into());
        }

        info!("QCD Start {:?}", reader.stream_position());
        header.quantization_default_marker_segment = self.decode_qcd(reader)?;
        info!("QCD Finish {:?}", reader.stream_position());

        reader.read_exact(&mut marker_type)?;
        // QCC (Optional, no more than one QCC per component)
        if marker_type == MARKER_SYMBOL_QCC {
            marker_type.fill(0);
            todo!();
        }

        // RGN (Optional, no more than one RGN per component)
        if marker_type == [0; 2] {
            reader.read_exact(&mut marker_type)?;
        }
        if marker_type == MARKER_SYMBOL_RGN {
            marker_type.fill(0);
            todo!();
        }

        // POC (Required in main or tile for any progression order changes)
        if marker_type == [0; 2] {
            reader.read_exact(&mut marker_type)?;
        }
        if marker_type == MARKER_SYMBOL_POC {
            marker_type.fill(0);
            todo!();
        }

        // PPM (Optional, either PPM or PPT or codestream packet headers required)
        if marker_type == [0; 2] {
            reader.read_exact(&mut marker_type)?;
        }
        if marker_type == MARKER_SYMBOL_PPM {
            marker_type.fill(0);
            todo!();
        }

        if marker_type == [0; 2] {
            reader.read_exact(&mut marker_type)?;
        }
        if marker_type == MARKER_SYMBOL_PPT {
            marker_type.fill(0);
            todo!();
        }

        // TLM (Optional)
        if marker_type == [0; 2] {
            reader.read_exact(&mut marker_type)?;
        }
        if marker_type == MARKER_SYMBOL_TLM {
            marker_type.fill(0);
            todo!();
        }

        // PLM (Optional)
        if marker_type == [0; 2] {
            reader.read_exact(&mut marker_type)?;
        }
        if marker_type == MARKER_SYMBOL_PLM {
            marker_type.fill(0);
            todo!();
        }

        // CRG (Optional)
        if marker_type == [0; 2] {
            reader.read_exact(&mut marker_type)?;
        }
        if marker_type == MARKER_SYMBOL_CRG {
            marker_type.fill(0);
            todo!();
        }

        // COM (Optional)
        if marker_type == [0; 2] {
            reader.read_exact(&mut marker_type)?;
        }
        if marker_type == MARKER_SYMBOL_COM {
            marker_type.fill(0);
            info!("COM Start {:?}", reader.stream_position());
            header.comment_marker_segment = Some(self.decode_com(reader)?);
            info!("COM Finish {:?}", reader.stream_position());
        }

        // No optional markers
        if marker_type != [0; 2] {
            reader.seek(io::SeekFrom::Current(-2))?;
        }

        Ok(header)
    }

    fn decode_tile_header<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<TileHeader, Box<dyn error::Error>> {
        let mut tile_header = TileHeader::default();

        let mut marker_type: MarkerSymbol = [0; 2];

        reader.read_exact(&mut marker_type)?;

        // SOC (Required as the first marker segment of every tile-part header.)
        if marker_type != MARKER_SYMBOL_SOT {
            return Err(CodestreamError::MarkerMissing {
                marker: MARKER_SYMBOL_SOT,
                offset: reader.stream_position()?,
            }
            .into());
        }

        info!("SOT Start {:?}", reader.stream_position());

        // LSot
        let mut marker_segment_length: [u8; 2] = [0; 2];
        reader.read_exact(&mut marker_segment_length)?;

        // ISot
        reader.read_exact(&mut tile_header.tile_index)?;

        // PSot
        reader.read_exact(&mut tile_header.tile_length)?;

        // TPSot
        reader.read_exact(&mut tile_header.tile_part_index)?;

        // TNSot
        reader.read_exact(&mut tile_header.no_tile_parts)?;

        info!("SOT Finish {:?}", reader.stream_position());

        // COD (Optional)
        reader.read_exact(&mut marker_type)?;
        if marker_type == MARKER_SYMBOL_COD {
            todo!()
        }

        // COC (Optional)
        if marker_type == MARKER_SYMBOL_COC {
            todo!()
        }

        // QCD (Optional)
        if marker_type == MARKER_SYMBOL_QCD {
            todo!()
        }

        // QCC (Optional)
        if marker_type == MARKER_SYMBOL_QCC {
            todo!()
        }

        // RGN (Optional)
        if marker_type == MARKER_SYMBOL_RGN {
            todo!()
        }

        // POC (Optional)
        if marker_type == MARKER_SYMBOL_POC {
            todo!()
        }

        // PPT (Optional)
        if marker_type == MARKER_SYMBOL_PPT {
            todo!()
        }

        // PLT (Optional)
        if marker_type == MARKER_SYMBOL_PLT {
            todo!()
        }

        // COM (Optional)
        if marker_type == MARKER_SYMBOL_COM {
            todo!()
        }

        reader.seek(io::SeekFrom::Current(-2))?;

        Ok(tile_header)
    }

    fn decode<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<(), Box<dyn error::Error>> {
        // The main header is found at the beginning of the codestream
        self.header = self.decode_main_header(reader)?;

        // The tile-part headers are found at the beginning of each tile-part
        let tile_header = self.decode_tile_header(reader)?;
        let mut marker_type: MarkerSymbol = [0; 2];

        reader.read_exact(&mut marker_type)?;
        if marker_type != MARKER_SYMBOL_SOD {
            return Err(CodestreamError::MarkerMissing {
                marker: MARKER_SYMBOL_SOD,
                offset: reader.stream_position()?,
            }
            .into());
        }

        let coding_styles = self.header.coding_style_marker_segment.coding_styles();

        info!("SOD Start {:?}", reader.stream_position());

        let mut codestream: Vec<u8> = vec![];
        loop {
            match reader.read_exact(&mut marker_type) {
                Ok(_) => match marker_type {
                    MARKER_SYMBOL_EPH => {
                        if coding_styles.contains(&CodingStyle::NoEPH) {
                            return Err(CodestreamError::MarkerUnexpected {
                                marker: MARKER_SYMBOL_EPH,
                                offset: reader.stream_position()?,
                            }
                            .into());
                        }

                        todo!();
                    }
                    MARKER_SYMBOL_SOP => {
                        if coding_styles.contains(&CodingStyle::NoSOP) {
                            return Err(CodestreamError::MarkerUnexpected {
                                marker: MARKER_SYMBOL_SOP,
                                offset: reader.stream_position()?,
                            }
                            .into());
                        }
                        todo!();
                    }
                    MARKER_SYMBOL_EOC => {
                        break;
                    }
                    MARKER_SYMBOL_SOT => {
                        todo!();
                    }
                    _ => {
                        codestream.extend_from_slice(&marker_type);
                    }
                },

                Err(e) => match e.kind() {
                    io::ErrorKind::UnexpectedEof => break,
                    _ => return Err(e.into()),
                },
            }
        }
        info!("SOD Finish {:?}", reader.stream_position(),);
        self.tiles.push(Tile {
            header: tile_header,
            parts: codestream,
        });

        Ok(())
    }
}

#[derive(Debug)]
pub struct J2C {}

pub fn decode_j2c<R: io::Read + io::Seek>(reader: &mut R) -> Result<J2C, Box<dyn error::Error>> {
    let result = J2C {};

    let mut continuous_codestream_box = ContiguousCodestream::default();
    continuous_codestream_box.decode(reader)?;

    // Tile: A rectangular array of points on the reference grid, registered
    // with and offset from the reference grid origin and defined by a width and
    // height. The tiles which overlap are used to define tile-components.
    //
    // Tile-component: All the samples of a given component in a tile
    //
    // Component: A two-dimensional array of samples. A image typically consists
    // of several components, forinstance representing red, green, and blue.
    //
    // Sample: One element in the two-dimensional array that comprises a
    // component

    // Layer: A collection of compressed image data from coding passes of one,
    // or more, code-blocks of a tile-component.
    //
    // Layers have an order for encoding and decoding that must be preserved.
    //
    //
    // Coding pass: A complete pass through a code-block where the appropriate
    // coefficient values and context are applied.
    //
    // There are three types of coding passes:
    // - significance propagation pass
    // - magnitude refinement pass
    // - and cleanup pass.
    //
    // The result of each pass (after arithmetic coding, if selective arithmetic
    // coding bypass is not used) is a stream of compressed image data.
    //
    //
    // Code-block: A rectangular grouping of coefficients from the same subband
    // of a tile-component.
    //
    //
    // Subband: A group of transform coefficients resulting from the same
    // sequence of low-pass and high-pass filtering operations, both vertically
    // and horizontally.
    //
    //
    // Decomposition level: A collection of wavelet subbands where each
    // coefficient has the same spatial impact or span with respect to the source
    // component samples.
    //
    // These include the HL, LH, and HH subbands of the same two dimensional
    // subband decomposition.
    // For the last decomposition level the LL subband is also included.0

    Ok(result)
}
