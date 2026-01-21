#![allow(dead_code)]

use log::{error, info};
use std::cmp;
use std::convert::TryInto;
use std::error;
use std::fmt;
use std::io;
use std::str;

pub mod code_block;
mod coder;
mod shared;
mod tag_tree;

#[derive(Debug)]
pub enum CodestreamError {
    /// Marker generic error
    MarkerError {
        marker: String,
        error: String,
    },
    /// Marker is unknown, potentially due to lack of support, malformed file, or parsing bug
    MarkerUnknown {
        marker: String,
        offset: u64,
    },
    /// Marker is expected but missing
    MarkerMissing {
        marker: String,
    },
    /// Marker is known but another marker is expected
    MarkerUnexpected {
        actual_marker: String,
        expected_marker: String,
        offset: u64,
    },
    /// Marker is known but disallowed potentially due to previous marker values
    MarkerDisallowed {
        marker: String,
        offset: u64,
    },
    /// Marker is known and expected but is malformed
    MarkerMalformed {
        marker: String,
        offset: u64,
    },
    TileSizeOverflow {
        image_horizontal_offset: u32,
        image_vertical_offset: u32,
        tile_horizontal_offset: u32,
        tile_vertical_offset: u32,
        reference_tile_width: u32,
        reference_tile_height: u32,
    },
    TileGridOffsetOverflow {
        tile_horizontal_offset: u32,
        tile_vertical_offset: u32,
        image_horizontal_offset: u32,
        image_vertical_offset: u32,
    },
    /// Marker is known but feature is unsupported
    UnsupportedFeature {
        marker: String,
        offset: u64,
    },
    InputFormatError {
        error: String,
    },
}

impl error::Error for CodestreamError {}
impl fmt::Display for CodestreamError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::MarkerError { marker, error } => {
                write!(f, "marker {marker} error {error:?}",)
            }
            Self::MarkerMissing { marker } => {
                write!(f, "missing marker {marker}")
            }
            Self::MarkerDisallowed { marker, offset } => {
                write!(f, "disallowed marker {marker} at byte offset {offset}")
            }
            Self::MarkerUnknown { marker, offset } => {
                write!(f, "unknown marker {marker} at byte offset {offset}")
            }
            Self::MarkerUnexpected {
                actual_marker,
                expected_marker,
                offset,
            } => {
                write!(f, "unexpected marker {actual_marker} expected {expected_marker} at byte offset {offset}",)
            }
            Self::TileGridOffsetOverflow {
                image_horizontal_offset,
                image_vertical_offset,
                tile_horizontal_offset,
                tile_vertical_offset,
            } => {
                write!(
                    f,
                    "tile grid offset overflow: XOSiz = {:?}, YOsiz = {:?}, XTOsiz = {:?}, YTOsiz = {:?}",
                    image_horizontal_offset,
                    image_vertical_offset,
                    tile_horizontal_offset,
                    tile_vertical_offset,
                )
            }
            // XTsiz + XTOsiz > XOsiz
            // YTsiz + YTOsiz > YOsiz
            Self::TileSizeOverflow {
                image_horizontal_offset,
                image_vertical_offset,
                tile_horizontal_offset,
                tile_vertical_offset,
                reference_tile_width,
                reference_tile_height,
            } => {
                write!(
                    f,
                    "tile size overflow: XOSiz = {:?}, YOsiz = {:?}, XTOsiz = {:?}, YTOsiz = {:?}, XTsize = {:?}, YTsize = {:?}",
                    image_horizontal_offset,
                    image_vertical_offset,
                    tile_horizontal_offset,
                    tile_vertical_offset,
                    reference_tile_width,
                    reference_tile_height,
                )
            }
            Self::MarkerMalformed { marker, offset } => {
                write!(f, "malformed marker {marker} at byte offset {offset}",)
            }
            Self::UnsupportedFeature { marker, offset } => {
                write!(
                    f,
                    "unsupported feature for marker {marker} at byte offset {offset}",
                )
            }
            Self::InputFormatError { error } => write!(f, "Unknown error in input: {}", error),
        }
    }
}

impl From<io::Error> for CodestreamError {
    fn from(value: io::Error) -> Self {
        CodestreamError::InputFormatError {
            error: value.to_string(),
        }
    }
}

#[derive(Default, PartialEq, Eq)]
struct MarkerSymbol([u8; 2]);
impl MarkerSymbol {
    fn decode<R: io::Read + io::Seek>(reader: &mut R) -> io::Result<MarkerSymbol> {
        let mut marker_type = MarkerSymbol::default();
        reader.read_exact(&mut marker_type.0)?;
        Ok(marker_type)
    }
}

impl From<MarkerSymbol> for String {
    fn from(value: MarkerSymbol) -> Self {
        format!("{}", value)
    }
}

impl fmt::Debug for MarkerSymbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:0>2X}{:0>2X}", self.0[0], self.0[1])
    }
}

impl fmt::Display for MarkerSymbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} (0x{:0>2X}{:0>2X})",
            match *self {
                MARKER_SYMBOL_SOC => "SOC",
                MARKER_SYMBOL_SOT => "SOT",
                MARKER_SYMBOL_SOD => "SOD",
                MARKER_SYMBOL_EOC => "EOC",
                MARKER_SYMBOL_SIZ => "SIZ",
                MARKER_SYMBOL_PRF => "PRF",
                MARKER_SYMBOL_CAP => "CAP",
                MARKER_SYMBOL_COD => "COD",
                MARKER_SYMBOL_COC => "COC",
                MARKER_SYMBOL_RGN => "RGN",
                MARKER_SYMBOL_QCD => "QCD",
                MARKER_SYMBOL_QCC => "QCC",
                MARKER_SYMBOL_POC => "POC",
                MARKER_SYMBOL_TLM => "TLM",
                MARKER_SYMBOL_PLM => "PLM",
                MARKER_SYMBOL_PLT => "PLT",
                MARKER_SYMBOL_PPM => "PPM",
                MARKER_SYMBOL_PPT => "PPT",
                MARKER_SYMBOL_SOP => "SOP",
                MARKER_SYMBOL_EPH => "EPH",
                MARKER_SYMBOL_CRG => "CRG",
                MARKER_SYMBOL_COM => "COM",
                MARKER_SYMBOL_CPF => "CPF",
                _ => "Unknown Marker",
            },
            self.0[0],
            self.0[1]
        )
    }
}

// Markers and segment markers from ITU T.800 | ISO/IEC 15444-1 Table A.2
// Delimiting markers and marker segments

/// Start of code stream
const MARKER_SYMBOL_SOC: MarkerSymbol = MarkerSymbol([0xFF, 0x4F]);
/// Start of tile-part
const MARKER_SYMBOL_SOT: MarkerSymbol = MarkerSymbol([0xFF, 0x90]);
/// Start of data
const MARKER_SYMBOL_SOD: MarkerSymbol = MarkerSymbol([0xFF, 0x93]);
/// End of codestream
const MARKER_SYMBOL_EOC: MarkerSymbol = MarkerSymbol([0xFF, 0xD9]);

// Fixed information marker segments
/// Image and tile size
const MARKER_SYMBOL_SIZ: MarkerSymbol = MarkerSymbol([0xFF, 0x51]);
/// Profile
const MARKER_SYMBOL_PRF: MarkerSymbol = MarkerSymbol([0xFF, 0x56]);
/// Extended capabilities
const MARKER_SYMBOL_CAP: MarkerSymbol = MarkerSymbol([0xFF, 0x50]);

// Functional marker segments
/// Coding style default
const MARKER_SYMBOL_COD: MarkerSymbol = MarkerSymbol([0xFF, 0x52]);
/// Coding style component
const MARKER_SYMBOL_COC: MarkerSymbol = MarkerSymbol([0xFF, 0x53]);
/// Region-of-interest
const MARKER_SYMBOL_RGN: MarkerSymbol = MarkerSymbol([0xFF, 0x5E]);
/// Quantization default
const MARKER_SYMBOL_QCD: MarkerSymbol = MarkerSymbol([0xFF, 0x5C]);
/// Quantization component
const MARKER_SYMBOL_QCC: MarkerSymbol = MarkerSymbol([0xFF, 0x5D]);
/// Progression order change
const MARKER_SYMBOL_POC: MarkerSymbol = MarkerSymbol([0xFF, 0x5F]);

// Pointer marker segments
/// Tile-part lengths
const MARKER_SYMBOL_TLM: MarkerSymbol = MarkerSymbol([0xFF, 0x55]);
/// Packet length, main header
const MARKER_SYMBOL_PLM: MarkerSymbol = MarkerSymbol([0xFF, 0x57]);
/// Packet length, tile-part header
const MARKER_SYMBOL_PLT: MarkerSymbol = MarkerSymbol([0xFF, 0x58]);
/// Packed packet headers, main header
const MARKER_SYMBOL_PPM: MarkerSymbol = MarkerSymbol([0xFF, 0x60]);
/// Packed packet headers, tile-part header
const MARKER_SYMBOL_PPT: MarkerSymbol = MarkerSymbol([0xFF, 0x61]);

// In bit stream markers and marker segments
/// Start of packet
const MARKER_SYMBOL_SOP: MarkerSymbol = MarkerSymbol([0xFF, 0x91]);
/// End of packet header
const MARKER_SYMBOL_EPH: MarkerSymbol = MarkerSymbol([0xFF, 0x92]);

// Informational marker segments
/// Component registration
const MARKER_SYMBOL_CRG: MarkerSymbol = MarkerSymbol([0xFF, 0x63]);
/// Comment
const MARKER_SYMBOL_COM: MarkerSymbol = MarkerSymbol([0xFF, 0x64]);

// Marker segment from ITU-T T.814 | ISO/IEC 15444-15 Section A.6:
/// Corresponding profile
const MARKER_SYMBOL_CPF: MarkerSymbol = MarkerSymbol([0xFF, 0x59]);

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum ProgressionOrder {
    /// Layer-resolution level-component-position progression
    ///
    /// 0b0000_0000
    LRLCPP,

    /// Resolution level-layer-component-position progression
    ///
    /// 0b0000_0001
    RLLCPP,

    /// 0000 0010 Resolution level-position-component-layer progression
    ///
    /// 0b0000_0010
    RLPCLP,

    /// Position-component-resolution level-layer progression
    ///
    /// 0b0000_0011
    PCRLLP,

    /// Component-position-resolution level-layer progression
    ///
    /// 0b0000_0100
    CPRLLP,

    /// All other values reserved
    Reserved { value: u8 },
}

impl ProgressionOrder {
    fn new(value: u8) -> ProgressionOrder {
        match value {
            0b0000_0000 => ProgressionOrder::LRLCPP,
            0b0000_0001 => ProgressionOrder::RLLCPP,
            0b0000_0010 => ProgressionOrder::RLPCLP,
            0b0000_0011 => ProgressionOrder::PCRLLP,
            0b0000_0100 => ProgressionOrder::CPRLLP,
            _ => ProgressionOrder::Reserved { value },
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum CodingBlockStyle {
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
    Reserved { value: [u8; 1] },
}

impl CodingBlockStyle {
    fn new(value: u8) -> Vec<CodingBlockStyle> {
        let mut coding_block_styles: Vec<CodingBlockStyle> = vec![];

        if value & 0b_0000_0001 != 0 {
            coding_block_styles.push(CodingBlockStyle::SelectiveArithmeticCodingBypass);
        } else {
            coding_block_styles.push(CodingBlockStyle::NoSelectiveArithmeticCodingBypass);
        }

        if value & 0b_0000_0010 != 0 {
            coding_block_styles.push(CodingBlockStyle::ResetContextProbabilities);
        } else {
            coding_block_styles.push(CodingBlockStyle::NoResetOfContextProbabilities);
        }

        if value & 0b_0000_0100 != 0 {
            coding_block_styles.push(CodingBlockStyle::TerminationOnEachCodingPass);
        } else {
            coding_block_styles.push(CodingBlockStyle::NoTerminationOnEachCodingPass);
        }

        if value & 0b_0000_1000 != 0 {
            coding_block_styles.push(CodingBlockStyle::VerticallyCausalContext);
        } else {
            coding_block_styles.push(CodingBlockStyle::NoVerticallyCausalContext);
        }

        if value & 0b_0001_0000 != 0 {
            coding_block_styles.push(CodingBlockStyle::PredictableTermination);
        } else {
            coding_block_styles.push(CodingBlockStyle::NoPredictableTermination);
        }

        if value & 0b_0010_0000 != 0 {
            coding_block_styles.push(CodingBlockStyle::SegmentationSymbolsAreUsed);
        } else {
            coding_block_styles.push(CodingBlockStyle::NoSegmentationSymbolsAreUsed);
        }

        coding_block_styles
    }
}

// A.13 – Coding style parameter values for the Scod parameter
#[derive(Debug, PartialEq)]
pub enum CodingStyleDefault {
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
    Reserved { value: u8 },
}

impl CodingStyleDefault {
    fn new(value: u8) -> Vec<CodingStyleDefault> {
        let mut coding_styles: Vec<CodingStyleDefault> = vec![];

        if value & 0b11111001 == 0 {
            coding_styles.push(CodingStyleDefault::EntropyCoderWithPrecinctsDefined);
        } else if value & 0b11111001 == 0b0001 {
            coding_styles.push(CodingStyleDefault::EntropyCoderWithPrecincts);
        }

        if value & 0b11111010 == 0 {
            coding_styles.push(CodingStyleDefault::NoSOP);
        } else if value & 0b11111010 == 0b10 {
            coding_styles.push(CodingStyleDefault::SOP);
        }

        if value & 0b11111100 == 0 {
            coding_styles.push(CodingStyleDefault::NoEPH);
        } else if value & 0b11111100 == 0b0100 {
            coding_styles.push(CodingStyleDefault::EPH);
        }

        // TODO implement ISO/IEC 15444-1 Table A.13 reservered
        // TODO implement ISO/IEC 15444-2 Table A.5 extensions

        coding_styles
    }
}

#[derive(Debug, PartialEq)]
pub enum CodingStyleComponent {
    // 0000 0000 Entropy coder, precincts with PPx = 15 and PPy = 15
    EntropyCoderWithPrecincts,

    // 0000 0001 Entropy coder with precincts defined below
    EntropyCoderWithPrecinctsDefined,

    // All other values reserved
    Reserved { value: u8 },
}

impl CodingStyleComponent {
    fn new(value: u8) -> CodingStyleComponent {
        if value == 0b_0000_0000 {
            return CodingStyleComponent::EntropyCoderWithPrecinctsDefined;
        } else if value == 0b_0000_0001 {
            return CodingStyleComponent::EntropyCoderWithPrecincts;
        }

        CodingStyleComponent::Reserved { value }
    }
}

const MULTIPLE_COMPONENT_TRANSFORMATION_NONE: u8 = 0b_0000_0000;
const MULTIPLE_COMPONENT_TRANSFORMATION_MULTIPLE: u8 = 0b_0000_0001;

#[derive(Debug, PartialEq)]
pub enum MultipleComponentTransformation {
    // No multiple component transformation specified.
    None,

    // Component transformation used on components 0, 1, 2 for coding efficiency.
    // Irreversible component transformation used with the 9-7 irreversible filter.
    // Reversible component transformation used with the 5-3 reversible filter.
    Multiple,

    // All other values reserved
    Reserved { value: u8 },
}

impl MultipleComponentTransformation {
    fn new(value: u8) -> MultipleComponentTransformation {
        match value {
            MULTIPLE_COMPONENT_TRANSFORMATION_NONE => MultipleComponentTransformation::None,
            MULTIPLE_COMPONENT_TRANSFORMATION_MULTIPLE => MultipleComponentTransformation::Multiple,
            _ => MultipleComponentTransformation::Reserved { value },
        }
    }
}

const TRANSFORMATION_FILTER_IRREVERSIBLE: [u8; 1] = [0];
const TRANSFORMATION_FILTER_REVERSIBLE: [u8; 1] = [1];

#[derive(Debug, PartialEq)]
pub enum TransformationFilter {
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

// A.4.2
//
// Start of tile-part (SOT)
//
// Function: Marks the beginning of a tile-part, the index of its tile, and the
// index of its tile-part. The tile-parts of a given tile shall appear in order
// (see TPsot) in the codestream. However, tile-parts from other tiles may be
// interleaved in the codestream. Therefore, the tile-parts from a given tile
// may not appear contiguously in the codestream.
#[derive(Debug, Default)]
pub struct StartOfTileSegment {
    offset: u64,
    length: u16,

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
    tile_length: u32,

    // TPsot: Tile-part index.
    //
    // There is a specific order required for decoding tile-parts; this index
    // denotes the order from 0.
    //
    // If there is only one tile-part for a tile then this value is zero.
    //
    // The tile-parts of this tile shall appear in the codestream in this order,
    // although not necessarily consecutively.
    tile_part_index: [u8; 1],

    // TNsot: Number of tile-parts of a tile in the codestream.
    //
    // Two values are allowed: the correct number of tile-parts for that tile
    // and zero. A zero value indicates that the number of tile-parts of this
    // tile is not specified in this tile-part.
    no_tile_parts: [u8; 1],
}

// A.12
//
// Coding style default (COD)
//
// Function: Describes the coding style, number of decomposition levels,
// and layering that is the default used for compressing all components of
// an image (if in the main header) or a tile (if in the tile-part header).
//
// The parameter values can be overridden for an individual component by a
// COC marker segment in either the main or tile-part header.
#[derive(Debug, Default)]
pub struct CodingStyleMarkerSegment {
    offset: u64,

    length: u16,

    coding_style: [u8; 1],

    // Progression order
    progression_order: [u8; 1],

    // Number of layers
    no_layers: [u8; 2],

    // Multiple component transformation
    multiple_component_transformation: [u8; 1],

    coding_style_parameters: CodingStyleParameters,
}

impl CodingStyleMarkerSegment {
    pub fn length(&self) -> u16 {
        self.length
    }

    pub fn offset(&self) -> u64 {
        self.offset
    }

    pub fn coding_style(&self) -> u8 {
        self.coding_style[0]
    }

    pub fn coding_styles(&self) -> Vec<CodingStyleDefault> {
        CodingStyleDefault::new(self.coding_style[0])
    }

    pub fn progression_order(&self) -> ProgressionOrder {
        ProgressionOrder::new(self.progression_order[0])
    }

    pub fn no_layers(&self) -> u16 {
        u16::from_be_bytes(self.no_layers)
    }

    pub fn multiple_component_transformation(&self) -> MultipleComponentTransformation {
        MultipleComponentTransformation::new(self.multiple_component_transformation[0])
    }

    pub fn coding_style_parameters(&self) -> &CodingStyleParameters {
        &self.coding_style_parameters
    }
}

// A.6.2
//
// Coding style component (COC)
//
// Function: Describes the coding style, number of decomposition levels, and
// layering used for compressing a particular component.
#[derive(Debug, Default)]
pub struct CodingStyleComponentSegment {
    offset: u64,

    length: u16,

    // Ccoc: The index of the component to which this marker segment relates.
    index: [u8; 2],

    // Scoc: Coding style for this component
    coding_style: [u8; 1],

    // SPcoc: Parameters for coding style designated in Scoc.
    coding_style_parameters: CodingStyleParameters,
}

impl CodingStyleComponentSegment {
    pub fn length(&self) -> u16 {
        self.length
    }

    pub fn offset(&self) -> u64 {
        self.offset
    }

    pub fn component_index(&self) -> u16 {
        u16::from_be_bytes(self.index)
    }

    pub fn component_coding_style(&self) -> CodingStyleComponent {
        CodingStyleComponent::new(self.coding_style[0])
    }
}

#[derive(Debug, Default)]
pub struct CodingStyleParametersPrecinctSize {
    value: u8,
}

impl CodingStyleParametersPrecinctSize {
    pub fn height_exponent(&self) -> u8 {
        // 4 LSBs are the precinct width exponent, PPx = value
        self.value << 4 >> 4
    }

    pub fn width_exponent(&self) -> u8 {
        // 4 MSBs are the precinct height exponent PPy = value
        self.value >> 4
    }
}

// A.12 – Coding style default parameter values
#[derive(Debug, Default)]
pub struct CodingStyleParameters {
    // Coding style
    coding_style: [u8; 1],

    // Number of decomposition levels, N_L, Zero implies no transformation
    no_decomposition_levels: [u8; 1],

    // Code-block width exponent offset value, xcb
    code_block_width: [u8; 1],

    // Code-block height exponent offset value, ycb
    code_block_height: [u8; 1],

    // Style of the code-block coding passes
    code_block_style: [u8; 1],

    // Wavelet transformation used.
    transformation: [u8; 1],

    // If Scod or Scoc = xxxx xxx0, this parameter is not present; otherwise
    // this indicates precinct width and height.
    precinct_size: Vec<u8>,
}

impl CodingStyleParameters {
    pub fn no_decomposition_levels(&self) -> u8 {
        self.no_decomposition_levels[0]
    }

    // A.18
    //
    // Code-block width and height exponent offset value xcb = value + 2 or ycb = value + 2.
    //
    // TODO: validate
    // The code-block width and height are limited to powers of two with the minimum size being 2^2 and the maximum
    // being 2^10.
    //
    // Furthermore, the code-block size is restricted so that xcb + ycb <= 12.
    pub fn code_block_width(&self) -> u16 {
        2u16.pow(((self.code_block_width[0] & 0b00001111) + 2) as u32)
    }

    pub fn code_block_height(&self) -> u16 {
        2u16.pow(((self.code_block_height[0] & 0b00001111) + 2) as u32)
    }

    pub fn code_block_style(&self) -> u8 {
        self.code_block_style[0]
    }

    pub fn coding_block_styles(&self) -> Vec<CodingBlockStyle> {
        CodingBlockStyle::new(self.code_block_style[0])
    }

    pub fn transformation(&self) -> TransformationFilter {
        TransformationFilter::new(self.transformation)
    }

    pub fn has_defined_precinct_size(&self) -> bool {
        self.coding_style[0] & 0b1001 == 1
    }

    pub fn has_default_precinct_size(&self) -> bool {
        self.coding_style[0] & 0b1001 == 0
    }

    pub fn precinct_sizes(&self) -> Option<Vec<CodingStyleParametersPrecinctSize>> {
        // If entropy coder, precincts with PPx = 15 and PPy = 15
        if self.has_default_precinct_size() {
            return Some(vec![CodingStyleParametersPrecinctSize { value: 255 }]);
        }

        Some(
            self.precinct_size
                .iter()
                .map(|value: &u8| CodingStyleParametersPrecinctSize { value: *value })
                .collect(),
        )
    }
}

pub enum RegionOfInterestStyle {
    ImplicitRegionOfInterest,
    Reserved { value: u8 },
}
impl RegionOfInterestStyle {
    fn new(value: u8) -> RegionOfInterestStyle {
        match value {
            0 => RegionOfInterestStyle::ImplicitRegionOfInterest,
            _ => RegionOfInterestStyle::Reserved { value },
        }
    }
}

// A.6.3
//
// Region of interest (RGN)
//
// Function: Signals the presence of an ROI in the codestream.
#[derive(Debug, Default)]
pub struct RegionOfInterestSegment {
    offset: u64,

    // Lrgn: Length of marker segment in bytes (not including the marker)
    length: u16,

    // Crgn: The index of the component to which this marker segment relates.
    // The components are indexed 0, 1, 2, etc.
    component_index: [u8; 2],

    // Srgn: ROI style for the current ROI.
    region_of_interest_style: [u8; 1],

    // SPrgn: Parameter for ROI style designated in Srgn.
    region_of_interest_style_parameter: [u8; 1],
}

// A.6.6
//
// Progression order change (POC)
//
// Function: Describes the bounds and progression order for any progression
// order other than specified in the COD marker segments in the codestream.
#[derive(Debug, Default)]
pub struct ProgressionOrderChangeSegment {
    offset: u64,
    length: u16,

    progressions: Vec<CodingStyleComponentSegmentProgression>,
}

#[derive(Debug, Default)]
pub struct CodingStyleComponentSegmentProgression {
    // RSpoc: Resolution level index (inclusive) for the start of a progression.
    resolution_level_index_start: [u8; 1],

    // Ccoc: The index of the component to which this marker segment relates.
    // The components are indexed 0, 1, 2, etc.
    component_index_start: [u8; 2],

    // LYEpoc: Layer index (exclusive) for the end of a progression.
    // The layer index always starts at zero for every progression. Packets
    // that have already been included in the codestream are not included again
    layer_index_end: [u8; 2],

    // REpoc: Resolution Level index (exclusive) for the end of a progression.
    resolution_level_index_end: [u8; 1],

    // CEpoc: Component index (exclusive) for the end of a progression.
    component_index_end: [u8; 2],

    // Ppoc: Progression order.
    progression_order: [u8; 1],
}

impl CodingStyleComponentSegmentProgression {
    pub fn component_index_start(&self) -> u16 {
        u16::from_be_bytes(self.component_index_start)
    }

    pub fn component_index_end(&self) -> u16 {
        // TODO: Verify
        u16::from_be_bytes(self.component_index_end)
    }

    pub fn progression_order(&self) -> ProgressionOrder {
        ProgressionOrder::new(self.progression_order[0])
    }
}

pub enum DecoderCapability {
    Part1,
    Reserved { value: [u8; 2] },
}
impl DecoderCapability {
    fn new(value: [u8; 2]) -> Vec<DecoderCapability> {
        match value {
            [0, 0] => vec![DecoderCapability::Part1],
            _ => vec![DecoderCapability::Reserved { value }],
        }
    }
}

/// Tile-part lengths (TLM).
///
/// Function: Describes the length of every tile-part in the codestream. Each
/// tile-part's length is measured from the first byte of the SOT marker segment
/// to the end of the bit-stream data of that tile-part. The value of each
/// individual tile-part length in the TLM marker segment is the same as the
/// value in the corresponding Psot in the SOT marker segment.
///
/// Usage: Main header. It can be used optionally in the main header only.
/// There may be multiple TLM marker segments in the main header.
///
/// See ITU-T T.800(V4) | ISO/IEC 15444-1:2024 Section A.7 and A.7.1.
#[derive(Debug, Default)]
pub struct TilePartLengthsSegment {
    offset: u64,

    // Ltlm: Length of marker segment in bytes (not including the marker).
    length: u16,

    // Ztlm: Index of this marker segment relative to all other TLM marker
    // segments present in the current header.
    index: [u8; 1],

    // Stlm: Size of the Ttlm and Ptlm parameters
    parameter_sizes: [u8; 1],

    tile_part_lengths: Vec<TilePartLength>,
}

impl TilePartLengthsSegment {
    fn parameter_sizes(&self) -> Vec<TilePartParameterSize> {
        TilePartParameterSize::new(self.parameter_sizes[0])
    }

    /// Marker segment index (Ztlm).
    ///
    /// Index of this marker segment relative to all other TLM marker
    /// segments present in the current header.
    pub fn segment_index(&self) -> u8 {
        self.index[0]
    }

    /// Tile part lengths (Ttlm<sup>i</sup> / Ptlm<sup>i</sup>).
    ///
    /// Each entry in the vector corresponds to the index (implied or explicit)
    /// of the tile-part, and the length of the tile-part.
    pub fn tile_part_lengths(&self) -> &Vec<TilePartLength> {
        &self.tile_part_lengths
    }
}

/// Tile part length.
///
/// This provides one entry in the TLM segment.
#[derive(Debug, Default)]
pub struct TilePartLength {
    // Ttlm^i: Tile index of the ith tile-part.
    //
    // There is either none or one value for every tile-part.
    // The number of tile-parts in each tile can be derived from this marker
    // segment (or the concatenated list of all such markers) or from a
    // non-zero TNsot parameter, if present.
    tile_index: Option<u16>,

    // Ptlm^i: Length in bytes, from the beginning of the SOT marker of the ith
    // tile-part to the end of the bit stream data for that tile-part.
    //
    // There is one value for every tile-part
    tile_length: u32,
}

impl TilePartLength {
    /// Tile index (Ttlm<sup>i</sup>).
    ///
    /// Tile index of the _ith_ tile-part. There is either none or one value for every
    /// tile-part. The number of tile-parts in each tile can be derived from this marker
    /// segment (or the concatenated list of all such markers) or from a non-zero TNsot
    /// parameter, if present.
    ///
    /// If this is None, the Ttlm parameter is encoded in 0 bits, which means
    /// only one tile-part per tile and the tiles are in index order without omission
    /// or repetition.
    pub fn tile_index(&self) -> &Option<u16> {
        &self.tile_index
    }

    /// Tile-part length (Ptlm<sup>i</sup>).
    ///
    /// Length in bytes, from the beginning of the SOT marker of the _ith_ tile-part
    /// to the end of the bit stream data for that tile-part. There is one value for
    /// every tile-part.
    pub fn tile_length(&self) -> u32 {
        self.tile_length
    }
}
#[derive(Debug, PartialEq)]
enum TilePartParameterSize {
    TtlmNone,
    Ttlm8Bit,
    Ttlm16Bit,
    Ptlm16Bit,
    Ptlm32Bit,
    Reserved { value: u8 },
}

impl TilePartParameterSize {
    fn new(value: u8) -> Vec<TilePartParameterSize> {
        let mut tile_part_parameter_sizes = vec![];

        match (value >> 4) & 0b11 {
            0 => tile_part_parameter_sizes.push(TilePartParameterSize::TtlmNone),
            1 => tile_part_parameter_sizes.push(TilePartParameterSize::Ttlm8Bit),
            2 => tile_part_parameter_sizes.push(TilePartParameterSize::Ttlm16Bit),
            _ => {} // TODO: Add reserve values by removed known bits
        }

        match (value >> 6) & 0b1 {
            0 => tile_part_parameter_sizes.push(TilePartParameterSize::Ptlm16Bit),
            1 => tile_part_parameter_sizes.push(TilePartParameterSize::Ptlm32Bit),
            _ => {} // TODO: Add reserve values by removed known bits
        }

        tile_part_parameter_sizes
    }
}

// A.7.2
//
// Packet length, main header (PLM)
//
// Function: A list of packet lengths in the tile-parts for every tile-part in
// order.
#[derive(Debug, Default)]
pub struct PacketLengthSegment {
    offset: u64,

    // Lplm: Length of marker segment in bytes (not including the marker).
    length: u16,

    // Zplm: Index of this marker segment relative to all other PLM marker
    // segments present in the current header.
    //
    // The sequence of (Nplmi, Iplmi) parameters from this marker segment is
    // concatenated, in the order of increasing Zplm, with the sequences of
    // parameters from other marker segments.
    //
    // The kth entry in the resulting list contains the number of bytes and
    // packet header pair for the kth tile-part appearing in the codestream.
    //
    // Every marker segment in this series shall end with a completed packet
    // header length. However, the series of Iplm parameters described by the
    // Nplm does not have to be complete in a given marker segment. Therefore,
    // it is possible that the next PLM marker segment will not have an Nplm
    // parameter after Zplm, but the continuation of the Iplm series from the
    // last PLM marker segment.
    index: [u8; 1],

    // Nplm^i: Number of bytes of Iplm information for the ith tile-part in the
    // order found in the codestream.
    //
    // There is one value for each tile-part. If a codestream contains one or
    // more tile-parts exceeding the limitations of PLM markers, these markers
    // shall not be used.
    no_bytes: [u8; 1],

    // Iplm^ij: Length of the jth packet in the ith tile-part.
    //
    // If packet headers are stored with the packet, this length includes the
    // packet header.
    // If packet headers are stored in the PPM or PPT, this length does not
    // include the packet header length.
    //
    // There is one range of values for each tile-part.
    // There is one value for each packet in the tile.
    packet_length: Vec<u8>,
}

impl PacketLengthSegment {
    fn no_bytes(&self) -> u8 {
        u8::from_be_bytes(self.no_bytes)
    }
}

// A.7.3
//
// Packet length, tile-part header (PLT)
//
// Function: A list of packet lengths in the tile-part
#[derive(Debug, Default)]
pub struct TilePacketLength {
    offset: u64,

    // Lplt: Length of marker segment in bytes (not including the marker).
    length: u16,

    // Zplt: Index of this marker segment relative to all other PLT marker
    // segments present in the current header.
    //
    // The sequence of (Iplti) parameters from this marker segment is
    // concatenated, in the order of increasing Zplt, with the sequences of
    // parameters from other marker segments.
    //
    // Every marker segment in this series shall end with a completed packet
    // header length.
    index: [u8; 1],

    // Iplt^i: Length of the ith packet.
    //
    // If packet headers are stored with the packet, this length includes the
    // packet header. If packet headers are stored in the PPM or PPT, this
    // length does not include the packet header lengths.
    packet_length: Vec<u64>,
}

// A.7.4
//
// Packed packet headers, main header (PPM)
//
// Function: A collection of the packet headers from all tiles.
#[derive(Debug, Default)]
pub struct PackedPacketHeaderSegment {
    offset: u64,

    // Lppm: Length of marker segment in bytes, not including the marker.
    length: u16,

    // Zppm: Index of this marker segment relative to all other PPM marker
    // segments present in the main header.
    index: [u8; 1],

    // Nppm^i: Number of bytes of Ippm information for the ith tile-part in the
    // order found in the codestream. One value for each tile-part (not tile).
    number_of_bytes: [u8; 4],

    // Ippm^ij: Packet header for every packet in order in the tile-part.
    // The contents are exactly the packet header which would have been
    // distributed in the bit stream as described in B.10
    data: Vec<u8>,
}

impl PackedPacketHeaderSegment {
    pub fn index(&self) -> usize {
        u8::from_be_bytes(self.index) as usize
    }

    pub fn number_of_bytes(&self) -> usize {
        u32::from_be_bytes(self.number_of_bytes) as usize
    }
}

// A.7.5
//
// Packed packet headers, tile-part header (PPT)
//
// Function: A collection of the packet headers from one tile or tile-part.
#[derive(Debug, Default)]
pub struct TilePackedPacketHeaderSegment {
    offset: u64,

    // Lppt: Length of marker segment in bytes, not including the marker.
    length: u16,

    // Zppt: Index of this marker segment relative to all other PPT marker
    // segments present in the current header.
    //
    // The sequence of (Ippti) parameters from this marker segment is
    // concatenated, in the order of increasing Zppt, with the sequences of
    // parameters from other marker segments. Every marker segment in this
    // series shall end with a completed packet header.
    index: [u8; 1],

    // Ippt^i: Packet header for every packet in order in the tile-part.
    //
    // The component index, layer, and resolution level are determined from the
    // method of progression or POC marker segments.
    //
    // The contents are exactly the packet header which would have been
    // distributed in the bit stream as described in B.10.
    data: Vec<u8>,
}

impl TilePackedPacketHeaderSegment {
    pub fn index(&self) -> usize {
        u8::from_be_bytes(self.index) as usize
    }
}

// A.9.1
//
// Component registration (CRG)
//
// Function: Allows specific registration of components with respect to each
// other. For coding purposes the samples of components are considered to be
// located at reference grid points that are integer multiples of XRsiz and
// YRsiz.
//
// However, this may be inappropriate for rendering the image. The CRG marker
// segment describes the "centre of mass" of each component's samples with
// respect to the separation.
//
// This marker segment has no effect on decoding the codestream.
#[derive(Debug, Default)]
pub struct ComponentRegistrationSegment {
    offset: u64,

    // Lcrg: Length of marker segment in bytes (not including the marker).
    length: u16,

    // Xcrg^i: Value of the horizontal offset, in units of 1/65536 of the
    // horizontal separation XRsizi, for the ith component.
    //
    // Thus, values range from 0/65536 (sample occupies its reference grid
    // point) to XRsizc(65535/65536) (just before the next sample's reference
    // grid point).
    //
    // This value is repeated for every component.
    horizontal_offset: Vec<[u8; 2]>,

    // Ycrg^i: Value of the vertical offset, in units of 1/65536 of the
    // vertical separation YRsizi, for the ith component.
    //
    // Thus, values range from 0/65536 (sample occupies its reference grid
    // point) to YRsizc(65535/65536) (just before the next sample's reference grid point).
    // This value is repeated for every component.
    vertical_offset: Vec<[u8; 2]>,
}

// A.5.1
//
// Image and tile size (SIZ)
//
// Function: Provides information about the uncompressed image such as the
// width and height of the reference grid, the width and height of the tiles,
// the number of components, component bit depth, and the separation of
// component samples with respect to the reference grid.
#[derive(Debug, Default)]
pub struct ImageAndTileSizeMarkerSegment {
    offset: u64,
    length: u16,

    // Rsiz: Denotes capabilities that a decoder needs to properly decode the
    // codestream.
    decoder_capabilities: [u8; 2],

    // XSiz: Width of the reference grid.
    reference_grid_width: [u8; 4],

    // YSiz: Height of the reference grid.
    reference_grid_height: [u8; 4],

    // XOsiz: Horizontal offset from the origin of the reference grid to the
    // top side of the image area.
    image_horizontal_offset: [u8; 4],

    // YOsiz: Vertical offset from the origin of the reference grid to the top
    // side of the image area.
    image_vertical_offset: [u8; 4],

    // XTsiz: Width of one reference tile with respect to the reference grid
    reference_tile_width: [u8; 4],

    // YTsiz: Height of one reference tile with respect to the reference grid.
    reference_tile_height: [u8; 4],

    // XTOsiz: Horizontal offset from the origin of the reference grid to the
    // left side of the first tile.
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
    pub fn length(&self) -> u16 {
        self.length
    }

    pub fn offset(&self) -> u64 {
        self.offset
    }

    pub fn decoder_capabilities(&self) -> u16 {
        u16::from_be_bytes(self.decoder_capabilities)
    }

    pub fn reference_grid_width(&self) -> u32 {
        u32::from_be_bytes(self.reference_grid_width)
    }
    pub fn reference_grid_height(&self) -> u32 {
        u32::from_be_bytes(self.reference_grid_height)
    }

    pub fn image_horizontal_offset(&self) -> u32 {
        u32::from_be_bytes(self.image_horizontal_offset)
    }
    pub fn image_vertical_offset(&self) -> u32 {
        u32::from_be_bytes(self.image_vertical_offset)
    }

    pub fn reference_tile_width(&self) -> u32 {
        u32::from_be_bytes(self.reference_tile_width)
    }
    pub fn reference_tile_height(&self) -> u32 {
        u32::from_be_bytes(self.reference_tile_height)
    }

    pub fn tile_horizontal_offset(&self) -> u32 {
        u32::from_be_bytes(self.tile_horizontal_offset)
    }
    pub fn tile_vertical_offset(&self) -> u32 {
        u32::from_be_bytes(self.tile_vertical_offset)
    }

    pub fn no_components(&self) -> u16 {
        u16::from_be_bytes(self.no_components)
    }

    pub fn precision(&self, i: usize) -> Result<u8, CodestreamError> {
        let ssiz = self
            .precision
            .get(i)
            .ok_or(CodestreamError::InputFormatError {
                error: String::from("unable to get precision for component"),
            })?;
        let precision = u8::from_be_bytes(*ssiz) & 0x7f;
        // ISO/IEC 15444-1:2019 Table A.11, component bit depth is value + 1.
        Ok(precision + 1)
    }

    pub fn values_are_signed(&self, i: usize) -> Result<bool, CodestreamError> {
        let ssiz = self
            .precision
            .get(i)
            .ok_or(CodestreamError::InputFormatError {
                error: String::from("unable to get signedness for component"),
            })?;
        let is_signed = (u8::from_be_bytes(*ssiz) & 0x80) == 0x80;
        Ok(is_signed)
    }

    pub fn horizontal_separation(&self, i: usize) -> Result<u8, CodestreamError> {
        let horizontal_separation =
            self.horizontal_separation
                .get(i)
                .ok_or(CodestreamError::InputFormatError {
                    error: String::from("unable to get horizontal_separation for component"),
                })?;
        Ok(u8::from_be_bytes(*horizontal_separation))
    }
    pub fn vertical_separation(&self, i: usize) -> Result<u8, CodestreamError> {
        let vertical_separation =
            self.vertical_separation
                .get(i)
                .ok_or(CodestreamError::InputFormatError {
                    error: String::from("unable to get vertical_separation for component"),
                })?;
        Ok(u8::from_be_bytes(*vertical_separation))
    }

    // The number of tiles in the X direction (numXtiles) and the Y direction
    // (numYtiles) is the following
    //
    // numXtiles = [(Xsiz - XTOsiz) / XTsiz]
    // numYtiles = [(Ysiz - YTOsiz) / YTsiz]
    fn num_x_tiles(&self) -> u32 {
        (self.reference_grid_width() - self.tile_horizontal_offset()) / self.reference_tile_width()
    }
    fn num_y_tiles(&self) -> u32 {
        (self.reference_grid_height() - self.tile_vertical_offset()) / self.reference_tile_height()
    }

    // Let p be the horizontal index of a tile, ranging from 0 to numXtiles -1
    // p = mod(t, numXTiles)
    // where t is the index of the tile
    fn tile_horizontal_index(&self, t: u32) -> u32 {
        t % self.num_x_tiles()
    }

    // Let q be the vertical index of a tile, ranging from 0 to numYtiles -1,
    // q = [t / numXtiles]
    // where t is the index of the tile
    fn tile_vertical_index(&self, t: u32) -> u32 {
        t / self.num_x_tiles()
    }

    /// tx_0(p,q) = max(XTOsiz + p · XTsiz, XOsiz)
    fn tile_x0(&self, t: u32) -> u32 {
        cmp::max(
            self.tile_horizontal_offset()
                + (self.tile_horizontal_index(t) * self.reference_tile_width()),
            self.image_horizontal_offset(),
        )
    }

    /// ty_0(p,q) = max(YTOsiz + q · YTsiz, YOsiz)
    fn tile_y0(&self, t: u32) -> u32 {
        cmp::max(
            self.tile_vertical_offset()
                + (self.tile_vertical_index(t) * self.reference_tile_height()),
            self.image_vertical_offset(),
        )
    }

    /// tx_1(p,q) = min(XTOsiz + (p + 1) · XTsiz, Xsiz)
    fn tile_x1(&self, t: u32) -> u32 {
        cmp::min(
            self.tile_horizontal_offset()
                + ((self.tile_horizontal_index(t) + 1) * self.reference_tile_width()),
            self.reference_grid_width(),
        )
    }

    /// ty_1(p,q) = min(YTOsiz + (q + 1) · YTsiz, Ysiz)
    fn tile_y1(&self, t: u32) -> u32 {
        cmp::min(
            self.tile_vertical_offset()
                + ((self.tile_vertical_index(t) + 1) * self.reference_tile_height()),
            self.reference_grid_height(),
        )
    }

    pub fn tile_dimensions(&self, t: u32) -> (u32, u32) {
        (
            self.tile_x1(t) - self.tile_x0(t),
            self.tile_y1(t) - self.tile_y0(t),
        )
    }
}

/// Extended Capabilities (CAP) Marker Segment.
///
/// From ITU-T T.800(V4) | ISO/IEC 15444-1:2024 Section A.5.2:
/// > Function: Signals that extended capabilities were used to create (and are recommended
/// > or required to decode) a codestream.
/// >
/// > Usage: Optional. If present, it must be included in the main header after the SIZ
/// > marker segment and before any other marker segment defined in this Recommendation
/// > | International Standard. The second-most-significant bit in Rsiz may optionally be
/// > set to 1 to indicate the presence of the CAP marker segment.
#[derive(Debug, Default, PartialEq)]
pub struct ExtendedCapabilitiesMarkerSegment {
    offset: u64,

    // Lcap: Length of marker segment in bytes (not including the marker).
    length: u16,

    // 16 bit fields, defined outside T.800 | ISO/IEC 15444-1.
    capabilities: Vec<Option<u16>>,
}

impl ExtendedCapabilitiesMarkerSegment {
    pub fn length(&self) -> u16 {
        self.length
    }

    pub fn offset(&self) -> u64 {
        self.offset
    }

    /// Capabilities flags.
    ///
    /// This is a vector of the flags. Each element in the vector
    /// is a different set of capabilities, typically defined in a
    /// different standard. The element will be None if the flags
    /// were not included, corresponding to a zero in the corresponding
    /// Pcap<sup>i</sup> value.
    ///
    /// Known usages are at:
    ///  - position 2, defined in ITU-T T.801(V4) | ISO/IEC 15444-2:2024 Section A.3.1.13.
    ///  - position 15, defined in ITU-T T.814(06/2019) | ISO/IEC 15444-15:2019 Section A.3.
    ///
    /// Note that the positions are 1 based, so those correspond to 1 and 14 if using
    /// 0 based vector indexing.
    pub fn capabilities(&self) -> &Vec<Option<u16>> {
        &self.capabilities
    }

    /// Capabilities flags.
    ///
    /// This is a single set of flags.
    ///
    /// The value will be None if the flags were not included, corresponding to a zero in the corresponding
    /// Pcap<sup>i</sup> value.
    ///
    /// Known usages are at:
    ///  - position 2, defined in ITU-T T.801(V4) | ISO/IEC 15444-2:2024 Section A.3.1.13.
    ///  - position 15, defined in ITU-T T.814(06/2019) | ISO/IEC 15444-15:2019 Section A.3.
    ///
    /// Note that the positions are 1 based, which is how this function operates.
    pub fn capability(&self, index: u8) -> Option<u16> {
        self.capability_base_zero(index - 1)
    }

    /// Capabilities flags.
    ///
    /// This is a single set of flags.
    ///
    /// The value will be None if the flags were not included, corresponding to a zero in the corresponding
    /// Pcap<sup>i</sup> value.
    ///
    /// Known usages are at:
    ///  - position 2, defined in ITU-T T.801(V4) | ISO/IEC 15444-2:2024 Section A.3.1.13.
    ///  - position 15, defined in ITU-T T.814(06/2019) | ISO/IEC 15444-15:2019 Section A.3.
    ///
    /// Note that the positions are 1 based, so those correspond to 1 and 14 if using
    /// 0 based indexing as in this function.
    pub fn capability_base_zero(&self, index: u8) -> Option<u16> {
        self.capabilities[index as usize]
    }
}

/// Corresponding Profile (CPF) Marker Segment.
///
/// From ITU-T T.814 | ISO/IEC 15444-15 Section A.6:
/// > Function: The corresponding Pprofile (CPF) marker segment is provided to facilitate the reversible
/// > transcoding of HTJ2K codestreams to and from codestreams that conform to Rec. ITU-T T.800 | ISO/IEC
/// > 15444-1.
/// >
/// > Zero or one CPF marker segment shall be present in an HTJ2K codestream.
///
/// > Usage: Optional. If present, the CPF marker segment shall appear after the SIZ marker segment,
/// > CAP marker segment and, if present, the PRF marker segment, but before any other marker segments
/// > defined in Rec. ITU-T T.800 | ISO/IEC 15444-1.
#[derive(Debug, Default, PartialEq)]
pub struct CorrespondingProfileMarkerSegment {
    offset: u64,

    // Lcpf: Length of marker segment in bytes (not including the marker).
    length: u16,

    // Pcpf_i: the integers that encode CPFnum. None of these may be zero
    pcpf: Vec<u16>,
}

impl CorrespondingProfileMarkerSegment {
    pub fn length(&self) -> u16 {
        self.length
    }

    pub fn offset(&self) -> u64 {
        self.offset
    }

    /// Vector of Pcpf<sup>i</sup> values.
    pub fn pcpf_raw(&self) -> &[u16] {
        &self.pcpf
    }

    /// CPFnum.
    ///
    /// This is computed from the Pcpf<sup>i</sup> integers.
    pub fn cpf_num(&self) -> i32 {
        let mut cpf_num = -1i32;
        for i in 0..self.pcpf.len() {
            let pcpf = self.pcpf.get(i).unwrap();
            cpf_num += (pcpf * 2u16.pow((16 * i) as u32)) as i32;
        }
        cpf_num
    }
}

#[derive(Debug, PartialEq)]
pub enum CommentRegistrationValue {
    // General use (binary values)
    Binary,

    // General use (ISO 8859-15:1999 (Latin) values)
    Latin,

    // All other values reserved
    Reserved { value: [u8; 2] },
}

impl CommentRegistrationValue {
    fn new(value: [u8; 2]) -> CommentRegistrationValue {
        match i16::from_be_bytes(value) {
            // See ISO/IEC 15444-1:2019 Table A.44
            0 => CommentRegistrationValue::Binary,
            1 => CommentRegistrationValue::Latin,
            _ => CommentRegistrationValue::Reserved { value },
        }
    }
}

// A.9.2
//
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
    pub fn registration_value(&self) -> CommentRegistrationValue {
        CommentRegistrationValue::new(self.registration_value)
    }

    pub fn comment_utf8(&self) -> Result<&str, str::Utf8Error> {
        str::from_utf8(&self.comment)
    }
}

/// Quantization info contains the style, guard bits, and quantization values
///
/// See ITU-T T.800(V4) or ISO/IEC 15444-1:2024 Section A.6.4
#[derive(Debug)]
pub struct QuantizationInfo {
    pub guard_bits: u8, // 0..=7
    pub style: QuantizationStyle,
    values_bytes: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum QuantizationStyle {
    NoQuantization,
    ScalarDerived,
    ScalarExpounded,
    Reserved(u8),
}

impl QuantizationInfo {
    const SHIFT_GUARD: u8 = 5; // guard bits are encoded into top 3 bits of a u8
    const SHIFT_EXP: u8 = 3;
    const MASK_EXPONENT: u8 = 0b11111; // 5 bits for exponent

    /// encode guard bits and style into u8
    pub fn style_as_u8(&self) -> u8 {
        let g = self.guard_bits << Self::SHIFT_GUARD;
        let qs = match self.style {
            QuantizationStyle::NoQuantization => 0,
            QuantizationStyle::ScalarDerived => 1,
            QuantizationStyle::ScalarExpounded => 2,
            QuantizationStyle::Reserved(v) => v,
        };
        g + qs
    }

    /// Pull the exponent from the high byte of a value
    fn exponent_from_quant_value(v: &u8) -> u8 {
        let v: u8 = *v;
        (v >> Self::SHIFT_EXP) & Self::MASK_EXPONENT
    }

    /// Grab the exponents from the quantization values
    ///
    /// See ITU-T T.800(V4) or ISO/IEC 15444-1:2024 Section A.6.4
    /// See also Tables A.28, A.29, A.30
    pub fn exponents(&self) -> Vec<u8> {
        // values are currently packed in a values_bytes
        match &self.style {
            QuantizationStyle::NoQuantization => self
                .values_bytes
                // each value is logically a u8
                .iter()
                .map(Self::exponent_from_quant_value)
                .collect(),
            QuantizationStyle::ScalarDerived => {
                // Only need the high byte for the single u16
                vec![Self::exponent_from_quant_value(&self.values_bytes[0])]
            }
            QuantizationStyle::ScalarExpounded => self
                .values_bytes
                // logically each value is a u16, but we only need the high byte
                .chunks_exact(2)
                .map(|c| Self::exponent_from_quant_value(&c[0]))
                .collect(),
            QuantizationStyle::Reserved(v) => {
                panic!("unable to convert exponents for unknown QuantStyle {}", v)
            }
        }
    }

    /// raw values
    pub fn values(&self) -> Vec<u16> {
        info!("Weird to grab raw values for quantization style");
        match &self.style {
            QuantizationStyle::NoQuantization => {
                self.values_bytes.iter().map(|v| *v as u16).collect()
            }
            QuantizationStyle::ScalarDerived => {
                let b1 = self.values_bytes[0];
                let b2 = self.values_bytes[1];
                vec![u16::from_be_bytes([b1, b2])]
            }
            QuantizationStyle::ScalarExpounded => self
                .values_bytes
                .chunks_exact(2)
                .map(|b| u16::from_be_bytes(b.try_into().unwrap()))
                .collect(),
            QuantizationStyle::Reserved(v) => {
                panic!("unable to convert values for unknown QuantStyle {}", v)
            }
        }
    }

    /// Decode a QuantizationInfo given expected length
    ///
    /// length should be the length of style and associated values
    fn decode<R: io::Read + io::Seek>(
        reader: &mut R,
        length: u16,
    ) -> Result<QuantizationInfo, CodestreamError> {
        // know from length how much to read
        let qb = {
            let mut buf = [0u8];
            reader.read_exact(&mut buf)?;
            buf[0]
        };
        let guard_bits = qb >> Self::SHIFT_GUARD;
        let style_code = qb & 0b11111; // 5 bits for style
        let style = match style_code {
            0 => {
                if !(length - 2).is_multiple_of(3) {
                    Err(CodestreamError::InputFormatError {
                        error: String::from("Invalid length for quantization style"),
                    })?
                }
                QuantizationStyle::NoQuantization
            }
            1 => {
                if length != 3 {
                    Err(CodestreamError::InputFormatError {
                        error: String::from("Invalid length for quantization style"),
                    })?
                }
                QuantizationStyle::ScalarDerived
            }
            2 => {
                if !(length - 3).is_multiple_of(6) {
                    Err(CodestreamError::InputFormatError {
                        error: String::from("Invalid length for quantization style"),
                    })?
                }
                QuantizationStyle::ScalarExpounded
            }
            _ => {
                // error out because we usually need quantization style
                Err(CodestreamError::InputFormatError {
                    error: format!("Unknown quantization style: {:x}", qb),
                })?
            }
        };

        // grab remaining length after reading style
        let mut buf = vec![0u8; (length - 1) as usize];
        reader.read_exact(&mut buf)?;
        Ok(QuantizationInfo {
            guard_bits,
            style,
            values_bytes: buf,
        })
    }
}

#[derive(Debug)]
enum QuantizationValue {
    Reversible { value: [u8; 1] },
    Irreversible { value: [u8; 2] },
}

impl QuantizationValue {
    fn value(&self) -> u16 {
        match &self {
            QuantizationValue::Reversible { value } => u8::from_be_bytes(*value) as u16,
            QuantizationValue::Irreversible { value } => u16::from_be_bytes(*value),
        }
    }

    fn exponent(&self) -> u8 {
        match &self {
            QuantizationValue::Reversible { value } => u8::from_be_bytes([value[0] >> 3]),
            QuantizationValue::Irreversible { value } => u8::from_be_bytes([value[0] >> 3]),
        }
    }

    fn mantissa(&self) -> u16 {
        match &self {
            QuantizationValue::Reversible { value: _value } => {
                // should't exist?
                panic!();
            }
            // discard 5 most significant bits
            QuantizationValue::Irreversible { value } => {
                u16::from_be_bytes([value[0] << 5 >> 5, value[1]])
            }
        }
    }
}

// A.6.4
//
// Quantization default (QCD)
//
// Function: Describes the quantization default used for compressing all
// components not defined by a QCC marker segment. The parameter values can be
// overridden for an individual component by a QCC marker segment in either the
// main or tile-part header.
#[derive(Debug)]
pub struct QuantizationDefaultMarkerSegment {
    // Length of marker segment in bytes (not including the marker).
    length: u16,

    // Quantization style info for all components
    quantization_info: QuantizationInfo,
}

impl QuantizationDefaultMarkerSegment {
    pub fn length(&self) -> u16 {
        self.length
    }

    pub fn quantization_style_u8(&self) -> u8 {
        self.quantization_info.style_as_u8()
    }

    pub fn quantization_info(&self) -> &QuantizationInfo {
        &self.quantization_info
    }

    pub fn guard_bits(&self) -> u8 {
        self.quantization_info.guard_bits
    }

    pub fn quantization_values(&self) -> Vec<u16> {
        self.quantization_info.values()
    }
}

// A.6.5
//
// Quantization component (QCC)
//
// Function: Describes the quantization used for compressing a particular
// component
#[derive(Debug)]
pub struct QuantizationComponentSegment {
    offset: u64,

    // Lqcc
    length: u16,

    // Cqcc: The index of the component to which this marker segment relates.
    component_index: [u8; 2],

    quantization_info: QuantizationInfo,
}

impl QuantizationComponentSegment {
    pub fn length(&self) -> u16 {
        self.length
    }

    pub fn component_index(&self) -> u16 {
        u16::from_be_bytes(self.component_index)
    }

    pub fn quantization_info(&self) -> &QuantizationInfo {
        &self.quantization_info
    }
}

// Contiguous Codestream
//
// The codestream is a linear stream of bits from the first bit to the last
// bit.
//
// For convenience, it can be divided into (8 bit) bytes, starting with
// the first bit of the codestream, with the "earlier" bit in a byte viewed as
// the most significant bit of the byte when given e.g. a hexadecimal
// representation.
//
// This byte stream may be divided into groups of consecutive bytes.
//
// The hexadecimal value representation is sometimes implicitly assumed in the
// text when describing bytes or group ofbytes that do not have a “natural”
// numeric value representation
#[derive(Debug, Default)]
pub struct ContiguousCodestream {
    offset: u64,
    length: u16,
    header: Header,
    tile_parts: Vec<TilePart>,
}

impl ContiguousCodestream {
    pub fn header(&self) -> &Header {
        &self.header
    }

    // Length of marker segment in bytes (not including the marker).
    fn decode_length<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<u16, Box<dyn error::Error>> {
        let mut length = [0u8; 2];
        reader.read_exact(&mut length)?;
        Ok(u16::from_be_bytes(length))
    }

    fn decode_siz<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<ImageAndTileSizeMarkerSegment, Box<dyn error::Error>> {
        info!("SIZ start at byte offset {}", reader.stream_position()? - 2);
        let mut segment = ImageAndTileSizeMarkerSegment {
            offset: reader.stream_position()?,
            length: self.decode_length(reader)?,
            ..Default::default()
        };

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

        segment.precision = Vec::with_capacity(no_components as usize);
        segment.horizontal_separation = Vec::with_capacity(no_components as usize);
        segment.vertical_separation = Vec::with_capacity(no_components as usize);

        for _ in 0..no_components {
            // TODO: Consider putting into struct
            let mut precision = [0u8; 1];
            reader.read_exact(&mut precision)?;
            segment.precision.push(precision);

            let mut horizontal_separation = [0u8; 1];
            reader.read_exact(&mut horizontal_separation)?;
            segment.horizontal_separation.push(horizontal_separation);

            let mut vertical_separation = [0u8; 1];
            reader.read_exact(&mut vertical_separation)?;
            segment.vertical_separation.push(vertical_separation);
        }

        // The tile grid offsets (XTOsiz, YTOsiz) are constrained to be no
        // greater than the image area offsets. This is expressed by the
        // following ranges
        // 0 ≤ XTOsiz ≤ XOsiz
        // 0 ≤ YTOsiz ≤ YOsiz
        if segment.tile_horizontal_offset() > segment.image_horizontal_offset()
            || segment.tile_vertical_offset() > segment.image_vertical_offset()
        {
            return Err(CodestreamError::TileGridOffsetOverflow {
                tile_horizontal_offset: segment.tile_horizontal_offset(),
                image_horizontal_offset: segment.image_horizontal_offset(),
                tile_vertical_offset: segment.tile_vertical_offset(),
                image_vertical_offset: segment.image_vertical_offset(),
            }
            .into());
        }

        // Also, the tile size plus the tile offset shall be greater than the image
        // area offset. This ensures that the first tile (tile 0) will contain at least
        // one reference grid point from the image area. This is expressed by the
        // following ranges
        //
        // XTsiz + XTOsiz > XOsiz
        // YTsiz + YTOsiz > YOsiz
        if ((segment.reference_tile_width() + segment.tile_horizontal_offset())
            < segment.image_horizontal_offset())
            || ((segment.reference_tile_height() + segment.tile_vertical_offset())
                < segment.image_vertical_offset())
        {
            return Err(CodestreamError::TileSizeOverflow {
                reference_tile_width: segment.reference_tile_width(),
                tile_horizontal_offset: segment.tile_horizontal_offset(),
                image_horizontal_offset: segment.image_horizontal_offset(),
                reference_tile_height: segment.reference_tile_height(),
                tile_vertical_offset: segment.tile_vertical_offset(),
                image_vertical_offset: segment.image_vertical_offset(),
            }
            .into());
        }
        info!("SIZ end at byte offset {}", reader.stream_position()?);

        Ok(segment)
    }

    fn decode_cap<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<ExtendedCapabilitiesMarkerSegment, Box<dyn error::Error>> {
        log::info!("CAP start at byte offset {}", reader.stream_position()? - 2);
        let mut segment = ExtendedCapabilitiesMarkerSegment::default();

        // Lcap
        let mut marker_segment_length = [0u8; 2];
        reader.read_exact(&mut marker_segment_length)?;
        segment.length = u16::from_be_bytes(marker_segment_length);
        segment.capabilities = Vec::<Option<u16>>::with_capacity(32);

        // Pcap
        let mut capability_flags_present = [0u8; 4];
        reader.read_exact(&mut capability_flags_present)?;
        let pcap = u32::from_be_bytes(capability_flags_present);
        let num_capabilities = pcap.count_ones();
        if num_capabilities != ((segment.length - 6) / 2) as u32 {
            log::error!(
                "Marker length {} inconsistent with Pcap ones: {num_capabilities}",
                segment.length
            );
            return Err(CodestreamError::MarkerMalformed {
                marker: MARKER_SYMBOL_CAP.into(),
                offset: self.offset,
            }
            .into());
        }
        for i in 0..32 {
            let mask = 1u32 << (31 - i);
            if (pcap & mask) == mask {
                let mut ccap_i_bytes = [0u8; 2];
                reader.read_exact(&mut ccap_i_bytes)?;
                let ccap_i = u16::from_be_bytes(ccap_i_bytes);
                segment.capabilities.push(Some(ccap_i));
            } else {
                segment.capabilities.push(None);
            }
        }

        info!("CAP end at byte offset {}", reader.stream_position()?);

        Ok(segment)
    }

    fn decode_cpf<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<CorrespondingProfileMarkerSegment, Box<dyn error::Error>> {
        log::info!("CPF start at byte offset {}", reader.stream_position()? - 2);
        let mut segment = CorrespondingProfileMarkerSegment::default();

        // Lcpf
        let mut marker_segment_length_bytes = [0u8; 2];
        reader.read_exact(&mut marker_segment_length_bytes)?;
        segment.length = u16::from_be_bytes(marker_segment_length_bytes);

        // Pcpf
        let num_pfcp = (segment.length - 2) / 2;
        if num_pfcp > 1 {
            // Supporting more is possible, but we need a sanity check to prevent CPFnum overflow
            log::error!("Only a single Pcpf value is supported at this time");
            return Err(CodestreamError::UnsupportedFeature {
                marker: MARKER_SYMBOL_CPF.into(),
                offset: self.offset,
            }
            .into());
        }
        let mut pcpf_bytes = [0u8; 2];
        for _ in 0..num_pfcp {
            reader.read_exact(&mut pcpf_bytes)?;
            let pcpf = u16::from_be_bytes(pcpf_bytes);
            segment.pcpf.push(pcpf);
        }

        log::info!("CPF end at byte offset {}", reader.stream_position()?);

        Ok(segment)
    }

    fn decode_sot<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<StartOfTileSegment, Box<dyn error::Error>> {
        let offset = reader.stream_position()? - 2;
        info!("SOT start at byte offset {}", offset);

        let mut segment = StartOfTileSegment {
            offset,
            ..Default::default()
        };

        // LSot
        let mut marker_segment_length = [0u8; 2];
        reader.read_exact(&mut marker_segment_length)?;
        segment.length = u16::from_be_bytes(marker_segment_length);

        // ISot
        reader.read_exact(&mut segment.tile_index)?;

        // PSot
        segment.tile_length = {
            let mut b = [0u8; 4];
            reader.read_exact(&mut b)?;
            u32::from_be_bytes(b)
        };

        // TPSot
        reader.read_exact(&mut segment.tile_part_index)?;

        // TNSot
        reader.read_exact(&mut segment.no_tile_parts)?;

        info!("SOT end at byte offset {}", reader.stream_position()?);

        Ok(segment)
    }

    // A.6.1 - Coding style default (COD)
    fn decode_cod<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<CodingStyleMarkerSegment, Box<dyn error::Error>> {
        info!("COD start at byte offset {}", reader.stream_position()? - 2);
        let mut segment = CodingStyleMarkerSegment {
            offset: reader.stream_position()?,
            length: self.decode_length(reader)?,
            ..Default::default()
        };

        reader.read_exact(&mut segment.coding_style)?;
        reader.read_exact(&mut segment.progression_order)?;
        reader.read_exact(&mut segment.no_layers)?;
        reader.read_exact(&mut segment.multiple_component_transformation)?;

        self.decode_coding_style_parameters(
            reader,
            segment.coding_style[0],
            &mut segment.coding_style_parameters,
        )?;
        info!("COD end at byte offset {}", reader.stream_position()?);

        Ok(segment)
    }

    fn decode_coding_style_parameters<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
        coding_style: u8,
        coding_style_parameters: &mut CodingStyleParameters,
    ) -> Result<(), Box<dyn error::Error>> {
        coding_style_parameters.coding_style = [coding_style];

        reader.read_exact(&mut coding_style_parameters.no_decomposition_levels)?;
        reader.read_exact(&mut coding_style_parameters.code_block_width)?;
        reader.read_exact(&mut coding_style_parameters.code_block_height)?;
        reader.read_exact(&mut coding_style_parameters.code_block_style)?;
        reader.read_exact(&mut coding_style_parameters.transformation)?;

        if coding_style_parameters.has_defined_precinct_size() {
            // The first parameter (8 bits) corresponds to the N<sub>L</sub>LL sub-band.
            // Each successive parameter corresponds to each successive resolution level in order.
            coding_style_parameters.precinct_size =
                vec![0; coding_style_parameters.no_decomposition_levels() as usize + 1];
            reader.read_exact(&mut coding_style_parameters.precinct_size)?;
        }

        Ok(())
    }

    // TODO: Convert to usize/u16?
    fn decode_component_index<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
        no_components: u16,
    ) -> Result<[u8; 2], Box<dyn error::Error>> {
        // Either 8 or 16 bits depending on Csiz value.
        if no_components < 257 {
            let mut buffer = [0u8; 1];
            reader.read_exact(&mut buffer)?;
            Ok([0, buffer[0]])
        } else {
            // TODO: Understand why 2 MSB are unused (signness is only 1 bit)
            let mut buffer = [0u8; 2];
            reader.read_exact(&mut buffer)?;
            Ok(buffer)
        }
    }

    fn decode_coc<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
        no_components: u16,
    ) -> Result<CodingStyleComponentSegment, Box<dyn error::Error>> {
        info!("COC start at byte offset {}", reader.stream_position()? - 2);
        let mut segment = CodingStyleComponentSegment {
            offset: reader.stream_position()?,
            length: self.decode_length(reader)?,
            ..Default::default()
        };

        segment.index = self.decode_component_index(reader, no_components)?;

        reader.read_exact(&mut segment.coding_style)?;

        self.decode_coding_style_parameters(
            reader,
            segment.coding_style[0],
            &mut segment.coding_style_parameters,
        )?;
        info!("COC end at byte offset {}", reader.stream_position()?);

        Ok(segment)
    }

    fn decode_rgn<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
        no_components: u16,
    ) -> Result<RegionOfInterestSegment, Box<dyn error::Error>> {
        info!("RGN start at byte offset {}", reader.stream_position()? - 2);
        let mut segment = RegionOfInterestSegment {
            offset: reader.stream_position()?,
            length: self.decode_length(reader)?,
            ..Default::default()
        };

        segment.component_index = self.decode_component_index(reader, no_components)?;

        reader.read_exact(&mut segment.region_of_interest_style)?;
        reader.read_exact(&mut segment.region_of_interest_style_parameter)?;
        info!("RGN end at byte offset {}", reader.stream_position()?);

        Ok(segment)
    }

    fn decode_poc<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
        no_components: u16,
    ) -> Result<ProgressionOrderChangeSegment, Box<dyn error::Error>> {
        info!("POC start at byte offset {}", reader.stream_position()? - 2);
        let mut segment = ProgressionOrderChangeSegment {
            offset: reader.stream_position()?,
            length: self.decode_length(reader)?,
            ..Default::default()
        };

        // The number of progression changes can be derived from the length of the
        // marker segment. See Part 1 Equation A-6.
        let no_progression_order_change = match no_components < 257 {
            true => (segment.length - 2) / 7,
            false => (segment.length - 2) / 9,
        };

        segment.progressions = Vec::with_capacity(no_progression_order_change as usize);

        let mut index = 0;
        while index < no_progression_order_change {
            let mut progression = CodingStyleComponentSegmentProgression::default();

            reader.read_exact(&mut progression.resolution_level_index_start)?;

            progression.component_index_start =
                self.decode_component_index(reader, no_components)?;

            reader.read_exact(&mut progression.layer_index_end)?;

            reader.read_exact(&mut progression.resolution_level_index_end)?;

            progression.component_index_end = self.decode_component_index(reader, no_components)?;

            reader.read_exact(&mut progression.progression_order)?;

            segment.progressions.push(progression);

            index += 1;
        }
        info!("POC end at byte offset {}", reader.stream_position()?);

        Ok(segment)
    }

    fn decode_ppm<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<PackedPacketHeaderSegment, Box<dyn error::Error>> {
        info!("PPM start at byte offset {}", reader.stream_position()? - 2);
        let offset = reader.stream_position()?;
        let length = self.decode_length(reader)?;
        let mut segment = PackedPacketHeaderSegment {
            offset,
            length,
            index: [0],
            number_of_bytes: [0; 4],
            // TODO: It is possible that the next PPM marker segment will not
            // have an Nppm parameter after Zppm, but the continuation of the
            // Ippm series from the last PPM marker segment.
            data: vec![0; (length as usize) - 7],
        };

        reader.read_exact(&mut segment.index)?;
        reader.read_exact(&mut segment.number_of_bytes)?;
        reader.read_exact(&mut segment.data)?;
        info!("PPM end at byte offset {}", reader.stream_position()?);

        Ok(segment)
    }

    fn decode_ppt<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<TilePackedPacketHeaderSegment, Box<dyn error::Error>> {
        info!("PPT start at byte offset {}", reader.stream_position()? - 2);
        let offset = reader.stream_position()?;
        let length = self.decode_length(reader)?;
        let mut segment = TilePackedPacketHeaderSegment {
            offset,
            length,
            index: [0],
            data: vec![0; (length as usize) - 3],
        };

        reader.read_exact(&mut segment.index)?;
        reader.read_exact(&mut segment.data)?;

        info!("PPT end at byte offset {}", reader.stream_position()?);

        Ok(segment)
    }

    fn decode_tlm<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<TilePartLengthsSegment, Box<dyn error::Error>> {
        info!("TLM start at byte offset {}", reader.stream_position()? - 2);
        let mut segment = TilePartLengthsSegment {
            offset: reader.stream_position()?,
            length: self.decode_length(reader)?,
            ..Default::default()
        };
        reader.read_exact(&mut segment.index)?;
        reader.read_exact(&mut segment.parameter_sizes)?;

        let parameter_sizes = segment.parameter_sizes();

        let mut tile_part_size = 0;
        if parameter_sizes.contains(&TilePartParameterSize::Ttlm8Bit) {
            tile_part_size += 1;
        } else if parameter_sizes.contains(&TilePartParameterSize::Ttlm16Bit) {
            tile_part_size += 2;
        }
        if parameter_sizes.contains(&TilePartParameterSize::Ptlm16Bit) {
            tile_part_size += 2;
        } else if parameter_sizes.contains(&TilePartParameterSize::Ptlm32Bit) {
            tile_part_size += 4;
        }

        // number of tile lengths
        let no_tile_part_lengths = (segment.length - 4) / tile_part_size;

        for _ in 0..no_tile_part_lengths {
            let mut tile_part_length = TilePartLength::default();

            // Ttlm
            if parameter_sizes.contains(&TilePartParameterSize::Ttlm8Bit) {
                let mut buf = [0u8; 1];
                reader.read_exact(&mut buf)?;
                tile_part_length.tile_index = Some(buf[0] as u16);
            } else if parameter_sizes.contains(&TilePartParameterSize::Ttlm16Bit) {
                let mut buf = [0u8; 2];
                reader.read_exact(&mut buf)?;
                tile_part_length.tile_index = Some(u16::from_be_bytes(buf));
            } else {
                tile_part_length.tile_index = None;
            }

            // Ptlm
            if parameter_sizes.contains(&TilePartParameterSize::Ptlm16Bit) {
                let mut buf = [0u8; 2];
                reader.read_exact(&mut buf)?;
                tile_part_length.tile_length = u16::from_be_bytes(buf) as u32;
            } else if parameter_sizes.contains(&TilePartParameterSize::Ptlm32Bit) {
                let mut buf = [0u8; 4];
                reader.read_exact(&mut buf)?;
                tile_part_length.tile_length = u32::from_be_bytes(buf);
            }
            segment.tile_part_lengths.push(tile_part_length);
        }

        info!("TLM end at byte offset {}", reader.stream_position()?);
        Ok(segment)
    }

    fn decode_qcd<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<QuantizationDefaultMarkerSegment, Box<dyn error::Error>> {
        info!("QCD start at byte offset {}", reader.stream_position()? - 2);
        let length = self.decode_length(reader)?;
        let quantization_style = QuantizationInfo::decode(reader, length - 2)?;
        info!("QCD end at byte offset {}", reader.stream_position()?);

        Ok(QuantizationDefaultMarkerSegment {
            length,
            quantization_info: quantization_style,
        })
    }

    fn decode_qcc<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
        no_components: u16,
    ) -> Result<QuantizationComponentSegment, Box<dyn error::Error>> {
        info!("QCC start at byte offset {}", reader.stream_position()? - 2);
        let offset = reader.stream_position()?;
        let length = self.decode_length(reader)?;

        // Cqcc
        let component_index = self.decode_component_index(reader, no_components)?;

        let len_comp = if no_components < 257 { 1 } else { 2 };

        let quantization_info = QuantizationInfo::decode(reader, length - (2 + len_comp))?;

        info!("QCC end at byte offset {}", reader.stream_position()?);

        Ok(QuantizationComponentSegment {
            offset,
            length,
            component_index,
            quantization_info,
        })
    }

    fn decode_plm<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<PacketLengthSegment, Box<dyn error::Error>> {
        info!("PLM start at byte offset {}", reader.stream_position()? - 2);
        let mut segment = PacketLengthSegment {
            offset: reader.stream_position()?,
            length: self.decode_length(reader)?,
            ..Default::default()
        };

        reader.read_exact(&mut segment.index)?;
        reader.read_exact(&mut segment.no_bytes)?;

        segment.packet_length = Vec::with_capacity(segment.no_bytes() as usize);

        // TODO: Handle multiple PLM where the next PLM is missing
        // Nplm and is a continuation of previous Iplm
        self.decode_packet_length(reader, &mut segment.packet_length)?;

        info!("PLM end at byte offset {}", reader.stream_position()?);

        Ok(segment)
    }

    fn decode_packet_length<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
        vec: &mut Vec<u8>,
    ) -> Result<(), Box<dyn error::Error>> {
        let mut packet_length = [0u8; 1];
        loop {
            reader.read_exact(&mut packet_length)?;
            match packet_length[0] >> 7 {
                // 0xxx xxxx - Last 7 bits of packet length, terminate number
                0 => {
                    vec.push((packet_length[0] << 1) >> 1);
                    break;
                }
                // 1xxx xxxx - Continue reading
                _ => {
                    // These are not the last 7 bits that make up the packet
                    // length. Instead, these 7 bits are a portion of those that
                    // make up the packet length.
                    //
                    // The packet length has been broken into 7-bit segments
                    // which are sent in order from the most significant segment
                    // to the least significant segment.
                    //
                    // Furthermore, the bits in the most significant segment
                    // are right justified to the byte boundary.
                    vec.push((packet_length[0] << 1) >> 2);
                }
            }
        }
        Ok(())
    }

    fn decode_plt<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<TilePacketLength, Box<dyn error::Error>> {
        let plt_start_offset = reader.stream_position()? - 2;
        info!("PLT start at byte offset {}", plt_start_offset);
        let mut segment = TilePacketLength {
            offset: reader.stream_position()?,
            length: self.decode_length(reader)?,
            ..Default::default()
        };
        let end_offset = segment.offset + segment.length as u64;
        reader.read_exact(&mut segment.index)?;

        while reader.stream_position()? < end_offset {
            let iplt = Self::decode_tilepart_packet_length(reader).map_err(|_| {
                CodestreamError::MarkerMalformed {
                    marker: MARKER_SYMBOL_PLT.into(),
                    offset: plt_start_offset,
                }
            })?;
            segment.packet_length.push(iplt);
        }

        info!("PLT end at byte offset {}", reader.stream_position()?);

        Ok(segment)
    }

    fn decode_tilepart_packet_length<R: io::Read + io::Seek>(reader: &mut R) -> Result<u64, ()> {
        let mut next_byte = [0u8; 1];
        let mut result = 0u64;
        loop {
            reader.read_exact(&mut next_byte).map_err(|_| ())?;
            result = (result << 7) | ((next_byte[0] & 0b0111_1111) as u64);
            match next_byte[0] & 0b1000_0000 {
                // 0xxx xxxx - Last 7 bits of packet length, terminate number
                0b0000_0000 => {
                    return Ok(result);
                }
                // 1xxx xxxx - Continue reading
                _ => {
                    if result > u32::MAX as u64 {
                        return Err(());
                    }
                }
            }
        }
    }

    fn decode_crg<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
        no_components: u16,
    ) -> Result<ComponentRegistrationSegment, Box<dyn error::Error>> {
        info!("CRG start at byte offset {}", reader.stream_position()? - 2);
        let mut segment = ComponentRegistrationSegment {
            offset: reader.stream_position()?,
            length: self.decode_length(reader)?,
            ..Default::default()
        };

        segment.horizontal_offset = Vec::with_capacity(no_components as usize);
        segment.vertical_offset = Vec::with_capacity(no_components as usize);
        for _ in 0..no_components {
            // TODO: Consider putting into struct
            let mut horizontal_offset = [0u8; 2];
            reader.read_exact(&mut horizontal_offset)?;
            segment.horizontal_offset.push(horizontal_offset);

            let mut vertical_offset = [0u8; 2];
            reader.read_exact(&mut vertical_offset)?;
            segment.vertical_offset.push(vertical_offset);
        }
        info!("CRG end at byte offset {}", reader.stream_position()?);

        Ok(segment)
    }

    fn decode_com<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<CommentMarkerSegment, Box<dyn error::Error>> {
        info!("COM start at byte offset {}", reader.stream_position()? - 2);
        let mut segment = CommentMarkerSegment::default();

        // Length of marker segment in bytes (not including the marker).
        let mut marker_segment_length = [0u8; 2];
        reader.read_exact(&mut marker_segment_length)?;
        reader.read_exact(&mut segment.registration_value)?;

        let comment_length = u16::from_be_bytes(marker_segment_length) as usize
            - marker_segment_length.len()
            - segment.registration_value.len();

        segment.comment = vec![0; comment_length];

        reader.read_exact(&mut segment.comment)?;
        info!("COM end at byte offset {}", reader.stream_position()?);

        Ok(segment)
    }
}

#[derive(Debug, Default)]
pub struct Header {
    // SIZ (Required)
    image_and_tile_size_marker_segment: ImageAndTileSizeMarkerSegment,

    // CAP (Optional)
    extended_capabilities_marker_segment: Option<ExtendedCapabilitiesMarkerSegment>,

    // CPF (Optional)
    corresponding_profile_marker_segment: Option<CorrespondingProfileMarkerSegment>,

    // COD (Required)
    coding_style_marker_segment: Option<CodingStyleMarkerSegment>,

    // COC (Optional)
    coding_style_component_segment: Vec<CodingStyleComponentSegment>,

    // QCD (Required)
    quantization_default_marker_segment: Option<QuantizationDefaultMarkerSegment>,

    // QCC (Optional)
    quantization_component_segments: Vec<QuantizationComponentSegment>,

    // RGN (Optional)
    regions: Vec<RegionOfInterestSegment>,

    // POC (Optional)
    progression_order_change: Option<ProgressionOrderChangeSegment>,

    // PPM (Optional)
    packed_packet_headers: Vec<PackedPacketHeaderSegment>,

    // TLM (Optional)
    tile_part_lengths: Vec<TilePartLengthsSegment>,

    // PLM (Optional)
    packet_lengths: Vec<PacketLengthSegment>,

    // CRG (Optional)
    component_registration: Option<ComponentRegistrationSegment>,

    // COM (Optional, repeatable)
    comment_marker_segments: Vec<CommentMarkerSegment>,
}

impl Header {
    pub fn image_and_tile_size_marker_segment(&self) -> &ImageAndTileSizeMarkerSegment {
        &self.image_and_tile_size_marker_segment
    }

    /// Extended capabilities (CAP) marker segment.
    ///
    /// Signals that extended capabilities were used to create (and are recommended or required to decode) a codestream.
    ///
    /// This segment is optional. The second-most-significant bit in Rsiz may optionally be set to 1 to indicate the
    /// presence of the CAP marker segment.
    ///
    /// See ITU-T T.800(V4) or ISO/IEC 15444-1:2024 Section A.5.2 for how this works.
    pub fn extended_capabilities_marker_segment(
        &self,
    ) -> &Option<ExtendedCapabilitiesMarkerSegment> {
        &self.extended_capabilities_marker_segment
    }

    /// Corresponding profile (CPF) segment.
    ///
    /// Supports reversible transcoding of HTJ2K codestreams to and from Part 1 codestreams.
    ///
    /// This segment is optional.
    ///
    /// See ITU-T T.814(06/2019) or ISO/IEC 15444-15:2019 Section A.6 for how this works.
    pub fn corresponding_profile_marker_segment(
        &self,
    ) -> &Option<CorrespondingProfileMarkerSegment> {
        &self.corresponding_profile_marker_segment
    }

    pub fn coding_style_marker_segment(&self) -> &CodingStyleMarkerSegment {
        self.coding_style_marker_segment.as_ref().unwrap()
    }

    /// Coding style component (COC) segment
    ///
    /// Describes the coding style and number of decomposition levels for compressing
    /// a particular component. If present, the values in these segments overrides the
    /// COD coding style for a specific component. These values can in turn be overridden
    /// for specific tile parts.
    ///
    /// See ITU-T T.800 or ISO/IEC 15444-1:2019 Section A.6.2 for how this works.
    pub fn coding_style_component_segment(&self) -> &Vec<CodingStyleComponentSegment> {
        &self.coding_style_component_segment
    }

    pub fn quantization_default_marker_segment(&self) -> &QuantizationDefaultMarkerSegment {
        self.quantization_default_marker_segment.as_ref().unwrap()
    }

    // Quantization component (QCC) segments
    ///
    /// Describes the quantization used for compressing a particular component.
    /// If present, the values in these segments overrides the
    /// QCD quantization for a specific component. These values can in turn be overridden
    /// for specific tile parts.
    ///
    /// See ITU-T T.800 or ISO/IEC 15444-1:2019 Section A.6.5 for how this works.
    pub fn quantization_component_segments(&self) -> &Vec<QuantizationComponentSegment> {
        &self.quantization_component_segments
    }

    /// Region of interest (RGN) segments
    ///
    /// Signals the presence of an ROI in the codestream.
    ///
    /// See ITU-T T.800 or ISO/IEC 15444-1:2019 Section A.6.3 for how this works.
    pub fn region_of_interest_segments(&self) -> &Vec<RegionOfInterestSegment> {
        &self.regions
    }

    /// Progression order change (POC) segment
    ///
    /// Describes the bounds and progression order for any progression order than that
    /// specified in the COD marker segments. If present, the values in this segment override
    /// the progression order specified in COD. These values can in turn be overridden for
    /// specific tile parts.
    ///
    /// See ITU-T T.800 or ISO/IEC 15444-1:2019 Section A.6.6 for how this works.
    pub fn progression_order_change_segment(&self) -> &Option<ProgressionOrderChangeSegment> {
        &self.progression_order_change
    }

    /// Tile-part lengths (TLM) segment.
    ///
    /// Describes the length of every tile-part in the codestream.
    ///
    /// There can be multiple TLM segments. There is an index in the segment that
    /// identifies the order of the TLM segments relative to each other.
    ///
    /// See ITU-T T.800(V4) or ISO/IEC 15444-1:2024 Section A.7.1 for how this works.
    pub fn tile_part_lengths_segments(&self) -> &Vec<TilePartLengthsSegment> {
        // TODO: should we return them in sorted order?
        &self.tile_part_lengths
    }

    /// Packet length, main header (PLM) segments
    ///
    /// A list of packet lengths fin the tile-parts for every tile-part in order.
    ///
    /// See ITU-T T.800 or ISO/IEC 15444-1:2019 Section A.7.2 for how this works.
    pub fn packet_lengths_segments(&self) -> &Vec<PacketLengthSegment> {
        &self.packet_lengths
    }

    /// Packed packet headers, main header (PPM) segments
    ///
    /// A collection of the packet headers from all tiles.
    ///
    /// See ITU-T T.800 or ISO/IEC 15444-1:2019 Section A.7.4 for how this works.
    pub fn packed_packet_headers_segments(&self) -> &Vec<PackedPacketHeaderSegment> {
        &self.packed_packet_headers
    }

    /// Component registration (CRG) segment
    ///
    /// Allows specific registration of components with respect to each other.
    ///
    /// See ITU-T T.800 or ISO/IEC 15444-1:2019 Section A.9.1 for how this works.
    pub fn component_registration_segment(&self) -> &Option<ComponentRegistrationSegment> {
        &self.component_registration
    }

    pub fn comment_marker_segments(&self) -> &Vec<CommentMarkerSegment> {
        &self.comment_marker_segments
    }
}

// Many images have multiple components. This specification has a multiple component transformation to decorrelate threecomponents. This is the only function in this specification that relates components to each other
struct Image {}

// The image components may be divided into tiles.
//
// These tile-components are rectangular arrays that relate to the same portion
// of each of the components that make up the image.
//
// Thus, tiling of the image actually creates tile-components that can be
// extracted or decoded independently of each other.
//
// This tile independence provides one of the methods for extracting a region
// of the image
//
//
// TODO: Move
// The tile-components are decomposed into different decomposition levels using
// a wavelet transformation. These decomposition levels contain a number of
// subbands populated with coefficients that describe the horizontal and
// vertical spatial frequency characteristics of the original tile-components.
//
// The coefficients provide frequency information about a local area, rather
// than across the entire image like the Fourier transformation. That is, a
// small number of coefficients completely describe a single sample.
//
// A decomposition level is related to the next decomposition level by a
// spatial factor of two. That is, each successive decomposition level of the
// subbands has approximately half the horizontal and half the vertical
// resolution of the previous.
//
// Images of lower resolution than the original are generated by decoding a
// selected subset of these subbands.
#[derive(Debug, Default)]
struct Tile {}

/// A codestream is divided into tile-parts.
#[derive(Debug)]
struct TilePart {
    header: TilePartHeader,
    data_offset: u64,
}

/// A tile part header. Required for every tile part in the codestream. Contains the information
/// specific to the tile-part for decoding.
///
/// See ITU T.800 | ISO/IEC 15444-1 Figures A.4 and A.5
#[derive(Debug)]
struct TilePartHeader {
    // SOT (Required)
    start_of_tile_segment: StartOfTileSegment,

    first_headers: Option<FirstTilePartHeaders>,

    // POC (Optional, unless POC differ from main POC then Required)
    progression_order_change: Option<ProgressionOrderChangeSegment>,

    // PPT (Optional)
    packed_packet_headers: Option<TilePackedPacketHeaderSegment>,

    // PLT (Optional)
    // TODO double check there is only one per tile-part
    packet_lengths: Option<TilePacketLength>,

    // COM (Optional, repeatable)
    comment_marker_segments: Vec<CommentMarkerSegment>,
}

impl TilePartHeader {
    fn new(start_of_tile_segment: StartOfTileSegment) -> Self {
        Self {
            start_of_tile_segment,
            first_headers: None,
            progression_order_change: None,
            packed_packet_headers: None,
            packet_lengths: None,
            comment_marker_segments: Vec::new(),
        }
    }

    fn first_headers(&mut self) -> Result<&mut FirstTilePartHeaders, CodestreamError> {
        self.first_headers
            .as_mut()
            .ok_or(CodestreamError::InputFormatError {
                error: String::from(
                    "Some tile-part headers are only applicable to the first tile-part for a tile.",
                ),
            })
    }
}

/// Tile-part headers that are only allowed in the first tile-part for a given tile.
#[derive(Debug, Default)]
struct FirstTilePartHeaders {
    /// COD (Optional per tile)
    coding_style_marker_segment: Option<CodingStyleMarkerSegment>,

    /// COC (Optional per component)
    coding_style_component_segment: Vec<CodingStyleComponentSegment>,

    /// QCD (Optional per tile)
    quantization_default_marker_segment: Option<QuantizationDefaultMarkerSegment>,

    /// QCC (Optional per component)
    quantization_component_segment: Vec<QuantizationComponentSegment>,

    /// RGN (Optional per component)
    regions: Vec<RegionOfInterestSegment>,
}

impl ContiguousCodestream {
    pub fn length(&self) -> u16 {
        self.length
    }

    pub fn offset(&self) -> u64 {
        self.offset
    }

    // A.3 - Construction of the main header
    fn decode_main_header<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<Header, Box<dyn error::Error>> {
        let mut header = Header::default();

        let mut marker_type = MarkerSymbol::decode(reader)?;

        // SOC (Required as the first marker)
        if marker_type != MARKER_SYMBOL_SOC {
            return Err(CodestreamError::MarkerUnexpected {
                actual_marker: marker_type.into(),
                expected_marker: MARKER_SYMBOL_SOC.into(),
                offset: reader.stream_position()? - 2,
            }
            .into());
        }
        info!("SOC start at byte offset {}", reader.stream_position()? - 2);

        // SIZ (Required as the second marker segment)
        marker_type = MarkerSymbol::decode(reader)?;
        if marker_type != MARKER_SYMBOL_SIZ {
            return Err(CodestreamError::MarkerUnexpected {
                actual_marker: marker_type.into(),
                expected_marker: MARKER_SYMBOL_SIZ.into(),
                offset: reader.stream_position()? - 2,
            }
            .into());
        }

        header.image_and_tile_size_marker_segment = self.decode_siz(reader)?;

        let no_components = header.image_and_tile_size_marker_segment.no_components();

        loop {
            match MarkerSymbol::decode(reader) {
                Ok(marker_type) => match marker_type {
                    // COC (Optional, no more than one COC per component)
                    MARKER_SYMBOL_COC => {
                        header
                            .coding_style_component_segment
                            .push(self.decode_coc(reader, no_components)?);
                    }
                    // QCD (Required)
                    MARKER_SYMBOL_QCD => {
                        header.quantization_default_marker_segment = Some(self.decode_qcd(reader)?);
                    }

                    // COD (Required)
                    MARKER_SYMBOL_COD => {
                        header.coding_style_marker_segment = Some(self.decode_cod(reader)?);
                    }

                    // QCC (Optional, no more than one QCC per component)
                    MARKER_SYMBOL_QCC => {
                        header
                            .quantization_component_segments
                            .push(self.decode_qcc(reader, no_components)?);
                    }

                    // RGN (Optional, no more than one RGN per component)
                    MARKER_SYMBOL_RGN => {
                        header.regions.push(self.decode_rgn(reader, no_components)?);
                    }

                    // POC (Required in main or tile for any progression order changes)
                    MARKER_SYMBOL_POC => {
                        header.progression_order_change =
                            Some(self.decode_poc(reader, no_components)?);
                    }

                    // PPM (Optional, either PPM or PPT or codestream packet headers required)
                    MARKER_SYMBOL_PPM => {
                        // TODO: If the PPM marker segment is present, all the packet headers shall be found in the
                        // main header.
                        header.packed_packet_headers.push(self.decode_ppm(reader)?);
                    }

                    // TLM (Optional, repeatable)
                    MARKER_SYMBOL_TLM => {
                        header.tile_part_lengths.push(self.decode_tlm(reader)?);
                    }

                    // PLM (Optional)
                    MARKER_SYMBOL_PLM => {
                        let packet_length = self.decode_plm(reader)?;
                        header.packet_lengths.push(packet_length);
                    }

                    // CRG (Optional)
                    MARKER_SYMBOL_CRG => {
                        header.component_registration =
                            Some(self.decode_crg(reader, no_components)?);
                    }

                    // COM (Optional, repeatable)
                    MARKER_SYMBOL_COM => {
                        let comment_marker_segment = self.decode_com(reader)?;
                        header.comment_marker_segments.push(comment_marker_segment);
                    }

                    // CAP (Optional)
                    // TODO: in strict mode, ensure this is the first marker segment after SIZ
                    MARKER_SYMBOL_CAP => {
                        header.extended_capabilities_marker_segment =
                            Some(self.decode_cap(reader)?);
                    }

                    // CPF (Optional)
                    // From ITU-T T.814 | ISO/IEC 15444-15
                    MARKER_SYMBOL_CPF => {
                        header.corresponding_profile_marker_segment =
                            Some(self.decode_cpf(reader)?);
                    }

                    // Start of tile bit-stream
                    MARKER_SYMBOL_SOT => {
                        reader.seek(io::SeekFrom::Current(-2))?;
                        break;
                    }

                    // Reserved markers
                    // ITU-T H.800 or ISO/IEC 15444-1 2024, Section A.1.3 and Table A.1
                    MarkerSymbol([0xff, 0x30])
                    | MarkerSymbol([0xff, 0x31])
                    | MarkerSymbol([0xff, 0x32])
                    | MarkerSymbol([0xff, 0x33])
                    | MarkerSymbol([0xff, 0x34])
                    | MarkerSymbol([0xff, 0x35])
                    | MarkerSymbol([0xff, 0x36])
                    | MarkerSymbol([0xff, 0x37])
                    | MarkerSymbol([0xff, 0x38])
                    | MarkerSymbol([0xff, 0x39])
                    | MarkerSymbol([0xff, 0x3A])
                    | MarkerSymbol([0xff, 0x3B])
                    | MarkerSymbol([0xff, 0x3C])
                    | MarkerSymbol([0xff, 0x3D])
                    | MarkerSymbol([0xff, 0x3E])
                    | MarkerSymbol([0xff, 0x3F]) => {
                        // Reserved as marker only, not a segment
                        info!("Skipping marker: {:?}", marker_type);
                    }

                    _ => {
                        log::error!("unknown marker type: {marker_type:?}");
                        return Err(CodestreamError::MarkerUnknown {
                            marker: marker_type.into(),
                            offset: reader.stream_position()? - 2,
                        }
                        .into());
                    }
                },
                Err(e) => return Err(e.into()),
            }
        }

        // Required
        if header.quantization_default_marker_segment.is_none() {
            return Err(CodestreamError::MarkerMissing {
                marker: MARKER_SYMBOL_QCD.into(),
            }
            .into());
        }
        if header.coding_style_marker_segment.is_none() {
            return Err(CodestreamError::MarkerMissing {
                marker: MARKER_SYMBOL_COD.into(),
            }
            .into());
        }

        // A.6.2
        // No more than one per any given component may be present in either the main or tile-part headers
        if header.coding_style_component_segment.len() > (no_components as usize) {
            return Err(CodestreamError::MarkerError {
                marker: MARKER_SYMBOL_COC.into(),
                error: format!(
                    "number of coding style component (COC) {:?} exceeds number of components {:?}",
                    header.regions.len(),
                    no_components
                ),
            }
            .into());
        }

        // A.6.3 - here may be at most one
        // There may be at most one RGN marker segment for each component in either the main or tile-part headers
        if header.regions.len() > (no_components as usize) {
            return Err(CodestreamError::MarkerError {
                marker: MARKER_SYMBOL_RGN.into(),
                error: format!(
                    "number of region of interest (RGN) {:?} exceeds number of components {:?}",
                    header.regions.len(),
                    no_components
                ),
            }
            .into());
        }

        // A.6.5
        // No more than one per any given component may be present in either the main or tile-part headers
        if header.quantization_component_segments.len() > (no_components as usize) {
            return Err(CodestreamError::MarkerError {
                marker: MARKER_SYMBOL_QCC.into(),
                error: format!(
                    "number of quantization component (QCC) {:?} exceeds number of components {:?}",
                    header.regions.len(),
                    no_components
                ),
            }
            .into());
        }

        Ok(header)
    }

    /// Decode a tile-part header
    ///
    /// See ITU T.800 | ISO/IEC 15444-1 Figures A.4 and A.5
    fn decode_tile_part<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<TilePart, Box<dyn error::Error>> {
        // start of tile is always first
        let start_of_tile_segment = self.decode_sot(reader)?;
        let mut header = TilePartHeader::new(start_of_tile_segment);

        // todo determine if first header
        if let _tiles_first_tile_part = true {
            header.first_headers = Some(FirstTilePartHeaders::default())
        }

        let no_components = self
            .header
            .image_and_tile_size_marker_segment
            .no_components();

        loop {
            let pos = reader.stream_position()?;
            match MarkerSymbol::decode(reader)? {
                // COD (Optional)
                MARKER_SYMBOL_COD => {
                    header.first_headers()?.coding_style_marker_segment =
                        Some(self.decode_cod(reader)?);
                    let cod = self.decode_cod(reader)?;
                    let prev = header
                        .first_headers()?
                        .coding_style_marker_segment
                        .replace(cod);
                    if prev.is_some() {
                        return Err(CodestreamError::MarkerDisallowed {
                            marker: MARKER_SYMBOL_COD.into(),
                            offset: pos,
                        }
                        .into());
                    }
                }

                // COC (Optional)
                MARKER_SYMBOL_COC => {
                    // TODO check that there is only a single COC per component
                    let coc = self.decode_coc(reader, no_components)?;
                    header.first_headers()?.coding_style_component_segment = vec![coc];
                }

                // QCD (Optional)
                MARKER_SYMBOL_QCD => {
                    let qcd = self.decode_qcd(reader)?;
                    let prev = header
                        .first_headers()?
                        .quantization_default_marker_segment
                        .replace(qcd);
                    if prev.is_some() {
                        return Err(CodestreamError::MarkerDisallowed {
                            marker: MARKER_SYMBOL_QCD.into(),
                            offset: pos,
                        }
                        .into());
                    }
                }

                // QCC (Optional)
                MARKER_SYMBOL_QCC => {
                    // TODO check that there is only a single QCC per component
                    let qcc = self.decode_qcc(reader, no_components)?;
                    header
                        .first_headers()?
                        .quantization_component_segment
                        .push(qcc);
                }

                // RGN (Optional)
                MARKER_SYMBOL_RGN => {
                    header
                        .first_headers()?
                        .regions
                        .push(self.decode_rgn(reader, no_components)?);
                }

                // POC (Optional)
                MARKER_SYMBOL_POC => {
                    header.progression_order_change = Some(self.decode_poc(reader, no_components)?);
                }

                // PPT (Optional)
                MARKER_SYMBOL_PPT => {
                    // The packet headers shall be in only one of three places within the codestream. If the PPM
                    // marker segment is present, all the packet headers shall be found in the main header.
                    //
                    // In this case, the PPT marker segment and packets distributed in the bit stream of the
                    // tile-parts are disallowed.
                    if !self.header.packed_packet_headers.is_empty() {
                        return Err(CodestreamError::MarkerDisallowed {
                            marker: MARKER_SYMBOL_PPT.into(),
                            offset: reader.stream_position()? - 2,
                        }
                        .into());
                    }

                    header.packed_packet_headers = Some(self.decode_ppt(reader)?);
                }

                // PLT (Optional)
                MARKER_SYMBOL_PLT => {
                    let packet_length_segment = self.decode_plt(reader)?;
                    header.packet_lengths = Some(packet_length_segment);
                }

                // COM (Optional, repeatable)
                MARKER_SYMBOL_COM => {
                    header
                        .comment_marker_segments
                        .push(self.decode_com(reader)?);
                }

                // SOD
                MARKER_SYMBOL_SOD => {
                    // Always last
                    break;
                }
                marker_type => {
                    log::error!("unexpected marker type: {marker_type:?}");
                    return Err(CodestreamError::MarkerUnknown {
                        marker: marker_type.into(),
                        offset: reader.stream_position()? - 2,
                    }
                    .into());
                }
            }
        }

        // Should have just seen the SOD marker
        let data_offset = reader.stream_position()?;
        let sot_offset = header.start_of_tile_segment.offset;
        let data_end = sot_offset + header.start_of_tile_segment.tile_length as u64;

        // Seek past data, TODO read data
        reader.seek(io::SeekFrom::Start(data_end))?;
        Ok(TilePart {
            header,
            data_offset,
        })
    }

    fn decode<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<(), Box<dyn error::Error>> {
        // The main header is found at the beginning of the codestream
        self.header = self.decode_main_header(reader)?;

        // Grab tile-parts from stream
        loop {
            match MarkerSymbol::decode(reader)? {
                MARKER_SYMBOL_SOT => {
                    info!("Handle tile-part. SOT");
                    let tile_part = self.decode_tile_part(reader)?;
                    self.tile_parts.push(tile_part);
                }
                MARKER_SYMBOL_EOC => {
                    // No more tile-parts, proper EOC end
                    return Ok(());
                }
                marker_type => {
                    error!("Marker {marker_type}");
                    return Err(CodestreamError::MarkerUnexpected {
                        actual_marker: marker_type.into(),
                        expected_marker: MARKER_SYMBOL_SOT.into(),
                        offset: reader.stream_position()?,
                    }
                    .into());
                }
            }
        }
    }
}

// All components are defined with respect to the reference grid.
//
// The reference grid is a rectangular grid of points with the indices from
// (0, 0) to (Xsiz-1, Ysiz-1).
//
// Each component domain is a sub-sampled version of the reference grid with
// the (0, 0) coordinate as common point for each component
//
// Samples
// The samples of component c are at integer multiples of (XRsiz^c, YRsiz^c) on
// the reference grid.
//
// Row samples are located reference grid points that are at integer multiples
// of XRsiz^c and column samples are located reference grid points that are at
// integer multiples of YRsiz^c
//
// Only those samples which fall within the image area actually belong to the
// image component. Thus, the samples of component c are mapped to rectangle
// having upper left hand sample with coordinates (x0, y0) and lower right hand
// sample with coordinates (x1-1, y1-1), where
// x0 = [XOsiz / XRsiz^c]
// x1 = [Xsiz / XRsiz^c]
// y0 = [YOsiz / YRsiz^c]
// y1 = [Ysiz / YRsiz^c]
//
// Thus, the dimensions of component c are given by
// (width, height) = (x1 - x0, y1 - y0)
//
// The parameters, Ysiz, Ysiz, YOsiz, YOsiz, YRsiz^c and YRsiz^c are all
// defined in the SIZ marker segment
struct Component {}

// An “image area” is defined on the reference grid by the dimensional
// parameters, (Xsiz, Ysiz) and (XOsiz, YOsiz).
//
// Specifically, the image area on the reference grid is defined by its upper
// left hand reference grid point at location (XOsiz, YOsiz), and its lower
// right hand reference grid point at location (Xsiz-1, Ysiz-1).
struct ImageArea {}

pub fn decode_jpc<R: io::Read + io::Seek>(
    reader: &mut R,
) -> Result<ContiguousCodestream, Box<dyn error::Error>> {
    let mut continuous_codestream = ContiguousCodestream::default();
    continuous_codestream.decode(reader)?;

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
    // For the last decomposition level the LL subband is also included.

    Ok(continuous_codestream)
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Seek};

    use super::*;

    #[test]
    fn test_cap_marker_debug() {
        let marker = MARKER_SYMBOL_CAP;
        assert_eq!(format!("{marker:#?}"), "0xFF50");
    }

    #[test]
    fn test_cap_marker_format() {
        let marker = MARKER_SYMBOL_CAP;
        assert_eq!(format!("{marker}"), "CAP (0xFF50)");
    }

    #[test]
    fn test_soc_marker_debug() {
        let marker = MARKER_SYMBOL_SOC;
        assert_eq!(format!("{marker:#?}"), "0xFF4F");
    }

    #[test]
    fn test_soc_marker_format() {
        let marker = MARKER_SYMBOL_SOC;
        assert_eq!(format!("{marker}"), "SOC (0xFF4F)");
    }

    #[test]
    fn test_com_marker_debug() {
        let marker = MARKER_SYMBOL_COM;
        assert_eq!(format!("{marker:#?}"), "0xFF64");
    }

    #[test]
    fn test_com_marker_format() {
        let marker = MARKER_SYMBOL_COM;
        assert_eq!(format!("{marker}"), "COM (0xFF64)");
    }

    #[test]
    fn test_unknown_marker_debug() {
        let marker = MarkerSymbol([0xFE, 0xFD]);
        assert_eq!(format!("{marker:#?}"), "0xFEFD");
    }

    #[test]
    fn test_unknown_marker_format() {
        let marker = MarkerSymbol([0xFE, 0xFD]);
        assert_eq!(format!("{marker}"), "Unknown Marker (0xFEFD)");
    }

    #[test]
    fn test_codestream_error_marker_error() {
        let e = CodestreamError::MarkerError {
            marker: MARKER_SYMBOL_COC.into(),
            error: "test error".into(),
        };
        assert_eq!(format!("{e}"), "marker COC (0xFF53) error \"test error\"");
    }

    #[test]
    fn test_codestream_error_marker_malformed() {
        let e = CodestreamError::MarkerMalformed {
            marker: MARKER_SYMBOL_POC.into(),
            offset: 3,
        };
        assert_eq!(
            format!("{e}"),
            "malformed marker POC (0xFF5F) at byte offset 3"
        );
    }

    #[test]
    fn test_codestream_error_missing_marker() {
        let e = CodestreamError::MarkerMissing {
            marker: MARKER_SYMBOL_SIZ.into(),
        };
        assert_eq!(format!("{e}"), "missing marker SIZ (0xFF51)");
    }

    #[test]
    fn test_codestream_error_marker_disallowed() {
        let e = CodestreamError::MarkerDisallowed {
            marker: MARKER_SYMBOL_PPT.into(),
            offset: 6136,
        };
        assert_eq!(
            format!("{e}"),
            "disallowed marker PPT (0xFF61) at byte offset 6136"
        );
    }

    #[test]
    fn test_codestream_error_unexpected_marker() {
        let e = CodestreamError::MarkerUnexpected {
            actual_marker: MARKER_SYMBOL_SOP.into(),
            expected_marker: MARKER_SYMBOL_SOT.into(),
            offset: 75453,
        };
        assert_eq!(
            format!("{e}"),
            "unexpected marker SOP (0xFF91) expected SOT (0xFF90) at byte offset 75453"
        );
    }

    #[test]
    fn test_codestream_error_tilesize_overflow() {
        let e = CodestreamError::TileSizeOverflow {
            image_horizontal_offset: 1,
            image_vertical_offset: 2,
            tile_horizontal_offset: 3,
            tile_vertical_offset: 4,
            reference_tile_width: 5,
            reference_tile_height: 6,
        };
        assert_eq!(format!("{e}"), "tile size overflow: XOSiz = 1, YOsiz = 2, XTOsiz = 3, YTOsiz = 4, XTsize = 5, YTsize = 6");
    }

    #[test]
    fn test_codestream_error_tile_grid_offset_overflow() {
        let e = CodestreamError::TileGridOffsetOverflow {
            tile_horizontal_offset: 1,
            tile_vertical_offset: 2,
            image_horizontal_offset: 3,
            image_vertical_offset: 4,
        };
        assert_eq!(
            format!("{e}"),
            "tile grid offset overflow: XOSiz = 3, YOsiz = 4, XTOsiz = 1, YTOsiz = 2"
        );
    }

    #[test]
    fn test_codestream_error_unsupported_feature() {
        let e = CodestreamError::UnsupportedFeature {
            marker: MARKER_SYMBOL_PLT.into(),
            offset: 3425,
        };
        assert_eq!(
            format!("{e}"),
            "unsupported feature for marker PLT (0xFF58) at byte offset 3425"
        );
    }

    #[test]
    fn test_codestream_error_marker_unknown() {
        let e = CodestreamError::MarkerUnknown {
            marker: MARKER_SYMBOL_PLT.into(),
            offset: 3425,
        };
        assert_eq!(
            format!("{e}"),
            "unknown marker PLT (0xFF58) at byte offset 3425"
        );
    }

    #[test]
    fn test_decode_qcd() {
        {
            // Test no quant style
            let bytes = [
                0xff, 0x5c, 0, 0xD, 0xE0, 0x40, 0x48, 0x48, 0x50, 0x48, 0x48, 0x50, 0x48, 0x48,
                0x50,
            ];
            let mut cursor = Cursor::new(&bytes);
            cursor.seek_relative(2).unwrap(); // skip marker
            let mut continuous_codestream = ContiguousCodestream::default();
            let res = continuous_codestream.decode_qcd(&mut cursor).unwrap();
            assert_eq!(res.length, 13);

            let quant_info = res.quantization_info();
            assert_eq!(quant_info.guard_bits, 7);
            assert_eq!(quant_info.style, QuantizationStyle::NoQuantization);
            assert_eq!(
                quant_info.values(),
                vec![0x40, 0x48, 0x48, 0x50, 0x48, 0x48, 0x50, 0x48, 0x48, 0x50,]
            );
            assert_eq!(
                quant_info.exponents(),
                vec![8, 9, 9, 10, 9, 9, 10, 9, 9, 10]
            );
        }
        {
            // Test scalar derived style
            let bytes = [0xff, 0x5c, 0, 0x5, 0x41, 0xF8, 0x01, 0xff, 0x5d];
            let mut cursor = Cursor::new(&bytes);
            cursor.seek_relative(2).unwrap(); // skip marker
            let mut continuous_codestream = ContiguousCodestream::default();
            let res = continuous_codestream.decode_qcd(&mut cursor).unwrap();
            assert_eq!(res.length, 5);

            let quant_info = res.quantization_info();
            assert_eq!(quant_info.guard_bits, 2);
            assert_eq!(quant_info.style, QuantizationStyle::ScalarDerived);
            assert_eq!(quant_info.values(), vec![0xF801]);
            assert_eq!(quant_info.exponents(), vec![0x1F]);
        }
        {
            // Test scalar expounded
            let bytes = [
                0xff, 0x5c, 0, 0xB, 0x82, 0x40, 0x01, 0x48, 0x02, 0x48, 0x03, 0x50, 0x04,
            ];
            let mut cursor = Cursor::new(&bytes);
            cursor.seek_relative(2).unwrap(); // skip marker
            let mut continuous_codestream = ContiguousCodestream::default();
            let res = continuous_codestream.decode_qcd(&mut cursor).unwrap();
            assert_eq!(res.length, 11);

            let quant_info = res.quantization_info();
            assert_eq!(quant_info.guard_bits, 4);
            assert_eq!(quant_info.style, QuantizationStyle::ScalarExpounded);
            assert_eq!(quant_info.values(), vec![0x4001, 0x4802, 0x4803, 0x5004]);
            assert_eq!(quant_info.exponents(), vec![8, 9, 9, 10]);
        }
    }

    #[test]
    fn test_plt_decode_0() {
        let bytes = [
            0xff, 0x58, 0x00, 0x63, 0x00, 0x81, 0x10, 0x81, 0x29, 0x81, 0x1f, 0x81, 0x44, 0x81,
            0x17, 0x81, 0x02, 0x2e, 0x7a, 0x1e, 0x2c, 0x2b, 0x2a, 0x65, 0x57, 0x40, 0x82, 0x53,
            0x34, 0x18, 0x2c, 0x2b, 0x2c, 0x4b, 0x69, 0x58, 0x2c, 0x57, 0x33, 0x09, 0x0b, 0x09,
            0x70, 0x2f, 0x66, 0x81, 0x1a, 0x81, 0x18, 0x5d, 0x2a, 0x2a, 0x2b, 0x56, 0x79, 0x59,
            0x82, 0x09, 0x82, 0x3c, 0x81, 0x6e, 0x4d, 0x4e, 0x4e, 0x56, 0x76, 0x70, 0x83, 0x1c,
            0x82, 0x7a, 0x82, 0x2f, 0x09, 0x09, 0x09, 0x81, 0x32, 0x81, 0x31, 0x81, 0x2f, 0x83,
            0x7b, 0x82, 0x77, 0x84, 0x03, 0x0a, 0x0a, 0x0a, 0x32, 0x31, 0x30, 0x84, 0x22, 0x84,
            0x5b, 0x83, 0x40,
        ];

        let mut reader = Cursor::new(&bytes);
        reader.seek_relative(2).unwrap(); // skip marker
        let mut codestream = ContiguousCodestream::default();
        let res = codestream.decode_plt(&mut reader);
        assert!(res.is_ok());
        let plt = res.unwrap();
        assert_eq!(plt.length, 99);
        assert_eq!(plt.index[0], 0);
        // Iplt as reported by jpylyzer
        assert_eq!(
            plt.packet_length,
            vec![
                0x0090, 0x00A9, 0x009F, 0x00C4, 0x0097, 0x0082, 0x2E, 0x7A, 0x1E, 0x2C, 0x2B, 0x2A,
                0x65, 0x57, 0x40, 0x0153, 0x34, 0x18, 0x2C, 0x2B, 0x2C, 0x4B, 0x69, 0x58, 0x2C,
                0x57, 0x33, 0x09, 0x0B, 0x09, 0x70, 0x2F, 0x66, 0x009A, 0x0098, 0x5D, 0x2A, 0x2A,
                0x2B, 0x56, 0x79, 0x59, 0x0109, 0x013C, 0x00EE, 0x4D, 0x4E, 0x4E, 0x56, 0x76, 0x70,
                0x019C, 0x017A, 0x012F, 0x09, 0x09, 0x09, 0x00B2, 0x00B1, 0x00AF, 0x01FB, 0x0177,
                0x0203, 0x0A, 0x0A, 0x0A, 0x32, 0x31, 0x30, 0x0222, 0x025B, 0x01C0
            ]
        );
    }

    #[test]
    fn test_plt_decode_last() {
        let bytes = [
            0xff, 0x58, 0x00, 0x26, 0x00, 0x09, 0x7a, 0x13, 0x1e, 0x1b, 0x09, 0x29, 0x3e, 0x12,
            0x1d, 0x82, 0x05, 0x37, 0x52, 0x82, 0x2b, 0x5d, 0x87, 0x2d, 0x87, 0x37, 0x85, 0x2f,
            0x8d, 0x41, 0x8b, 0x23, 0x87, 0x70, 0x90, 0x78, 0x91, 0x0c, 0x91, 0x2f,
        ];
        let mut reader = Cursor::new(&bytes);
        reader.seek_relative(2).unwrap(); // skip marker
        let mut codestream = ContiguousCodestream::default();
        let res = codestream.decode_plt(&mut reader);
        assert!(res.is_ok());
        let plt = res.unwrap();
        assert_eq!(plt.length, 38);
        assert_eq!(plt.index[0], 0);
        // Iplt as reported by jpylyzer
        assert_eq!(
            plt.packet_length,
            vec![
                0x09, 0x7A, 0x13, 0x1E, 0x1B, 0x09, 0x29, 0x3E, 0x12, 0x1D, 0x0105, 0x37, 0x52,
                0x012B, 0x5D, 0x03AD, 0x03B7, 0x02AF, 0x06C1, 0x05A3, 0x03F0, 0x0878, 0x088C,
                0x08AF
            ]
        );
    }
}
