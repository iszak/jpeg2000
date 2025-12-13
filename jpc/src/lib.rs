#![allow(dead_code)]

use log::info;
use std::cmp;
use std::error;
use std::fmt;
use std::io;
use std::io::prelude::*;
use std::str;

mod coder;
mod tag_tree;

#[derive(Debug)]
enum CodestreamError {
    MarkerError {
        marker: MarkerSymbol,
        error: String,
    },
    MarkerMissing {
        marker: MarkerSymbol,
    },
    MarkerUnexpected {
        marker: MarkerSymbol,
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
}

impl error::Error for CodestreamError {}
impl fmt::Display for CodestreamError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::MarkerError { marker, error } => {
                write!(
                    f,
                    "marker 0x{:0>2X?}{:0>2X?} error {:?}",
                    marker[0], marker[1], error
                )
            }
            Self::MarkerMissing { marker } => {
                write!(f, "missing marker 0x{:0>2X?}{:0>2X?}", marker[0], marker[1])
            }
            Self::MarkerUnexpected { marker, offset } => {
                write!(
                    f,
                    "unexpected marker 0x{:0>2X?}{:0>2X?} at byte offset {}",
                    marker[0], marker[1], offset
                )
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

#[derive(Debug, PartialEq)]
pub enum ProgressionOrder {
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
    tile_length: [u8; 4],

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

// A.7.1
//
// Tile-part lengths (TLM)
//
// Function: Describes the length of every tile-part in the codestream. Each
// tile-part's length is measured from the first byte of the SOT marker segment
// to the end of the bit-stream data of that tile-part. The value of each
// individual tile-part length in the TLM marker segment is the same as the
// value in the corresponding Psot in the SOT marker segment.
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
}

#[derive(Debug, Default)]
struct TilePartLength {
    // Ttlm^i: Tile index of the ith tile-part.
    //
    // There is either none or one value for every tile-part.
    // The number of tile-parts in each tile can be derived from this marker
    // segment (or the concatenated list of all such markers) or from a
    // non-zero TNsot parameter, if present.
    tile_index: [u8; 2],

    // Ptlm^i: Length in bytes, from the beginning of the SOT marker of the ith
    // tile-part to the end of the bit stream data for that tile-part.
    //
    // There is one value for every tile-part
    tile_length: [u8; 4],
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

        match value << 2 >> 6 {
            0 => tile_part_parameter_sizes.push(TilePartParameterSize::TtlmNone),
            1 => tile_part_parameter_sizes.push(TilePartParameterSize::Ttlm8Bit),
            2 => tile_part_parameter_sizes.push(TilePartParameterSize::Ttlm16Bit),
            _ => {} // TODO: Add reserve values by removed known bits
        }

        match value << 1 >> 7 {
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

    // Iplm^i: Length of the ith packet.
    //
    // If packet headers are stored with the packet, this length includes the
    // packet header. If packet headers are stored in the PPM or PPT, this
    // length does not include the packet header lengths.
    packet_length: Vec<u8>,
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

    pub fn precision(&self, i: usize) -> Result<i16, Box<dyn error::Error>> {
        let ssiz = self.precision.get(i).unwrap();
        let precision = (u8::from_be_bytes(*ssiz) & 0x7f) as i16;
        // ISO/IEC 15444-1:2019 Table A.11, component bit depth is value + 1.
        Ok(precision + 1)
    }

    pub fn values_are_signed(&self, i: usize) -> Result<bool, Box<dyn error::Error>> {
        let ssiz = self.precision.get(i).unwrap();
        let is_signed = (u8::from_be_bytes(*ssiz) & 0x80) == 0x80;
        Ok(is_signed)
    }

    pub fn horizontal_separation(&self, i: usize) -> Result<u8, Box<dyn error::Error>> {
        let horizontal_separation = self.horizontal_separation.get(i).unwrap();
        Ok(u8::from_be_bytes(*horizontal_separation))
    }
    pub fn vertical_separation(&self, i: usize) -> Result<u8, Box<dyn error::Error>> {
        let vertical_separation = self.vertical_separation.get(i).unwrap();
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

    // upper left x corner of the tile
    // tx_0(p,q) = max(XTOsiz + p · XTsiz, XOsiz)
    fn tile_x_upper(&self, t: u32) -> u32 {
        cmp::max(
            self.tile_horizontal_offset()
                + (self.tile_horizontal_index(t) * self.reference_tile_width()),
            self.image_horizontal_offset(),
        )
    }

    // upper left y corner of the tile
    // ty_0(p,q) = max(YTOsiz + q · YTsiz, YOsiz)
    fn tile_y_upper(&self, t: u32) -> u32 {
        cmp::max(
            self.tile_vertical_offset()
                + (self.tile_vertical_index(t) * self.reference_tile_height()),
            self.image_vertical_offset(),
        )
    }

    // lower left x corner of the tile
    // tx_1(p,q) = max(XTOsiz + (p + 1) · XTsiz, XOsiz)
    fn tile_x_lower(&self, t: u32) -> u32 {
        cmp::min(
            self.tile_horizontal_offset()
                + ((self.tile_horizontal_index(t) + 1) * self.reference_tile_width()),
            self.image_horizontal_offset(),
        ) - 1
    }

    // lower left y corner of the tile
    // ty_1(p,q) = max(YTOsiz + (q + 1) · YTsiz, YOsiz)
    fn tile_y_lower(&self, t: u32) -> u32 {
        cmp::min(
            self.tile_vertical_offset()
                + ((self.tile_vertical_index(t) + 1) * self.reference_tile_height()),
            self.image_vertical_offset(),
        ) - 1
    }

    fn tile_dimensions(&self, t: u32) -> (u32, u32) {
        (
            self.tile_x_lower(t) - self.tile_x_upper(t),
            self.tile_y_lower(t) - self.tile_y_upper(t),
        )
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

#[derive(Debug, PartialEq)]
pub enum QuantizationStyle {
    No { guard: u8 },
    ScalarDerived { guard: u8 },
    ScalarExpounded { guard: u8 },
    Reserved { value: u8 },
}

impl QuantizationStyle {
    fn new(byte: u8) -> QuantizationStyle {
        let value = byte << 3 >> 3;

        // 000x xxxx to 111x xxxx, Number of guard bits: 0 to 7
        let guard = u8::from_be(byte >> 5);

        match value {
            // No quantization
            0b0000_0000 => QuantizationStyle::No { guard },

            // Scalar derived (values signalled for NLLL subband only).
            0b0000_0001 => QuantizationStyle::ScalarDerived { guard },
            // Scalar expounded (values signalled for each subband). There are
            // as many step sizes signalled as there are subbands.
            0b0000_0010 => QuantizationStyle::ScalarExpounded { guard },

            _ => QuantizationStyle::Reserved { value: byte },
        }
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
#[derive(Debug, Default)]
pub struct QuantizationDefaultMarkerSegment {
    // Length of marker segment in bytes (not including the marker).
    length: u16,

    // Sqcd: Quantization style for all components
    quantization_style: [u8; 1],

    // SPqcd^i: Quantization step size value for the ith subband in the defined
    // order.
    values: Vec<QuantizationValue>,
}

impl QuantizationDefaultMarkerSegment {
    pub fn length(&self) -> u16 {
        self.length
    }

    pub fn quantization_style_u8(&self) -> u8 {
        u8::from_be_bytes(self.quantization_style)
    }

    pub fn quantization_style(&self) -> QuantizationStyle {
        QuantizationStyle::new(self.quantization_style[0])
    }

    pub fn quantization_values(&self) -> Vec<u16> {
        self.values.iter().map(|e| e.value()).collect()
    }

    pub fn quantization_exponents(&self) -> Vec<u8> {
        self.values.iter().map(|e| e.exponent()).collect()
    }
}

// A.6.5
//
// Quantization component (QCC)
//
// Function: Describes the quantization used for compressing a particular
// component
#[derive(Debug, Default)]
pub struct QuantizationComponentSegment {
    offset: u64,

    // Lqcc
    length: u16,

    // Cqcc: The index of the component to which this marker segment relates.
    component_index: [u8; 2],

    // Sqcc: Quantization style for this component.
    quantization_style: [u8; 1],

    // SPqcci: Quantization value for each subband in the defined order.
    quantization_values: Vec<QuantizationValue>,
}

impl QuantizationComponentSegment {
    pub fn length(&self) -> u16 {
        self.length
    }

    pub fn component_index(&self) -> u16 {
        u16::from_be_bytes(self.component_index)
    }

    pub fn quantization_style_u8(&self) -> u8 {
        u8::from_be_bytes(self.quantization_style)
    }

    pub fn quantization_style(&self) -> QuantizationStyle {
        QuantizationStyle::new(self.quantization_style[0])
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
    tiles: Vec<Tile>,
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
        let mut length: [u8; 2] = [0; 2];
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
            let mut precision: [u8; 1] = [0; 1];
            reader.read_exact(&mut precision)?;
            segment.precision.push(precision);

            let mut horizontal_separation: [u8; 1] = [0; 1];
            reader.read_exact(&mut horizontal_separation)?;
            segment.horizontal_separation.push(horizontal_separation);

            let mut vertical_separation: [u8; 1] = [0; 1];
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
    fn decode_sot<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<StartOfTileSegment, Box<dyn error::Error>> {
        info!("SOT start at byte offset {}", reader.stream_position()? - 2);
        let mut segment = StartOfTileSegment::default();

        // LSot
        let mut marker_segment_length: [u8; 2] = [0; 2];
        reader.read_exact(&mut marker_segment_length)?;

        // ISot
        reader.read_exact(&mut segment.tile_index)?;

        // PSot
        reader.read_exact(&mut segment.tile_length)?;

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
            let mut buffer: [u8; 1] = [0; 1];
            reader.read_exact(&mut buffer)?;
            Ok([0, buffer[0]])
        } else {
            // TODO: Understand why 2 MSB are unused (signness is only 1 bit)
            let mut buffer: [u8; 2] = [0; 2];
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
        // marker segment.
        let no_progression_order_change = match no_components < 256 {
            true => segment.length - 2 - 7,
            false => segment.length - 2 - 9,
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
                reader
                    .take(1)
                    .read_exact(&mut tile_part_length.tile_index)?;
            } else if parameter_sizes.contains(&TilePartParameterSize::Ttlm16Bit) {
                reader
                    .take(2)
                    .read_exact(&mut tile_part_length.tile_index)?;
            }

            // Ptlm
            if parameter_sizes.contains(&TilePartParameterSize::Ptlm16Bit) {
                reader
                    .take(2)
                    .read_exact(&mut tile_part_length.tile_length)?;
            } else if parameter_sizes.contains(&TilePartParameterSize::Ptlm32Bit) {
                reader
                    .take(4)
                    .read_exact(&mut tile_part_length.tile_length)?;
            }
            segment.tile_part_lengths.push(tile_part_length);
        }

        info!("TLM end at byte offset {}", reader.stream_position()?);
        Ok(segment)
    }

    fn decode_quantization_values<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
        quantization_style: QuantizationStyle,
        no_decomposition_levels: u8,
    ) -> Result<Vec<QuantizationValue>, Box<dyn error::Error>> {
        // Decomposition levels are divided into subbands. These include the HL, LH, and HH subbands of the same two dimensional subband decomposition. For the last decomposition level the LL subband is also included.
        let no_subbands = no_decomposition_levels * 3 + 1;

        let mut quantization_values: Vec<QuantizationValue> =
            Vec::with_capacity(no_subbands as usize);

        for _ in 0..no_subbands {
            match quantization_style {
                // Reversible transformation values
                QuantizationStyle::No { guard: _ } => {
                    let mut value: [u8; 1] = [0; 1];
                    reader.read_exact(&mut value)?;

                    let quantization_value = QuantizationValue::Reversible { value };
                    quantization_values.push(quantization_value);
                }
                // Irreversible transformation values
                QuantizationStyle::ScalarExpounded { guard: _ }
                | QuantizationStyle::ScalarDerived { guard: _ } => {
                    let mut value: [u8; 2] = [0; 2];
                    reader.read_exact(&mut value)?;

                    let quantization_value = QuantizationValue::Irreversible { value };
                    quantization_values.push(quantization_value);
                }
                QuantizationStyle::Reserved { value: _value } => {
                    todo!()
                }
            }
        }

        Ok(quantization_values)
    }

    fn decode_qcd<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<QuantizationDefaultMarkerSegment, Box<dyn error::Error>> {
        info!("QCD start at byte offset {}", reader.stream_position()? - 2);
        let mut segment = QuantizationDefaultMarkerSegment {
            length: self.decode_length(reader)?,
            ..Default::default()
        };

        reader.read_exact(&mut segment.quantization_style)?;

        let no_decomposition_levels = match segment.quantization_style() {
            QuantizationStyle::No { guard: _ } => (segment.length() - 4) / 3,
            QuantizationStyle::ScalarDerived { guard: _ } => 5,
            QuantizationStyle::ScalarExpounded { guard: _ } => (segment.length() - 5) / 6,
            _ => panic!(),
        } as u8;

        segment.values = self.decode_quantization_values(
            reader,
            segment.quantization_style(),
            no_decomposition_levels,
        )?;
        info!("QCD end at byte offset {}", reader.stream_position()?);

        Ok(segment)
    }

    fn decode_qcc<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
        no_components: u16,
    ) -> Result<QuantizationComponentSegment, Box<dyn error::Error>> {
        info!("QCC start at byte offset {}", reader.stream_position()? - 2);
        let mut segment = QuantizationComponentSegment {
            offset: reader.stream_position()?,
            length: self.decode_length(reader)?,
            ..Default::default()
        };

        // Cqcc
        segment.component_index = self.decode_component_index(reader, no_components)?;

        // Sqcc
        reader.read_exact(&mut segment.quantization_style)?;

        let mut no_decomposition_levels = match segment.quantization_style() {
            QuantizationStyle::No { guard: _ } => (segment.length() - 5) / 3,
            QuantizationStyle::ScalarDerived { guard: _ } => 6,
            QuantizationStyle::ScalarExpounded { guard: _ } => (segment.length() - 6) / 6,
            _ => panic!(),
        } as u8;

        if no_components >= 257 {
            no_decomposition_levels += 1;
        }

        // SPqcc

        segment.quantization_values = self.decode_quantization_values(
            reader,
            segment.quantization_style(),
            no_decomposition_levels,
        )?;
        info!("QCC end at byte offset {}", reader.stream_position()?);

        Ok(segment)
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
        let mut packet_length: [u8; 1] = [0; 1];
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
        info!("PLT start at byte offset {}", reader.stream_position()? - 2);
        let mut segment = TilePacketLength {
            offset: reader.stream_position()?,
            length: self.decode_length(reader)?,
            ..Default::default()
        };

        reader.read_exact(&mut segment.index)?;

        self.decode_packet_length(reader, &mut segment.packet_length)?;

        info!("PLT end at byte offset {}", reader.stream_position()?);

        Ok(segment)
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
            let mut horizontal_offset: [u8; 2] = [0; 2];
            reader.read_exact(&mut horizontal_offset)?;
            segment.horizontal_offset.push(horizontal_offset);

            let mut vertical_offset: [u8; 2] = [0; 2];
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
        let mut marker_segment_length: [u8; 2] = [0; 2];
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
    tile_part_lengths: Option<TilePartLengthsSegment>,

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

    /// Tile-part lengths (TLM) segment
    ///
    /// Describes the length of every tile-part in the codestream.
    ///
    /// See ITU-T T.800 or ISO/IEC 15444-1:2019 Section A.7.1 for how this works.
    pub fn tile_part_lengths_segment(&self) -> &Option<TilePartLengthsSegment> {
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
struct Tile {
    header: TileHeader,
    parts: Vec<u8>,
}

#[derive(Debug, Default)]
struct TileHeader {
    // SOT (Required)
    start_of_tile_segment: StartOfTileSegment,

    // COD (Optional)
    coding_style_marker_segment: CodingStyleMarkerSegment,

    // COC (Optional)
    coding_style_component_segment: CodingStyleComponentSegment,

    // QCD (Optional)
    quantization_default_marker_segment: QuantizationDefaultMarkerSegment,

    // QCC (Optional)
    quantization_component_segment: QuantizationComponentSegment,

    // RGN (Optional)
    regions: Vec<RegionOfInterestSegment>,

    // POC (Required)
    progression_order_change: ProgressionOrderChangeSegment,

    // PPT (Optional)
    packed_packet_headers: Option<TilePackedPacketHeaderSegment>,

    // PLT (Optional)
    packet_lengths: Vec<PacketLengthSegment>,

    // COM (Optional)
    comment_marker_segment: Option<CommentMarkerSegment>,
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

        let mut marker_type: MarkerSymbol = [0; 2];

        // SOC (Required as the first marker)
        reader.read_exact(&mut marker_type)?;
        if marker_type != MARKER_SYMBOL_SOC {
            return Err(CodestreamError::MarkerUnexpected {
                marker: MARKER_SYMBOL_SOC,
                offset: reader.stream_position()? - 2,
            }
            .into());
        }
        info!("SOC start at byte offset {}", reader.stream_position()? - 2);

        // SIZ (Required as the second marker segment)
        reader.read_exact(&mut marker_type)?;
        if marker_type != MARKER_SYMBOL_SIZ {
            return Err(CodestreamError::MarkerUnexpected {
                marker: MARKER_SYMBOL_SIZ,
                offset: reader.stream_position()? - 2,
            }
            .into());
        }

        header.image_and_tile_size_marker_segment = self.decode_siz(reader)?;

        let no_components = header.image_and_tile_size_marker_segment.no_components();

        loop {
            match reader.read_exact(&mut marker_type) {
                Ok(_) => match marker_type {
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

                    // TLM (Optional)
                    MARKER_SYMBOL_TLM => {
                        header.tile_part_lengths = Some(self.decode_tlm(reader)?);
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

                    // COM (Optional)
                    MARKER_SYMBOL_COM => {
                        let comment_marker_segment = self.decode_com(reader)?;
                        header.comment_marker_segments.push(comment_marker_segment);
                    }

                    // Start of tile bit-stream
                    MARKER_SYMBOL_SOT => {
                        reader.seek(io::SeekFrom::Current(-2))?;
                        break;
                    }
                    _ => {
                        return Err(CodestreamError::MarkerUnexpected {
                            marker: marker_type,
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
                marker: MARKER_SYMBOL_QCD,
            }
            .into());
        }
        if header.coding_style_marker_segment.is_none() {
            return Err(CodestreamError::MarkerMissing {
                marker: MARKER_SYMBOL_COD,
            }
            .into());
        }

        // A.6.2
        // No more than one per any given component may be present in either the main or tile-part headers
        if header.coding_style_component_segment.len() > (no_components as usize) {
            return Err(CodestreamError::MarkerError {
                marker: MARKER_SYMBOL_COC,
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
                marker: MARKER_SYMBOL_RGN,
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
                marker: MARKER_SYMBOL_QCC,
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

    // A.4 – Construction of the first tile-part header of a given tile
    fn decode_first_tile_header<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
        no_components: u16,
    ) -> Result<TileHeader, Box<dyn error::Error>> {
        let mut tile_header = TileHeader::default();

        let mut marker_type: MarkerSymbol = [0; 2];

        reader.read_exact(&mut marker_type)?;

        // SOT (Required as the first marker segment of every tile-part header)
        if marker_type != MARKER_SYMBOL_SOT {
            return Err(CodestreamError::MarkerUnexpected {
                marker: MARKER_SYMBOL_SOT,
                offset: reader.stream_position()? - 2,
            }
            .into());
        }

        tile_header.start_of_tile_segment = self.decode_sot(reader)?;

        loop {
            match reader.read_exact(&mut marker_type) {
                Ok(_) => match marker_type {
                    // COD (Optional)
                    MARKER_SYMBOL_COD => {
                        tile_header.coding_style_marker_segment = self.decode_cod(reader)?;
                    }

                    // COC (Optional)
                    MARKER_SYMBOL_COC => {
                        tile_header.coding_style_component_segment =
                            self.decode_coc(reader, no_components)?;
                    }

                    // QCD (Optional)
                    MARKER_SYMBOL_QCD => {
                        tile_header.quantization_default_marker_segment =
                            self.decode_qcd(reader)?;
                    }

                    // QCC (Optional)
                    MARKER_SYMBOL_QCC => {
                        tile_header.quantization_component_segment =
                            self.decode_qcc(reader, no_components)?;
                    }

                    // RGN (Optional)
                    MARKER_SYMBOL_RGN => {
                        tile_header
                            .regions
                            .push(self.decode_rgn(reader, no_components)?);
                    }

                    // POC (Optional)
                    MARKER_SYMBOL_POC => {
                        tile_header.progression_order_change =
                            self.decode_poc(reader, no_components)?;
                    }

                    // PPT (Optional)
                    MARKER_SYMBOL_PPT => {
                        // The packet headers shall be in only one of three places within the codestream. If the PPM
                        // marker segment is present, all the packet headers shall be found in the main header.
                        //
                        // In this case, the PPT marker segment and packets distributed in the bit stream of the
                        // tile-parts are disallowed.
                        if !self.header.packed_packet_headers.is_empty() {
                            return Err(CodestreamError::MarkerUnexpected {
                                marker: MARKER_SYMBOL_PPT,
                                offset: reader.stream_position()? - 2,
                            }
                            .into());
                        }

                        tile_header.packed_packet_headers = Some(self.decode_ppt(reader)?);
                    }

                    // PLT (Optional)
                    MARKER_SYMBOL_PLT => {
                        let packet_length = self.decode_plm(reader)?;
                        tile_header.packet_lengths.push(packet_length);
                    }

                    // COM (Optional)
                    MARKER_SYMBOL_COM => {
                        tile_header.comment_marker_segment = Some(self.decode_com(reader)?);
                    }
                    // COM (Optional)
                    MARKER_SYMBOL_SOD => {
                        reader.seek(io::SeekFrom::Current(-2))?;
                        break;
                    }
                    _ => panic!(),
                },

                Err(e) => return Err(e.into()),
            }
        }

        Ok(tile_header)
    }

    fn decode<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<(), Box<dyn error::Error>> {
        // The main header is found at the beginning of the codestream
        self.header = self.decode_main_header(reader)?;

        let no_components = self
            .header
            .image_and_tile_size_marker_segment
            .no_components();

        // The tile-part headers are found at the beginning of each tile-part
        let tile_header = self.decode_first_tile_header(reader, no_components)?;
        let mut marker_type: MarkerSymbol = [0; 2];

        // Required as the last marker segment of every tile-part header
        reader.read_exact(&mut marker_type)?;
        if marker_type != MARKER_SYMBOL_SOD {
            return Err(CodestreamError::MarkerUnexpected {
                marker: MARKER_SYMBOL_SOD,
                offset: reader.stream_position()?,
            }
            .into());
        }

        let coding_styles = self.header.coding_style_marker_segment().coding_styles();

        let start_of_data = reader.stream_position()?;
        info!("SOD start at byte offset {}", start_of_data - 2);

        loop {
            match reader.read_exact(&mut marker_type) {
                Ok(_) => match marker_type {
                    // in bit-stream markers
                    MARKER_SYMBOL_SOP => {
                        info!("SOP start at byte offset {}", reader.stream_position()? - 2);
                        if coding_styles.contains(&CodingStyleDefault::NoSOP) {
                            return Err(CodestreamError::MarkerUnexpected {
                                marker: MARKER_SYMBOL_SOP,
                                offset: reader.stream_position()? - 2,
                            }
                            .into());
                        } else {
                            // ITU-T H.800 or ISO/IEC 15444-1 2024, Section A.8.1
                            let mut buf = [0u8; 2];
                            reader.read_exact(&mut buf)?;
                            let lsop = u16::from_be_bytes(buf);
                            // TODO: if using strict parsing, check length == 4
                            reader.read_exact(&mut buf)?;
                            let nsop = u16::from_be_bytes(buf);
                            // TODO: if using strict parsing, check nsop increment matches packet number,
                            // even if SOP wasn't present
                            info!("SOP length {lsop}, sequence number {nsop}");
                        }
                    }
                    MARKER_SYMBOL_EPH => {
                        // If packet headers are not in-bit stream (i.e., PPM or PPT marker segments are used), this
                        // marker shall not be used in the bit stream
                        if !self.header.packed_packet_headers.is_empty()
                            || tile_header.packed_packet_headers.is_some()
                        {
                            return Err(CodestreamError::MarkerUnexpected {
                                marker: MARKER_SYMBOL_EPH,
                                offset: reader.stream_position()? - 2,
                            }
                            .into());
                        }

                        if coding_styles.contains(&CodingStyleDefault::NoEPH) {
                            return Err(CodestreamError::MarkerUnexpected {
                                marker: MARKER_SYMBOL_EPH,
                                offset: reader.stream_position()? - 2,
                            }
                            .into());
                        } else {
                            // ITU-T H.800 or ISO/IEC 15444-1 2024, Section A.8.2
                            // Empty marker, not even the length.
                        }
                    }
                    // delimiting markers
                    MARKER_SYMBOL_EOC => {
                        info!("EOC end at byte offset {}", reader.stream_position()?);
                        break;
                    }
                    MARKER_SYMBOL_SOT => {
                        // A.4.4
                        todo!();
                    }
                    _ => {
                        // TODO: See J.10.3 Packet headers
                        //
                        // decode packet header B.10.8
                        //   1 bit for zero or non-zero length packet
                        //   for each sub-band (LL or HL, LH and HH)
                        //     for all code-blocks in this sub-band confined to the relevant precinct, in raster order
                        //       code-block inclusion bits (if not previously included then tag tree, else one bit)
                        //       if code-block included
                        //         if first instance of code-block
                        //           zero bit-planes information
                        //           number of coding passes included
                        //           increase of code-block length indicator (Lblock)
                        //           for each codeword segment
                        //           length of codeword segment

                        // decode packet data J.10.4
                        //   Arithmetic-coded compressed data

                        // Decode codestream packet header

                        // Bits are packed into bytes from the MSB to the LSB.
                        // Once a complete byte is assembled, it is appended to
                        // the packet header.

                        // If the value of the byte is 0xFF, the next byte
                        // includes an extra zero bit stuffed into the MSB

                        // Once all bits of the packet header have been
                        // assembled, the last byte is packed to the byte
                        // boundary and emitted

                        // The last byte in the packet header shall not be an
                        // 0xFF value (thus the single zero bit stuffed after a
                        // byte with 0xFF must be included even if the 0xFF
                        // would otherwise have been the last byte).
                    }
                },

                Err(e) => match e.kind() {
                    io::ErrorKind::UnexpectedEof => break,
                    _ => return Err(e.into()),
                },
            }
        }

        let end_of_data = reader.stream_position()?;
        info!("SOD end at byte offset {}", end_of_data);

        // TODO: Support multiple SOT

        // TODO: avoid seeking
        reader.seek(io::SeekFrom::Start(start_of_data))?;

        reader.seek(io::SeekFrom::Start(end_of_data))?;

        self.tiles.push(Tile {
            header: tile_header,
            parts: vec![],
        });

        Ok(())
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
    // For the last decomposition level the LL subband is also included.0

    Ok(continuous_codestream)
}
