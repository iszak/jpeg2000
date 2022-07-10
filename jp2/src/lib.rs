#![allow(dead_code)]

use log::{debug, info, warn};
use std::error;
use std::fmt;
use std::io;
use std::str;

#[derive(Debug)]
pub enum JP2Error {
    InvalidSignature { signature: [u8; 4], offset: u64 },
    InvalidBrand { brand: [u8; 4], offset: u64 },
    Unsupported,
    NotCompatible { compatibility_list: Vec<String> },
    BoxUnexpected { box_type: BoxType, offset: u64 },
    BoxDuplicate { box_type: BoxType, offset: u64 },
    BoxMalformed { box_type: BoxType, offset: u64 },
    BoxMissing { box_type: BoxType },
}

impl error::Error for JP2Error {}
impl fmt::Display for JP2Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::InvalidSignature { signature, offset } => {
                write!(
                    f,
                    "invalid signature {:?} at offset {}",
                    str::from_utf8(signature).unwrap(),
                    offset
                )
            }
            Self::InvalidBrand { brand, offset } => {
                write!(
                    f,
                    "invalid brand {:?} at offset {}",
                    str::from_utf8(brand).unwrap(),
                    offset
                )
            }
            Self::NotCompatible { compatibility_list } => {
                write!(
                    f,
                    "'jp2 ' not found in compatibility list '{}'",
                    compatibility_list.join(", ")
                )
            }
            Self::BoxDuplicate { box_type, offset } => {
                write!(
                    f,
                    "unexpected duplicate box type {:?} at offset {}",
                    box_type, offset
                )
            }
            Self::BoxUnexpected { box_type, offset } => {
                write!(f, "unexpected box type {:?} at offset {}", box_type, offset)
            }
            Self::BoxMalformed { box_type, offset } => {
                write!(f, "malformed box type {:?} at offset {}", box_type, offset)
            }
            Self::BoxMissing { box_type } => {
                write!(f, "box type {:?} missing", box_type)
            }
            Self::Unsupported => {
                write!(
                    f,
                    "only JPEG 2000 part-1 (ISO 15444-1 / T.800) is supported",
                )
            }
        }
    }
}

// jP\040\040 (0x6A50 2020)
const BOX_TYPE_SIGNATURE: BoxType = [106, 80, 32, 32];
const BOX_TYPE_FILE_TYPE: BoxType = [102, 116, 121, 112];
const BOX_TYPE_HEADER: BoxType = [106, 112, 50, 104];
const BOX_TYPE_IMAGE_HEADER: BoxType = [105, 104, 100, 114];
const BOX_TYPE_BITS_PER_COMPONENT: BoxType = [98, 112, 99, 99];
const BOX_TYPE_COLOUR_SPECIFICATION: BoxType = [99, 111, 108, 114];
const BOX_TYPE_PALETTE: BoxType = [112, 99, 108, 114];
const BOX_TYPE_COMPONENT_MAPPING: BoxType = [99, 109, 97, 112];
const BOX_TYPE_CHANNEL_DEFINITION: BoxType = [99, 100, 101, 102];
const BOX_TYPE_RESOLUTION: BoxType = [114, 101, 115, 32];
const BOX_TYPE_CAPTURE_RESOLUTION: BoxType = [114, 101, 115, 99];
const BOX_TYPE_DEFAULT_DISPLAY_RESOLUTION: BoxType = [114, 101, 115, 100];
const BOX_TYPE_CONTIGUOUS_CODESTREAM: BoxType = [106, 112, 50, 99];
const BOX_TYPE_INTELLECTUAL_PROPERTY: BoxType = [106, 112, 50, 105];
const BOX_TYPE_XML: BoxType = [120, 109, 108, 32];
const BOX_TYPE_UUID: BoxType = [117, 117, 105, 100];
const BOX_TYPE_UUID_INFO: BoxType = [117, 105, 110, 102];
const BOX_TYPE_UUID_LIST: BoxType = [117, 108, 115, 116];
const BOX_TYPE_DATA_ENTRY_URL: BoxType = [117, 114, 108, 32];

// jp2\040
const BRAND_JP2: [u8; 4] = [106, 112, 50, 32];

// jp2\040
const BRAND_JPX: [u8; 4] = [106, 112, 120, 32];

// <CR><LF><0x87><LF> (0x0D0A 870A).
const SIGNATURE_MAGIC: [u8; 4] = [13, 10, 135, 10];

#[derive(Debug)]
enum BoxTypes {
    Signature,
    FileType,
    Header,
    ImageHeader,
    BitsPerComponent,
    ColourSpecification,
    Palette,
    ComponentMapping,
    ChannelDefinition,
    Resolution,
    CaptureResolution,
    DefaultDisplayResolution,
    ContiguousCodestream,
    IntellectualProperty,
    XML,
    UUID,
    UUIDInfo,
    UUIDList,
    DataEntryURL,
    Unknown,
}

impl fmt::Display for BoxTypes {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl BoxTypes {
    fn new(value: BoxType) -> BoxTypes {
        match value {
            BOX_TYPE_SIGNATURE => BoxTypes::Signature,
            BOX_TYPE_FILE_TYPE => BoxTypes::FileType,
            BOX_TYPE_HEADER => BoxTypes::Header,
            BOX_TYPE_IMAGE_HEADER => BoxTypes::ImageHeader,
            BOX_TYPE_BITS_PER_COMPONENT => BoxTypes::BitsPerComponent,
            BOX_TYPE_COLOUR_SPECIFICATION => BoxTypes::ColourSpecification,
            BOX_TYPE_PALETTE => BoxTypes::Palette,
            BOX_TYPE_COMPONENT_MAPPING => BoxTypes::ComponentMapping,
            BOX_TYPE_CHANNEL_DEFINITION => BoxTypes::ChannelDefinition,

            BOX_TYPE_RESOLUTION => BoxTypes::Resolution,
            BOX_TYPE_CAPTURE_RESOLUTION => BoxTypes::CaptureResolution,
            BOX_TYPE_DEFAULT_DISPLAY_RESOLUTION => BoxTypes::DefaultDisplayResolution,

            BOX_TYPE_CONTIGUOUS_CODESTREAM => BoxTypes::ContiguousCodestream,
            BOX_TYPE_INTELLECTUAL_PROPERTY => BoxTypes::IntellectualProperty,
            BOX_TYPE_XML => BoxTypes::XML,

            BOX_TYPE_UUID => BoxTypes::UUID,
            BOX_TYPE_UUID_INFO => BoxTypes::UUIDInfo,
            BOX_TYPE_UUID_LIST => BoxTypes::UUIDList,
            BOX_TYPE_DATA_ENTRY_URL => BoxTypes::DataEntryURL,
            _ => BoxTypes::Unknown,
        }
    }
}

type BoxType = [u8; 4];

// The building-block of the JP2 file format is called a box.
//
// All information contained within the JP2 file is encapsulated in boxes.
//
// This Recommendation | International Standard defines several types of boxes;
// the definition of each specific box type defines the kinds of information
// that may be found within a box of that type. Some boxes will be defined to
// contain other boxes.
pub trait JBox {
    fn identifier(&self) -> BoxType;
    fn length(&self) -> u64;
    fn offset(&self) -> u64;

    fn decode<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<(), Box<dyn error::Error>>;
}

// I.5.1
//
// JPEG 2000 Signature box
//
// The Signature box identifies that the format of this file was defined by the
// JPEG 2000 Recommendation | International Standard, as well as provides a
// small amount of information which can help determine the validity of the rest
// of the file.
//
// The Signature box shall be the first box in the file, and all files shall
// contain one and only one Signature box.

// For file verification purposes, this box can be considered a fixed-length
// 12-byte string which shall have the value: 0x0000 000C 6A50 2020 0D0A 870A.

// The combination of the particular type and contents for this box enable an
// application to detect a common set of file transmission errors.
//
// - The CR-LF sequence in the contents catches bad file transfers that alter
// newline sequences.
// - The control-Z character in the type stops file display under MS-DOS.
// - The final linefeed checks for the inverse of the CR-LF translation problem.
// - The third character of the box contents has its high-bit set to catch bad
// file transfers that clear bit 7
#[derive(Debug, Default)]
pub struct SignatureBox {
    length: u64,
    offset: u64,
}

impl SignatureBox {
    pub fn signature(&self) -> [u8; 4] {
        SIGNATURE_MAGIC
    }
}

impl JBox for SignatureBox {
    // The type of the JPEG 2000 Signature box shall be ‘jP\040\040’ (0x6A50 2020)
    fn identifier(&self) -> BoxType {
        BOX_TYPE_SIGNATURE
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    // The contents of this box shall be the 4-byte character string ‘<CR><LF><0x87><LF>’ (0x0D0A 870A).
    fn decode<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<(), Box<dyn error::Error>> {
        self.length = 12;

        let mut buffer: [u8; 4] = [0; 4];

        reader.read_exact(&mut buffer)?;

        if buffer != SIGNATURE_MAGIC {
            return Err(JP2Error::InvalidSignature {
                signature: buffer,
                offset: reader.stream_position()?,
            }
            .into());
        };

        Ok(())
    }
}

type CompatibilityList = Vec<[u8; 4]>;

// I.5.2
//
// File Type box
//
// The File Type box completely defines all of the contents of this file, as
// well as a separate list of readers with which this file is compatible, and
// thus the file can be properly interpreted within the scope of that other
// standard.
//
// This box shall immediately follow the Signature box.
//
// All files shall contain one and only one File Type box
//
// This differentiates between the standard which completely describes the file,
// from other standards that interpret a subset of the file.
#[derive(Debug, Default)]
pub struct FileTypeBox {
    length: u64,
    offset: u64,
    brand: [u8; 4],
    min_version: [u8; 4],
    compatibility_list: CompatibilityList,
}

impl FileTypeBox {
    // Brand
    //
    // This field specifies the Recommendation | International Standard which
    // completely defines this file.
    //
    // This field is specified by a four byte string of ISO 646 characters.
    //
    // In addition, the Brand field shall be considered functionally equivalent
    // to a major version number. A major version change (if there ever is one),
    // representing an incompatible change in the JP2 file format, shall define
    // a different value for the Brand field.
    //
    // If the value of the Brand field is not ‘jp2\040’, then a value of
    // ‘jp2\040’ in the Compatibility listindicates that a JP2 reader can
    // interpret the file in some manner as intended by the creator of the
    // file.
    pub fn brand(&self) -> &str {
        str::from_utf8(&self.brand).unwrap()
    }

    // Minor version
    //
    // This parameter defines the minor version number of this JP2 specification
    // for which thefile complies.
    //
    // The parameter is defined as a 4-byte big endian unsigned integer.
    //
    // The value of this field shall be zero.
    //
    // However, readers shall continue to parse and interpret this file even if
    // the value of thisfield is not zero.
    pub fn min_version(&self) -> u32 {
        u32::from_be_bytes(self.min_version)
    }

    // Compatibility list
    //
    // This field specifies a code representing this Recommendation |
    // International Standard, another standard, or a profile of another
    // standard, to which the file conforms.
    //
    // This field is encoded as a four byte string of ISO 646 characters.
    pub fn compatibility_list(&self) -> Vec<String> {
        self.compatibility_list
            .iter()
            .map(|c| str::from_utf8(c).unwrap().to_owned())
            .collect()
    }
}

impl JBox for FileTypeBox {
    // The type of the File Type Box shall be ‘ftyp’ (0x6674 7970).
    fn identifier(&self) -> BoxType {
        BOX_TYPE_FILE_TYPE
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn decode<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<(), Box<dyn error::Error>> {
        reader.read_exact(&mut self.brand)?;
        if self.brand == BRAND_JPX {
            return Err(JP2Error::Unsupported {}.into());
        } else if self.brand != BRAND_JP2 {
            return Err(JP2Error::InvalidBrand {
                brand: self.brand,
                offset: reader.stream_position()?,
            }
            .into());
        }

        reader.read_exact(&mut self.min_version)?;

        let mut buffer: [u8; 4] = [0; 4];

        // The number of CL fields is determined by the length of this box
        let mut size = (self.length() - 8) / 4;
        while size > 0 {
            reader.read_exact(&mut buffer)?;
            self.compatibility_list.extend_from_slice(&[buffer]);
            size -= 1;
        }

        // A file shall have at least one CL field in the File Type box, and shall contain the value‘jp2\040’ in one of the CL fields in the File Type box, and all conforming readers shall properly interpret all files with ‘jp2\040’ in one of the CL fields.
        // Other values of the Compatibility list field are reserved for ISO use.
        if self
            .compatibility_list
            .iter()
            .find(|c| **c == BRAND_JP2)
            .is_none()
        {
            return Err(JP2Error::NotCompatible {
                compatibility_list: self.compatibility_list().clone(),
            }
            .into());
        }

        Ok(())
    }
}

// I.5.3
//
// JP2 Header Box
//
// The JP2 Header box contains generic information about the file, such as
// number of components, colourspace, and grid resolution.
//
// This box is a superbox.
// This box contains several boxes.
//
// Within a JP2 file, there shall be one and only one JP2 Header box.
//
// Other boxes may be defined in other standards and may be ignored by
// conforming readers. Those boxes contained within the JP2 Header box that are
// defined within this Recommendation | InternationalStandard are as follows:

// - Image Header box - This box specifies information about the image, such
// as its height and width.
//
// - Bits Per Component box - This box specifies the bit depth of each
// component in the codestream after decompression. This box may be found
// anywhere in the JP2 Header box provided that it comes after the Image Header
// box.
//
// - Colour Specification boxes - These boxes specify the colourspace of the
// decompressed image. The use of multiple Colour Specification boxes
// provides the ability for a decoder to be given multiple optimization or
// compatibility options for colour processing. These boxes may be found
// anywhere in the JP2 Header box provided that they come after the Image Header
// box. All Colour Specification boxes shall be contiguous within the JP2 Header
// box.
//
// - Palette box - This box defines the palette to use to create multiple
// components from a single component. This box may be found anywhere in the JP2
// Header box provided that it comes after the Image Header box.
//
// - Component Mapping box - This box defines how image channels are identified
// from the actual components in the codestream. This box may be found anywhere
// in the JP2 Header box provided that it comes after the Image Header box.
//
// - Channel Definition box - This box defines the channels in the image. This
// box may be found anywhere in the JP2 Header box provided that it comes after
// the ImageHeader box.
//
// - Resolution box - This box specifies the capture and default display grid
// resolutions of the image. This box may be found anywhere in the JP2 Header
// box provided that it comes after the Image Header box.
#[derive(Debug, Default)]
pub struct HeaderSuperBox {
    length: u64,
    offset: u64,
    pub image_header_box: ImageHeaderBox,
    pub bits_per_component_box: Option<BitsPerComponentBox>,
    pub colour_specification_boxes: Vec<ColourSpecificationBox>,
    pub palette_box: Option<PaletteBox>,
    pub component_mapping_box: Option<ComponentMappingBox>,
    pub channel_definition_box: Option<ChannelDefinitionBox>,
    pub resolution_box: Option<ResolutionSuperBox>,
}

impl JBox for HeaderSuperBox {
    // The type of the JP2 Header box shall be ‘jp2h’ (0x6A70 3268)
    fn identifier(&self) -> BoxType {
        BOX_TYPE_HEADER
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn decode<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<(), Box<dyn error::Error>> {
        let BoxHeader {
            box_length,
            box_type,
            header_length: _,
        } = decode_box_header(reader)?;

        if box_type != self.image_header_box.identifier() {
            return Err(JP2Error::BoxUnexpected {
                box_type,
                offset: reader.stream_position()?,
            }
            .into());
        }
        self.image_header_box.length = box_length;
        self.image_header_box.offset = reader.stream_position()?;
        info!("ImageHeaderBox start at {:?}", self.image_header_box.offset);
        self.image_header_box.decode(reader)?;
        info!("ImageHeaderBox finish at {:?}", reader.stream_position()?);

        loop {
            let BoxHeader {
                box_length,
                box_type,
                header_length,
            } = decode_box_header(reader)?;

            match BoxTypes::new(box_type) {
                BoxTypes::ImageHeader => {
                    // Instances of Image Header box in other places in the file shall be ignored.
                    warn!("ImageHeaderBox found in other place, ignoring");
                }
                BoxTypes::ColourSpecification => {
                    let mut colour_specification_box = ColourSpecificationBox {
                        length: box_length,
                        offset: reader.stream_position()?,
                        method: [0; 1],
                        precedence: [0; 1],
                        colourspace_approximation: [0; 1],
                        enumerated_colour_space: ENUMERATED_COLOUR_SPACE_UNKNOWN,
                        restricted_icc_profile: vec![],
                    };
                    info!(
                        "ColourSpecificationBox start at {:?}",
                        colour_specification_box.offset,
                    );
                    colour_specification_box.decode(reader)?;
                    self.colour_specification_boxes
                        .push(colour_specification_box);
                    info!(
                        "ColourSpecificationBox finish at {:?}",
                        reader.stream_position()?
                    );
                }
                BoxTypes::BitsPerComponent => {
                    // There shall be one and only one Bits Per Component box inside a JP2 Header box.
                    if self.bits_per_component_box.is_some() {
                        return Err(JP2Error::BoxDuplicate {
                            box_type: BOX_TYPE_BITS_PER_COMPONENT,
                            offset: reader.stream_position()?,
                        }
                        .into());
                    }
                    let components_num = self.image_header_box.components_num();
                    let mut bits_per_component_box = BitsPerComponentBox {
                        components_num,
                        bits_per_component: vec![0; components_num as usize],
                        length: box_length,
                        offset: reader.stream_position()?,
                    };
                    info!(
                        "BitsPerComponentBox start at {:?}",
                        bits_per_component_box.offset
                    );
                    bits_per_component_box.decode(reader)?;
                    self.bits_per_component_box = Some(bits_per_component_box);
                    info!(
                        "BitsPerComponentBox finish at {:?}",
                        reader.stream_position()?
                    );
                }
                BoxTypes::Palette => {
                    // There shall be at most one Palette box inside a JP2 Header box.
                    if self.palette_box.is_some() {
                        return Err(JP2Error::BoxDuplicate {
                            box_type: BOX_TYPE_PALETTE,
                            offset: reader.stream_position()?,
                        }
                        .into());
                    }
                    let mut palette_box = PaletteBox::default();
                    palette_box.length = box_length;
                    palette_box.offset = reader.stream_position()?;
                    info!("PaletteBox start at {:?}", palette_box.offset);
                    palette_box.decode(reader)?;
                    self.palette_box = Some(palette_box);
                    info!("PaletteBox finish at {:?}", reader.stream_position()?);
                }
                BoxTypes::ComponentMapping => {
                    // There shall be at most one Component Mapping box inside a JP2 Header box.
                    if self.component_mapping_box.is_some() {
                        return Err(JP2Error::BoxDuplicate {
                            box_type: BOX_TYPE_COMPONENT_MAPPING,
                            offset: reader.stream_position()?,
                        }
                        .into());
                    }

                    let mut component_mapping_box = ComponentMappingBox {
                        length: box_length,
                        offset: reader.stream_position()?,
                        mapping: vec![],
                    };
                    info!(
                        "ComponentMappingBox start at {:?}",
                        component_mapping_box.offset
                    );
                    component_mapping_box.decode(reader)?;
                    info!(
                        "ComponentMappingBox finish at {:?}",
                        reader.stream_position()?
                    );
                    self.component_mapping_box = Some(component_mapping_box);
                }
                BoxTypes::ChannelDefinition => {
                    // There shall be at most one Channel Definition box inside a JP2 Header box.
                    if self.channel_definition_box.is_some() {
                        return Err(JP2Error::BoxDuplicate {
                            box_type: BOX_TYPE_CHANNEL_DEFINITION,
                            offset: reader.stream_position()?,
                        }
                        .into());
                    }

                    let mut channel_definition_box = ChannelDefinitionBox::default();
                    channel_definition_box.length = box_length;
                    channel_definition_box.offset = reader.stream_position()?;
                    info!(
                        "ChannelDefinitionBox start at {:?}",
                        channel_definition_box.offset
                    );
                    channel_definition_box.decode(reader)?;
                    info!(
                        "ChannelDefinitionBox finish at {:?}",
                        reader.stream_position()?
                    );
                    self.channel_definition_box = Some(channel_definition_box);
                }
                BoxTypes::Resolution => {
                    // There shall be at most one Resolution box inside a JP2 Header box.
                    if self.resolution_box.is_some() {
                        return Err(JP2Error::BoxDuplicate {
                            box_type: BOX_TYPE_RESOLUTION,
                            offset: reader.stream_position()?,
                        }
                        .into());
                    }

                    let mut resolution_box = ResolutionSuperBox::default();
                    resolution_box.length = box_length;
                    resolution_box.offset = reader.stream_position()?;
                    info!("ResolutionBox start at {:?}", resolution_box.offset);
                    resolution_box.decode(reader)?;
                    info!("ResolutionBox finish at {:?}", reader.stream_position()?);
                    self.resolution_box = Some(resolution_box);
                }

                BoxTypes::Unknown => {
                    warn!(
                        "Unknown box type 2 {:?} {:?}",
                        reader.stream_position(),
                        box_type
                    );
                    break;
                }

                // End of header but recognised new box type
                _ => {
                    reader.seek(io::SeekFrom::Current(-(header_length as i64)))?;
                    break;
                }
            }
        }

        // There shall be at least one Colour Specification box
        // within the JP2 Header box.
        if self.colour_specification_boxes.len() == 0 {
            return Err(JP2Error::BoxMalformed {
                box_type: BOX_TYPE_IMAGE_HEADER,
                offset: reader.stream_position()?,
            }
            .into());
        }

        // TODO
        // Check that all u16/i16 are correct / big endian is correct

        Ok(())
    }
}

const COMPRESSION_TYPE_WAVELET: u8 = 7;

// I.5.3.1
//
// Image Header box
//
// This box contains fixed length generic information about the image, such as
// the image size and number of components.
//
// The contents of the JP2 Header box shall start with an Image Header box.
//
// The length of the Image Header box shall be 22 bytes, including the box
// length and type fields.
//
// Much of the information within the Image Header box is redundant with
// information stored in the codestream itself.
//
// All references to “the codestream” in the descriptions of fields in this
// Image Header box apply to the codestream found in the first Contiguous
// Codestream box in the file.
//
// Files that contain contradictory information between the Image Headerbox and
// the first codestream are not conforming files. However, readers may choose
// to attempt to read these files by usingthe values found within the
// codestream.
#[derive(Debug, Default)]
pub struct ImageHeaderBox {
    length: u64,
    offset: u64,
    height: [u8; 4],
    width: [u8; 4],
    components_num: [u8; 2],
    components_bits: [u8; 1],
    compression_type: [u8; 1],
    colourspace_unknown: [u8; 1],
    intellectual_property: [u8; 1],
}

impl ImageHeaderBox {
    // Image area height.
    //
    // The value of this parameter indicates the height of the image area.
    // This field is stored as a 4-byte big endian unsigned integer.
    //
    // The value of this field shall be Ysiz – YOsiz, where Ysiz and YOsiz are
    // the values of the respective fields in the SIZ marker in the codestream.
    //
    // However, reference grid points are not necessarily square; the aspect
    // ratio of a reference grid point is specified by the Resolution box.
    //
    // If the Resolution box is not present, then a reader shall assume that
    // reference grid points are square.
    pub fn height(&self) -> u32 {
        u32::from_be_bytes(self.height)
    }

    // Image area width.
    //
    // The value of this parameter indicates the width of the image area.
    // This field is stored as a 4-byte big endian unsigned integer.
    //
    // The value of this field shall be Xsiz – XOsiz, where Xsiz and XOsiz are
    // the values of the respective fields in the SIZ marker in the codestream.
    //
    // However, reference grid points are not necessarily square; theaspect
    // ratio of a reference grid point is specified by the Resolution box.
    //
    // If the Resolution box is not present, then a reader shall assume that
    // reference grid points are square
    pub fn width(&self) -> u32 {
        u32::from_be_bytes(self.width)
    }

    // Number of components.
    //
    // This parameter specifies the number of components in the codestream and
    // is stored as a 2-byte big endian unsigned integer.
    //
    // The value of this field shall be equal to the value of the Csiz field in
    // the SIZ marker in the codestream.
    pub fn components_num(&self) -> u16 {
        u16::from_be_bytes(self.components_num)
    }

    // Bits per component.
    //
    // This parameter specifies the bit depth of the components in the
    // codestream, minus 1, and is stored as a 1-byte field.
    //
    // If the bit depth is the same for all components, then this parameter
    // specifies that bit depth and shall be equivalent to the values of the
    // Ssizi fields in the SIZ marker in the codestream (which shall all be
    // equal).
    //
    // If the components vary in bit depth, then the value of this field shall
    // be 255 and the JP2 Header box shall also contain a Bits Per Component
    // box defining the bit depth of each component.
    //
    // The low 7-bits of the value indicate the bit depth of the components.
    // The high-bit indicates whether the components are signed or unsigned.
    // If the high-bit is 1, then the components contain signed values.
    // If the high-bit is 0, then the components contain unsigned values
    pub fn components_bits(&self) -> u8 {
        // 1111 1111 (255) Components vary in bit depth
        // 1xxx xxxx (128 - 254) Components are signed values
        // 0xxx xxxx (37 - 127) Components are unsigned values
        if self.components_bits[0] == 255 {
            self.components_bits[0]
        }
        // x000 0000 — x010 0101 Component bit depth = value + 1. From 1 bit
        // deep through 38 bits deep respectively (counting the sign bit, if
        // appropriate)
        //
        else if self.components_bits[0] <= 37 {
            self.components_bits[0] + 1
        }
        // All other values reserved for ISO use.
        else {
            todo!("reserved");
        }
    }

    // Compression type.
    //
    // This parameter specifies the compression algorithm used to compress the
    // image data.
    //
    // The value of this field shall be 7.
    // It is encoded as a 1-byte unsigned integer.
    // Other values are reserved for ISO use.
    pub fn compression_type(&self) -> u8 {
        self.compression_type[0]
    }

    // Colourspace Unknown.
    //
    // This field specifies if the actual colourspace of the image data in the
    // codestream is known.
    //
    // This field is encoded as a 1-byte unsigned integer.
    //
    // Legal values for this field are 0, if the colourspace of the image is
    // known and correctly specified in the Colourspace Specification boxes
    // within the file, or 1, if the colourspace of the image is not known.
    //
    // A value of 1 will be used in cases such as the transcoding of legacy
    // images where the actual colourspace of the image data is not known.
    //
    // In those cases, while the colourspace interpretation methods specified
    // in the file may not accurately reproduce the image with respect to some
    // original, the image should be treated as if the methods do accurately
    // reproduce the image.
    //
    // Values other than 0 and 1 are reserved for ISO use.
    pub fn colourspace_unknown(&self) -> u8 {
        self.colourspace_unknown[0]
    }

    // Intellectual Property.
    //
    // This parameter indicates whether this JP2 file contains intellectual
    // property rights information.
    //
    // If the value of this field is 0, this file does not contain rights
    // information, and thus the file does not contain an IPR box.
    //
    // If the value is 1, then the file does contain rights information and
    // thus does contain an IPR box.
    //
    // Other values are reserved for ISO use.
    pub fn intellectual_property(&self) -> u8 {
        self.intellectual_property[0]
    }
}

impl JBox for ImageHeaderBox {
    // The type of the Image Header box shall be ‘ihdr’ (0x6968 6472)
    fn identifier(&self) -> BoxType {
        BOX_TYPE_IMAGE_HEADER
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn decode<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<(), Box<dyn error::Error>> {
        reader.read_exact(&mut self.height)?;
        reader.read_exact(&mut self.width)?;
        reader.read_exact(&mut self.components_num)?;
        reader.read_exact(&mut self.components_bits)?;
        reader.read_exact(&mut self.compression_type)?;
        reader.read_exact(&mut self.colourspace_unknown)?;
        reader.read_exact(&mut self.intellectual_property)?;

        Ok(())
    }
}

// I.5.3.6
//
// Channel Definition Box
//
// The Channel Definition box specifies the meaning of the samples in each
// channel in the image. The exact location of this box within the JP2 Header
// box may vary provided that it follows the Image Header box.
//
// The mapping between actual components from the codestream to channels is
// specified in the Component Mapping box.
//
// If the JP2 Header box does not contain a Component Mapping box, then a
// reader shall map component i to channel i, for all components in
// the codestream.
//
// This box contains an array of channel descriptions. For each description,
// three values are specified:
// - the index of the channel described by that association
// - the type of that channel
// - and the association of that channel with particular colours.
//
// This box may specify multiple descriptions for a single channel; however,
// the type value in each description for the same channel shall be the same in
// all descriptions.
//
// If a multiple component transform is specified within the codestream, the
// image must be in an RGB colourspace and the red, green and blue colours as
// channels 0, 1 and 2 in the codestream, respectively.
#[derive(Debug, Default)]
pub struct ChannelDefinitionBox {
    length: u64,
    offset: u64,
    channels: Vec<Channel>,
}

impl ChannelDefinitionBox {
    pub fn channels(&self) -> &Vec<Channel> {
        &self.channels
    }
}

#[derive(Debug, Default)]
pub struct Channel {
    // Channel index
    //
    // This field specifies the index of the channel for this description.
    //
    // The value of this field represents the index of the channel as defined
    // within the Component Mapping box (or the actual component from the
    // codestream if the file does not contain a Component Mapping box).
    //
    // This field isencoded as a 2-byte big endian unsigned integer.
    channel_index: [u8; 2],

    // Channel type
    //
    // This field specifies the type of the channel for this description.
    // The value of this field specifies the meaning of the decompressed
    // samples in this channel.
    //
    // This field is encoded as a 2-byte bigendian unsigned integer.
    channel_type: [u8; 2],

    // Channel association
    //
    // This field specifies the index of the colour for which this channel is
    // directly associated (or a special value to indicate the whole image or
    // the lack of an association).
    //
    // For example, if this channel is an opacity channel for the red channel
    // in an RGB colourspace, this field would specify the index of the colour
    // red.
    channel_association: [u8; 2],
}

impl Channel {
    pub fn channel_index(&self) -> u16 {
        u16::from_be_bytes(self.channel_index)
    }

    pub fn channel_type(&self) -> ChannelTypes {
        ChannelTypes::new(self.channel_type)
    }

    pub fn channel_type_u16(&self) -> u16 {
        return u16::from_be_bytes(self.channel_type);
    }

    // TODO: Map channel association based on colourspace (Table I-18)
    pub fn channel_association(&self) -> u16 {
        u16::from_be_bytes(self.channel_association)
    }
}

// TODO: There shall not be more than one channel in a JP2 file with a the same
// Typ^i and Asoc^i value pair, with the exception of Typ^i and Asoc^i values of
// 2^16 – 1 (not specified)

const CHANNEL_TYPE_COLOUR_IMAGE_DATA: u16 = 0;
const CHANNEL_TYPE_OPACITY_DATA: u16 = 1;
const CHANNEL_TYPE_PREMULTIPLIED_OPACITY: u16 = 3;

#[derive(Debug)]
pub enum ChannelTypes {
    ColourImageData,
    Opacity,
    PremultipliedOpacity,
    Reserved { value: u16 },
    Unspecified { value: u16 },
}

impl ChannelTypes {
    fn new(value: [u8; 2]) -> ChannelTypes {
        let channel_type = u16::from_be_bytes(value);

        if channel_type == 0 {
            ChannelTypes::ColourImageData
        } else if channel_type == 1 {
            ChannelTypes::Opacity
        } else if channel_type == 2 {
            ChannelTypes::PremultipliedOpacity
        } else if channel_type <= 2u16.pow(16) - 2 {
            ChannelTypes::Reserved {
                value: channel_type,
            }
        } else {
            ChannelTypes::Unspecified {
                value: channel_type,
            }
        }
    }
}

impl JBox for ChannelDefinitionBox {
    fn identifier(&self) -> BoxType {
        BOX_TYPE_CHANNEL_DEFINITION
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn decode<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<(), Box<dyn error::Error>> {
        // Number of channel descriptions. This field specifies the number of
        // channel descriptions in this box. This field is encoded as a 2-byte
        // big endian unsigned integer.
        let mut no_channel_descriptions: [u8; 2] = [0; 2];

        reader.read_exact(&mut no_channel_descriptions)?;

        let mut size = u16::from_be_bytes(no_channel_descriptions);

        let mut channels: Vec<Channel> = Vec::with_capacity(size as usize);

        while size > 0 {
            let mut channel = Channel::default();
            reader.read_exact(&mut channel.channel_index)?;
            reader.read_exact(&mut channel.channel_type)?;
            reader.read_exact(&mut channel.channel_association)?;

            debug!(
                "Found channel at index {:?} of type {:?} and association {:?}",
                channel.channel_index(),
                channel.channel_type(),
                channel.channel_association(),
            );

            channels.push(channel);

            size -= 1;
        }

        self.channels = channels;

        Ok(())
    }
}

const COMPONENT_MAP_TYPE_DIRECT: [u8; 1] = [1];
const COMPONENT_MAP_TYPE_PALETTE: [u8; 1] = [2];

#[derive(Debug)]
pub enum ComponentMapType {
    // Direct use.
    //
    // This channel is created directly from an actual component in the
    // codestream.
    // The index of the component mapped to this channel is specified in the
    // CMPi field for this channel.
    Direct,

    // Palette mapping.
    //
    // This channel is created by applying the palette to an actual component
    // in the codestream.
    //
    // The index of the component mapped into the palette is specified in the
    // CMPi field for this channel.
    // The column from the palette to use is specified in the PCOLi field for
    // this channel
    Palette,

    // Reserved for ISO use
    Reserved { value: [u8; 1] },
}

impl ComponentMapType {
    fn new(value: [u8; 1]) -> ComponentMapType {
        match value {
            COMPONENT_MAP_TYPE_DIRECT => ComponentMapType::Direct,
            COMPONENT_MAP_TYPE_PALETTE => ComponentMapType::Palette,
            value => ComponentMapType::Reserved { value },
        }
    }
}

#[derive(Debug)]
pub struct ComponentMap {
    // This field specifies the index of component from the codestream that is
    // mapped to this channel (either directly or through a palette).
    //
    // This field is encoded as a 2-byte big endian unsigned integer.
    component: [u8; 2],

    // This field specifies how this channel is generated from the actual
    // components in the file. This field is encoded as a 1-byte unsigned
    // integer.
    mapping_type: ComponentMapType,

    // This field specifies the index component from the palette that is used
    // to map the actual component from the codestream.
    // This field is encoded as a 1-byte unsigned integer.
    //
    // If the value of the MTYPi field for this channel is 0, then the value of
    // this field shall be 0.
    palette: [u8; 1],
}

impl ComponentMap {
    pub fn component(&self) -> u16 {
        u16::from_be_bytes(self.component)
    }
    pub fn mapping_type(&self) -> u8 {
        match self.mapping_type {
            ComponentMapType::Direct => COMPONENT_MAP_TYPE_DIRECT[0],
            ComponentMapType::Palette => COMPONENT_MAP_TYPE_PALETTE[0],
            ComponentMapType::Reserved { value } => value[0],
        }
    }
    pub fn palette(&self) -> u8 {
        self.palette[0]
    }
}

// I.5.3.5
//
// The Component Mapping box defines how image channels are identified from the
// actual components decoded from the codestream.
//
// This abstraction allows a single structure (the Channel Definition box) to
// specify the colour or type of both palettized images and non-palettized
// images.
//
// This box contains an array of CMPi, MTYPi and PCOLi fields.
//
// Each group of these fields represents the definition of one channel in the
// image.
//
// The channels are numbered in order starting with zero, and the number of
// channels specified in the Component Mapping box is determined by the length
// of the box.
//
// If the JP2 Header box contains a Palette box, then the JP2 Header box shall
// also contain a Component Mapping box.
// If the JP2 Header box does not contain a Palette box, then the JP2 Header box
// shall not contain a Component Mapping box.
// In this case, the components shall be mapped directly to channels, such that
// component i is mapped to channel i.
#[derive(Debug, Default)]
pub struct ComponentMappingBox {
    length: u64,
    offset: u64,
    mapping: Vec<ComponentMap>,
}

impl ComponentMappingBox {
    pub fn component_map(&self) -> &Vec<ComponentMap> {
        &self.mapping
    }
}

impl JBox for ComponentMappingBox {
    fn identifier(&self) -> BoxType {
        BOX_TYPE_COMPONENT_MAPPING
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn decode<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<(), Box<dyn error::Error>> {
        let mut index = 0;
        while index < self.length {
            let mut component_map = ComponentMap {
                component: [0; 2],
                palette: [0; 1],
                mapping_type: ComponentMapType::new([255]),
            };
            reader.read_exact(&mut component_map.component)?;

            let mut mapping_type: [u8; 1] = [0; 1];
            reader.read_exact(&mut mapping_type)?;
            component_map.mapping_type = ComponentMapType::new(mapping_type);

            reader.read_exact(&mut component_map.palette)?;

            self.mapping.push(component_map);
            index = index + 4;
        }

        Ok(())
    }
}

#[derive(Clone, Debug, Default)]
pub struct GeneratedComponent {
    // This parameter specifies the bit depth of generated component i,
    // encoded as a 1-byte big endian integer.
    //
    // The low 7-bits of the value indicate the bit depth of this component.
    // The high-bit indicates whether the component is signed or unsigned.
    //
    // If the high-bit is 1, then the component contains signed values.
    // If the high-bit is 0, then the component contains unsigned values.
    //
    // The number of Bi values shall be the same as the value of the NPC field.
    bit_depth: [u8; 1],

    // The generated component value for entry j for component i.
    //
    // Cji values are organized in component major order; all of the component
    // values for entry j are grouped together, followed by all of the entries
    // for component j+1.
    //
    // The size of Cji is the value specified by field Bi.
    //
    // The number of components shall be the same as the NPC field.
    //
    // The number of Cji values shall be the number of created components
    // (the NPC field) times the number of entries in the palette (NE).
    //
    // If the value of Bi is not a multiple of 8, then each Cji value is padded
    // with zeros to a multiple of 8 bits and the actual value shall be stored
    // in the low-order bits of the padded value.
    //
    // For example, if the value of Bi is 10 bits, then the individual Cji
    // values shall be stored in the low 10 bits of a 16 bit field.
    values: Vec<u8>,
}

pub enum BitDepth {
    Signed { value: u8 },
    Unsigned { value: u8 },
    Reserved { value: u8 },
}

impl BitDepth {
    fn new(byte: u8) -> BitDepth {
        // The low 7-bits of the value indicate the bit depth of this component.
        let value = u8::from_be_bytes([byte << 1 >> 1]) + 1;

        // The high-bit indicates whether the component is signed or unsigned.
        let signedness = byte >> 7;
        match signedness {
            //  If the high-bit is 1, then the component contains signed values
            1 => BitDepth::Unsigned { value },
            //  If the high-bit is 0, then the component contains unsigned values.
            0 => BitDepth::Signed { value },
            _ => BitDepth::Reserved { value },
        }
    }

    pub fn value(&self) -> u8 {
        match &self {
            Self::Signed { value } => *value,
            Self::Unsigned { value } => *value,
            Self::Reserved { value } => *value,
        }
    }
}

impl GeneratedComponent {
    pub fn bit_depth(&self) -> BitDepth {
        BitDepth::new(self.bit_depth[0])
    }

    pub fn values(&self) -> &Vec<u8> {
        &self.values
    }
}

// I.5.3.4
//
// The palette specified in this box is applied to a single component to
// convert it into multiple components.
//
// The colourspace of the components generated by the palette is then
// interpreted based on the values of the Colour Specification boxes in the JP2
// Header box in the file.
//
// The mapping of an actual component from the codestream through the palette
// is specified inthe Component Mapping box.
//
// If the JP2 Header box contains a Palette box, then it shall also contain a
// Component Mapping box.
//
// If the JP2 Header box does not contain a Palette box, then it shall not
// contain a Component Mapping box
#[derive(Debug, Default)]
pub struct PaletteBox {
    length: u64,
    offset: u64,

    // Number of entries in the table.
    //
    // This value shall be in the range 1 to 1024 and is encoded as a 2-byte
    // big endian unsigned integer.
    num_entries: [u8; 2],

    // Number of components created by the application of the palette.
    //
    // For example, if the palette turns a single index component into a
    // three-component RGB image, then the value of this field shall be 3.
    //
    // This field is encoded as a 1-byte unsigned integer
    num_components: [u8; 1],

    generated_components: Vec<GeneratedComponent>,
}

impl PaletteBox {
    pub fn num_entries(&self) -> u16 {
        u16::from_be_bytes(self.num_entries)
    }

    pub fn num_components(&self) -> u8 {
        self.num_components[0]
    }

    pub fn generated_components(&self) -> &Vec<GeneratedComponent> {
        &self.generated_components
    }
}

impl JBox for PaletteBox {
    fn identifier(&self) -> BoxType {
        BOX_TYPE_PALETTE
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn decode<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<(), Box<dyn error::Error>> {
        reader.read_exact(&mut self.num_entries)?;
        reader.read_exact(&mut self.num_components)?;

        let num_entries = self.num_entries() as usize;
        self.generated_components = vec![
            GeneratedComponent {
                bit_depth: [0],
                values: Vec::with_capacity(num_entries),
            };
            self.num_components() as usize
        ];
        for generated_component in &mut self.generated_components {
            reader.read_exact(&mut generated_component.bit_depth)?;
        }

        for generated_component in &mut self.generated_components {
            let mut j = 0;
            while j < num_entries {
                let mut entry: [u8; 1] = [0; 1];
                reader.read_exact(&mut entry)?;
                generated_component.values.push(entry[0]);
                j = j + 1;
            }
        }

        Ok(())
    }
}

// I.5.3.2
//
// The Bits Per Component box specifies the bit depth of each component.
//
// If the bit depth of all components in the codestream is the same (in both
// sign and precision), then this box shall not be found. Otherwise, this box
// specifies the bit depth of each individual component.
//
// The order of bit depth values in this box is the actual order in which those
// components are enumerated within the codestream.
//
// The exact location of this box within the JP2 Header box may vary provided
// that it follows the Image Header box.
#[derive(Debug, Default)]
pub struct BitsPerComponentBox {
    length: u64,
    offset: u64,
    components_num: u16,

    // Bits per component.
    //
    // This parameter specifies the bit depth of component i, minus 1, encoded
    // as a 1-byte value.
    //
    // The ordering of the components within the Bits Per Component Box shall
    // be the same as the ordering of the components within the codestream.
    //
    // The number of BP_Ci fields shall be the same as the value of the NC
    // field from the Image Header box.
    //
    // The value of this field shall be equivalent to the respective Ssiz_i
    // field in the SIZ marker in the codestream.
    bits_per_component: Vec<u8>,
}
impl BitsPerComponentBox {
    pub fn bits_per_component(&self) -> Vec<BitDepth> {
        self.bits_per_component
            .iter()
            .map(|byte| BitDepth::new(*byte))
            .collect()
    }
}

impl JBox for BitsPerComponentBox {
    fn identifier(&self) -> BoxType {
        BOX_TYPE_BITS_PER_COMPONENT
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn decode<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<(), Box<dyn error::Error>> {
        reader.read_exact(&mut self.bits_per_component)?;
        Ok(())
    }
}

type Method = [u8; 1];

const METHOD_ENUMERATED_COLOUR_SPACE: Method = [1];
const METHOD_ENUMERATED_RESTRICTED_ICC_PROFILE: Method = [2];

#[derive(Debug)]
pub enum Methods {
    EnumeratedColourSpace,
    RestrictedICCProfile,
    Reserved { value: Method },
}

impl fmt::Display for Methods {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Methods::EnumeratedColourSpace => write!(f, "{}", METHOD_ENUMERATED_COLOUR_SPACE[0]),
            Methods::RestrictedICCProfile => {
                write!(f, "{}", METHOD_ENUMERATED_RESTRICTED_ICC_PROFILE[0])
            }
            Methods::Reserved { value } => write!(f, "{}", value[0]),
        }
    }
}

impl Methods {
    fn new(value: [u8; 1]) -> Methods {
        match value {
            METHOD_ENUMERATED_COLOUR_SPACE => Methods::EnumeratedColourSpace,
            METHOD_ENUMERATED_RESTRICTED_ICC_PROFILE => Methods::RestrictedICCProfile,
            value => Methods::Reserved { value },
        }
    }
}

type EnumeratedColourSpace = [u8; 4];

const ENUMERATED_COLOUR_SPACE_UNKNOWN: EnumeratedColourSpace = [0, 0, 0, 0];
const ENUMERATED_COLOUR_SPACE_SRGB: EnumeratedColourSpace = [0, 0, 0, 16];
const ENUMERATED_COLOUR_SPACE_GREYSCALE: EnumeratedColourSpace = [0, 0, 0, 17];

#[derive(Debug)]
enum EnumeratedColourSpaces {
    #[allow(non_camel_case_types)]
    sRGB,
    Greyscale,
    Reserved,
}

impl EnumeratedColourSpaces {
    fn new(value: [u8; 4]) -> EnumeratedColourSpaces {
        match value {
            ENUMERATED_COLOUR_SPACE_SRGB => EnumeratedColourSpaces::sRGB,
            ENUMERATED_COLOUR_SPACE_GREYSCALE => EnumeratedColourSpaces::Greyscale,
            _ => EnumeratedColourSpaces::Reserved,
        }
    }
}

// I.5.3.3
//
// Colour Specification box
//
// Each Colour Specification box defines one method by which an application can
// interpret the colourspace of the decompressed image data. This colour
// specification is to be applied to the image data after it has been
// decompressed and after any reverse decorrelating component transform has been
// applied to the image data.
//
// A JP2 file may contain multiple Colour Specification boxes, but must contain
// at least one, specifying different methods for achieving “equivalent” results.
// A conforming JP2 reader shall ignore all Colour Specification boxes after the
// first. However, readers conforming to other standards may use those boxes as
// defined in those other standards
#[derive(Debug, Default)]
pub struct ColourSpecificationBox {
    length: u64,
    offset: u64,
    method: [u8; 1],
    precedence: [u8; 1],
    colourspace_approximation: [u8; 1],
    enumerated_colour_space: EnumeratedColourSpace,
    restricted_icc_profile: Vec<u8>,
}

impl ColourSpecificationBox {
    // Specification method.
    //
    // This field specifies the method used by this Colour Specification box to
    // define the colourspace of the decompressed image.
    //
    // This field is encoded as a 1-byte unsigned integer.
    // The value of this field shall be 1 or 2.
    //
    pub fn method(&self) -> Methods {
        Methods::new(self.method)
    }

    // Precedence.
    //
    // This field is reserved for ISO use and the value shall be set to zero;
    // however, conforming readers shall ignore the value of this field.
    //
    // This field is specified as a signed 1 byte integer
    pub fn precedence(&self) -> i8 {
        self.precedence[0] as i8
    }

    // Colourspace approximation.
    //
    // This field specifies the extent to which this colour specification method
    // approximates the “correct” definition of the colourspace.
    //
    // The value of this field shall be set to zero; however, conforming readers
    // shall ignore the value of this field.
    //
    // Other values are reserved forother ISO use.
    // This field is specified as 1 byte unsigned integer.
    pub fn colourspace_approximation(&self) -> u8 {
        self.colourspace_approximation[0]
    }

    // Enumerated colourspace.
    //
    // This field specifies the colourspace of the image using integer codes.
    //
    // To correctly interpret the colour of an image using an enumerated
    // colourspace, the application must know the definition of that
    // colourspace internally.
    //
    // This field contains a 4-byte big endian unsigned integer value
    // indicating the colourspace of the image.
    //
    // If the value of the METH field is 2, then the EnumCSfield shall not exist.
    pub fn enumerated_colour_space(&self) -> Option<u32> {
        Some(u32::from_be_bytes(self.enumerated_colour_space))
    }
}

impl JBox for ColourSpecificationBox {
    // The type of a Colour Specification box shall be ‘colr’ (0x636F 6C72).
    fn identifier(&self) -> BoxType {
        BOX_TYPE_COLOUR_SPECIFICATION
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn decode<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<(), Box<dyn error::Error>> {
        reader.read_exact(&mut self.method)?;
        reader.read_exact(&mut self.precedence)?;
        reader.read_exact(&mut self.colourspace_approximation)?;

        if self.precedence() != 0 {
            warn!("Precedence {:?} Unexpected", self.precedence());
        }
        if self.colourspace_approximation() != 0 {
            warn!(
                "Colourspace Approximation {:?} unexpected",
                self.colourspace_approximation()
            );
        }

        debug!("Method {:?}", self.method());
        debug!("Precedence {:?}", self.precedence());
        debug!(
            "ColourSpace Approximation {:?}",
            self.colourspace_approximation()
        );

        match self.method() {
            // 1 - Enumerated Colourspace.
            //
            // This colourspace specification box contains the enumerated value
            // of the colourspace of this image.
            //
            // The enumerated value is found in the EnumCS field in this box.
            // If the value of the METH field is 1, then the EnumCS shall exist
            // in this box immediately following the APPROX field, and the
            // EnumCS field shall be the last field in this box
            Methods::EnumeratedColourSpace => {
                // TODO: Validate this box exists if METH field is 1 and is
                // immediately following the APPROX field and the last field.
                reader.read_exact(&mut self.enumerated_colour_space)?;
                debug!("Enumerated Colour Space {:?}", self.enumerated_colour_space);
            }

            // 2 - Restricted ICC profile.
            // This Colour Specification box contains an ICC profile in the PROFILE field.
            //
            // This profile shall specify the transformation needed to convert the decompressed image data into the PCS_XYZ, and shall conform to either the Monochrome Input or Three-Component Matrix-Based Input profile class, and contain all the required tags specified therein, as defined in ICC.1:1998-09.
            //
            // As such, the value of the Profile Connection Space field in the profile header in the embedded profile shall be ‘XYZ\040’ (0x5859 5A20) indicating that the
            // output colourspace of the profile is in the XYZ colourspace.
            //
            // Any private tags in the ICC profile shall not change the visual appearance of an image processed using this ICC profile.
            //
            // The components from the codestream may have a range greater than the input range of the tone reproduction curve (TRC) of the ICC profile.
            //
            // Any decoded values should be clipped to the limits of the TRC before processing the image through the ICC profile.
            //
            // For example,
            // negative sample values of signed components may be clipped to zero before processing the image data through the profile.
            //
            // If the value of METH is 2, then the PROFILE field shall immediately follow the APPROX field and the PROFILE field shall be the last field in the box.
            Methods::RestrictedICCProfile => {
                let mut restricted_icc_profile: Vec<u8> = vec![0; self.length as usize - 3];

                reader.read_exact(&mut restricted_icc_profile)?;
                debug!("Restricted ICC Profile");
            }

            // Reserved for other ISO use. If the value of METH is not 1 or 2, there may be fields in this box following the APPROX field, and a conforming JP2 reader shall ignore the
            // entire Colour Specification box.
            Methods::Reserved { value } => {
                debug!("Reserved method {}", value[0]);
            }
        }

        Ok(())
    }
}

// I.5.3.7
//
// Resolution box (superbox)
//
// This box specifies the capture and default display grid resolutions of this
// image.
#[derive(Debug, Default)]
pub struct ResolutionSuperBox {
    length: u64,
    offset: u64,

    // Capture Resolution box.
    //
    // This box specifies the grid resolution at which this image was captured.
    capture_resolution_box: Option<CaptureResolutionBox>,

    // Default Display Resolution box.
    //
    // This box specifies the default grid resolution at which this image
    // should be displayed.
    default_display_resolution_box: Option<DefaultDisplayResolutionBox>,
}
impl ResolutionSuperBox {
    pub fn capture_resolution_box(&self) -> &Option<CaptureResolutionBox> {
        &self.capture_resolution_box
    }

    pub fn default_display_resolution_box(&self) -> &Option<DefaultDisplayResolutionBox> {
        &self.default_display_resolution_box
    }
}

impl JBox for ResolutionSuperBox {
    fn identifier(&self) -> BoxType {
        BOX_TYPE_RESOLUTION
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    // The type of a Resolution box shall be ‘res\040’ (0x7265 7320).
    fn decode<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<(), Box<dyn error::Error>> {
        loop {
            let BoxHeader {
                box_length,
                box_type,
                header_length,
            } = decode_box_header(reader)?;

            match BoxTypes::new(box_type) {
                BoxTypes::CaptureResolution => {
                    if self.capture_resolution_box.is_some() {
                        return Err(JP2Error::BoxUnexpected {
                            box_type: BOX_TYPE_CAPTURE_RESOLUTION,
                            offset: reader.stream_position()?,
                        }
                        .into());
                    }
                    let mut capture_resolution_box = CaptureResolutionBox::default();
                    capture_resolution_box.length = box_length;
                    capture_resolution_box.offset = reader.stream_position()?;
                    info!(
                        "CaptureResolutionBox start at {:?}",
                        capture_resolution_box.offset
                    );
                    capture_resolution_box.decode(reader)?;
                    info!(
                        "CaptureResolutionBox finish at {:?}",
                        reader.stream_position()?
                    );
                    self.capture_resolution_box = Some(capture_resolution_box);
                }
                BoxTypes::DefaultDisplayResolution => {
                    if self.default_display_resolution_box.is_some() {
                        return Err(JP2Error::BoxUnexpected {
                            box_type: BOX_TYPE_DEFAULT_DISPLAY_RESOLUTION,
                            offset: reader.stream_position()?,
                        }
                        .into());
                    }

                    let mut default_display_resolution_box = DefaultDisplayResolutionBox::default();
                    default_display_resolution_box.length = box_length;
                    default_display_resolution_box.offset = reader.stream_position()?;
                    info!(
                        "DisplayResolutionBox start at {:?}",
                        default_display_resolution_box.offset
                    );
                    default_display_resolution_box.decode(reader)?;
                    info!(
                        "DisplayResolutionBox finish at {:?}",
                        reader.stream_position()?
                    );
                    self.default_display_resolution_box = Some(default_display_resolution_box);
                }

                // End of capture resolution but recognised new box type
                _ => {
                    reader.seek(io::SeekFrom::Current(-(header_length as i64)))?;
                    break;
                }
            }
        }

        // If this box exists, it shall contain either a Capture Resolution box,
        // or a Default Display Resolution box, or both.
        if self.capture_resolution_box.is_none() && self.default_display_resolution_box.is_none() {
            return Err(JP2Error::BoxMalformed {
                box_type: BOX_TYPE_RESOLUTION,
                offset: self.offset,
            }
            .into());
        }

        Ok(())
    }
}

// I.6
//
// Intellectual Property box
//
// A box type for a box which is devoted to carrying intellectual property
// rights information within a JP2 file.
//
// Inclusion of this information in a JP2 file is optional for conforming files.
// The definition of the format of the contents of this box is reserved for ISO.
//
// However, the type of this box is defined as a means to allow applications to
// recognize the existence of IPR information.
//
// Use and interpretation of this information is beyond the scope of this.
#[derive(Debug, Default)]
struct IntellectualPropertyBox {
    length: u64,
    offset: u64,
    data: Vec<u8>,
}

impl JBox for IntellectualPropertyBox {
    // The type of the Intellectual Property Box shall be ‘jp2i’ (0x6A70 3269).
    fn identifier(&self) -> BoxType {
        BOX_TYPE_INTELLECTUAL_PROPERTY
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn decode<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<(), Box<dyn error::Error>> {
        self.data = vec![0; self.length as usize];
        reader.read_exact(&mut self.data)?;
        Ok(())
    }
}

// I.7.1
//
// XML box
//
// An XML box contains vendor specific information (in XML format) other than
// the information contained within boxes defined.
//
// There may be multiple XML boxes within the file, and those boxes may be found
// anywhere in the file except before the File Type box.
#[derive(Debug, Default)]
pub struct XMLBox {
    length: u64,
    offset: u64,
    xml: Vec<u8>,
}

impl XMLBox {
    pub fn format(&self) -> String {
        format!("{:?}", str::from_utf8(&self.xml).unwrap())
    }
}

impl JBox for XMLBox {
    // The type of an XML box is ‘xml\040’ (0x786D 6C20).
    fn identifier(&self) -> BoxType {
        BOX_TYPE_XML
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn decode<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<(), Box<dyn error::Error>> {
        self.xml = vec![0; self.length as usize];
        reader.read_exact(&mut self.xml)?;
        Ok(())
    }
}

// I.7.2
//
// UUID box
//
// A UUID box contains vendor specific information other than the information
// contained within boxes defined.
//
// There may be multiple UUID boxes within the file, and those boxes may be
// found anywhere in the file except before the File Type box.
#[derive(Debug, Default)]
pub struct UUIDBox {
    length: u64,
    offset: u64,
    uuid: [u8; 16],
    data: Vec<u8>,
}

impl UUIDBox {
    pub fn uuid(&self) -> &[u8; 16] {
        &self.uuid
    }
    pub fn data(&self) -> &Vec<u8> {
        &self.data
    }
}

impl JBox for UUIDBox {
    // The type of a UUID box shall be ‘uuid’ (0x7575 6964).
    fn identifier(&self) -> BoxType {
        BOX_TYPE_UUID
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn decode<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<(), Box<dyn error::Error>> {
        reader.read_exact(&mut self.uuid)?;
        self.data = vec![0; self.length as usize - self.uuid.len()];
        reader.read_exact(&mut self.data)?;

        Ok(())
    }
}

// I.7.3
//
// UUID Info box (superbox)
//
// While it is useful to allow vendors to extend JP2 files by adding information
// using UUID boxes, it is also useful to provide information in a standard form
// which can be used by non-extended applications to get more information about
// the extensions in the file. This information is contained in UUID Info boxes.
//
// A JP2 file may contain zero or more UUID Info boxes.
//
// These boxes may be found anywhere in the top level of the file (the superbox
// of a UUID Info box shall be the JP2 file itself) except before the File Type
// box.
//
// These boxes, if present, may not provide a complete index for the UUIDs in
// the file, may reference UUIDs not used in the file, and possibly may provide
// multiple references for the same UUID
#[derive(Debug, Default)]
pub struct UUIDInfoSuperBox {
    length: u64,
    offset: u64,
    uuid_list: Vec<UUIDListBox>,
    data_entry_url_box: Vec<DataEntryURLBox>,
}

impl JBox for UUIDInfoSuperBox {
    // The type of a UUID Info box shall be 'uinf' (0x7569 6E66)
    fn identifier(&self) -> BoxType {
        BOX_TYPE_UUID_INFO
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn decode<R: io::Read + io::Seek>(
        &mut self,
        _reader: &mut R,
    ) -> Result<(), Box<dyn error::Error>> {
        Ok(())
    }
}

// I.7.3.1
//
// UUID List box
//
// This box contains a list of UUIDs.
#[derive(Debug, Default)]
pub struct UUIDListBox {
    length: u64,
    offset: u64,

    // NU: Number of UUIDs.
    //
    // This field specifies the number of UUIDs found in this UUID List box.
    //
    // This field is encoded as a 2-byte big-endian unsigned integer.
    number_of_uuids: [u8; 2],

    // ID^i: ID
    //
    // This field specifies one UUID, as specified in ISO/IEC 11578, which
    // shall be associated with the URL contained in the URL box within the
    // same UUID Info box.
    //
    // The number of UUIDi fields shall be the same as the value of the NU
    // field.
    //
    // The value of this field shall be a 16-byte UUID
    ids: Vec<[u8; 16]>,
}

impl UUIDListBox {
    fn ids(&self) -> Vec<&str> {
        self.ids
            .iter()
            .map(|id| str::from_utf8(id).unwrap())
            .collect()
    }
    fn number_of_uuids(&self) -> i16 {
        i16::from_be_bytes(self.number_of_uuids)
    }
}

impl JBox for UUIDListBox {
    // The type of a UUID List box shall be ‘ulst’ (0x756C 7374)
    fn identifier(&self) -> BoxType {
        BOX_TYPE_UUID_LIST
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn decode<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<(), Box<dyn error::Error>> {
        reader.read_exact(&mut self.number_of_uuids)?;

        let mut size = self.number_of_uuids() as usize;

        self.ids = Vec::with_capacity(size);

        let mut buffer: [u8; 16] = [0; 16];
        while size > 0 {
            reader.read_exact(&mut buffer)?;
            self.ids.extend_from_slice(&[buffer]);
            size -= 1;
        }

        Ok(())
    }
}

// I.7.3.2
//
// Data Entry URL box
//
// This box contains a URL which can be used by an application to acquire more
// information about the associated vendor-specific extensions.
//
// The format of the information acquired through the use of this URL is not
// defined in this Recommendation | International Standard.
//
// The URL type should be of a service which delivers a file (e.g., URLs of
// type file, http, ftp, etc.), which ideally also permits random access.
//
// Relative URLs are permissible and are relative to the file containing this
// Data Entry URL box.
#[derive(Debug, Default)]
pub struct DataEntryURLBox {
    length: u64,
    offset: u64,

    // VERS: Version number.
    //
    // This field specifies the version number of the format of this box and is
    // encoded as a 1-byte unsigned integer.
    //
    // The value of this field shall be 0.
    version: [u8; 1],

    // FLAG: Flags.
    //
    // This field is reserved for other uses to flag particular attributes of
    // this box and is encoded as a 3-byte unsigned integer.
    //
    // The value of this field shall be 0.
    flags: [u8; 3],

    // LOC: Location.
    //
    // This field specifies the URL of the additional information associated
    // with the UUIDs contained in the UUID List box within the same UUID Info
    // superbox.
    //
    // The URL is encoded as a null terminated string of UTF-8 characters.
    location: Vec<u8>,
}

impl DataEntryURLBox {
    fn location(&self) -> Result<&str, str::Utf8Error> {
        str::from_utf8(&self.location)
    }
}

impl JBox for DataEntryURLBox {
    // The type of a Data Entry URL box shall be 'url\040' (0x7572 6C20).
    fn identifier(&self) -> BoxType {
        BOX_TYPE_DATA_ENTRY_URL
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn decode<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<(), Box<dyn error::Error>> {
        reader.read_exact(&mut self.version)?;
        reader.read_exact(&mut self.flags)?;

        // location
        let mut size = self.length() - 4;

        let mut buffer: [u8; 1] = [0; 1];
        while size > 0 {
            reader.read_exact(&mut buffer)?;
            self.location.extend_from_slice(&buffer);
            size -= 1;
        }

        Ok(())
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

// I.5.4
//
// Contiguous Codestream box
//
// The Contiguous Codestream box contains a valid and complete JPEG 2000
// codestream. When displaying the image, a conforming reader shall ignore all
// codestreams after the first codestream found in the file.
//
// Contiguous Codestream boxes may be found anywhere in the file
// except before the JP2 Header box.
#[derive(Debug, Default)]
pub struct ContiguousCodestreamBox {
    length: u64,
    pub offset: u64,
}

impl JBox for ContiguousCodestreamBox {
    // The type of a Contiguous Codestream box shall be ‘jp2c’
    fn identifier(&self) -> BoxType {
        BOX_TYPE_CONTIGUOUS_CODESTREAM
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn decode<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<(), Box<dyn error::Error>> {
        if self.length == 0 {
            reader.seek(io::SeekFrom::End(0))?;
            self.length = reader.stream_position()? - self.offset;
        } else {
            reader.seek(io::SeekFrom::Current(self.length as i64))?;
        }

        Ok(())
    }
}

// I.5.3.7.2
//
// Default Display Resolution box
//
// This box specifies a desired display grid resolution.
//
// For example, this may be used to determine the size of the image on a page
// when the image is placed in a page-layout program.
//
// However, this value is only a default. Each application must determine an
// appropriate display size for that application.
#[derive(Debug, Default)]
pub struct DefaultDisplayResolutionBox {
    length: u64,
    offset: u64,

    // Vertical Display grid resolution numerator.
    vertical_display_grid_resolution_numerator: [u8; 2],

    // Vertical Display grid resolution denominator.
    vertical_display_grid_resolution_denominator: [u8; 2],

    // Horizontal Display grid resolution numerator.
    horizontal_display_grid_resolution_numerator: [u8; 2],

    // Horizontal Display grid resolution denominator.
    horizontal_display_grid_resolution_denominator: [u8; 2],

    // Vertical Display grid resolution exponent.
    vertical_display_grid_resolution_exponent: [u8; 1],

    // Horizontal Display grid resolution exponent.
    horizontal_display_grid_resolution_exponent: [u8; 1],
}

impl DefaultDisplayResolutionBox {
    pub fn vertical_display_grid_resolution_numerator(&self) -> u16 {
        u16::from_be_bytes(self.vertical_display_grid_resolution_numerator)
    }
    pub fn vertical_display_grid_resolution_denominator(&self) -> u16 {
        u16::from_be_bytes(self.vertical_display_grid_resolution_denominator)
    }
    pub fn horizontal_display_grid_resolution_numerator(&self) -> u16 {
        u16::from_be_bytes(self.horizontal_display_grid_resolution_numerator)
    }
    pub fn horizontal_display_grid_resolution_denominator(&self) -> u16 {
        u16::from_be_bytes(self.horizontal_display_grid_resolution_denominator)
    }
    pub fn vertical_display_grid_resolution_exponent(&self) -> i8 {
        self.vertical_display_grid_resolution_exponent[0] as i8
    }
    pub fn horizontal_display_grid_resolution_exponent(&self) -> i8 {
        self.horizontal_display_grid_resolution_exponent[0] as i8
    }

    // VRd = VRdN/VRdD * 10^VRdE
    pub fn vertical_display_grid_resolution(&self) -> u64 {
        self.vertical_display_grid_resolution_numerator() as u64
            / self.vertical_display_grid_resolution_denominator() as u64
            * (10 as u64).pow(self.vertical_display_grid_resolution_exponent() as u32)
    }

    // HRd = HRdN/HRdD * 10^HRdE
    pub fn horizontal_display_grid_resolution(&self) -> u64 {
        self.horizontal_display_grid_resolution_numerator() as u64
            / self.horizontal_display_grid_resolution_denominator() as u64
            * (10 as u64).pow(self.horizontal_display_grid_resolution_exponent() as u32)
    }
}

impl JBox for DefaultDisplayResolutionBox {
    fn identifier(&self) -> BoxType {
        BOX_TYPE_DEFAULT_DISPLAY_RESOLUTION
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn decode<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<(), Box<dyn error::Error>> {
        reader.read_exact(&mut self.vertical_display_grid_resolution_numerator)?;
        reader.read_exact(&mut self.vertical_display_grid_resolution_denominator)?;

        reader.read_exact(&mut self.horizontal_display_grid_resolution_numerator)?;
        reader.read_exact(&mut self.horizontal_display_grid_resolution_denominator)?;

        reader.read_exact(&mut self.vertical_display_grid_resolution_exponent)?;
        reader.read_exact(&mut self.horizontal_display_grid_resolution_exponent)?;

        Ok(())
    }
}

// I.5.3.7.1
//
// This box specifies the grid resolution at which the source was digitized to
// create the image samples specified by thecodestream.
//
// For example, this may specify the resolution of the flatbed scanner that
// captured a page from a book. The capture grid resolution could also specify
// the resolution of an aerial digital camera or satellite camera.
#[derive(Debug, Default)]
pub struct CaptureResolutionBox {
    length: u64,
    offset: u64,

    // VRcN: Vertical Capture grid resolution numerator.
    //
    // This parameter specifies the VRcN value in which is used to calculate
    // the vertical capture grid resolution.
    //
    // This parameter is encoded as a 2-byte big endian unsigned integer.
    vertical_capture_grid_resolution_numerator: [u8; 2],

    // VRcD: Vertical Capture grid resolution denominator.
    //
    // This parameter specifies the VRcD value which is used to calculate the
    // vertical capture grid resolution.
    //
    // This parameter is encoded as a 2-byte big endian unsigned integer.
    vertical_capture_grid_resolution_denominator: [u8; 2],

    // HRcN: Horizontal Capture grid resolution numerator.
    //
    // This parameter specifies the HRcN value  which is used to calculate the
    // horizontal capture grid resolution.
    //
    // This parameter is encoded as a 2-byte big endian unsigned integer.
    horizontal_capture_grid_resolution_numerator: [u8; 2],

    // HRcD: Horizontal Capture grid resolution denominator.
    //
    // This parameter specifies the HRcD value in which is used to calculate
    // the horizontal capture grid resolution.
    //
    // This parameter is encoded as a 2-byte big endian unsigned integer.
    horizontal_capture_grid_resolution_denominator: [u8; 2],

    // VRcE: Vertical Capture grid resolution exponent.
    //
    // This parameter specifies the VRcE value which is used to calculate the
    // vertical capture grid resolution.
    //
    // This parameter is encoded as a twos-complement 1-byte signed integer.
    vertical_capture_grid_resolution_exponent: [u8; 1],

    // HRcE: Horizontal Capture grid resolution exponent.
    //
    // This parameter specifies the HRcE value in which is used to calculate
    // the horizontal capture grid resolution.
    //
    // This parameter is encoded as a twos-complement 1-byte signed integer.
    horizontal_capture_grid_resolution_exponent: [u8; 1],
}

impl CaptureResolutionBox {
    pub fn vertical_capture_grid_resolution_numerator(&self) -> u16 {
        u16::from_be_bytes(self.vertical_capture_grid_resolution_numerator)
    }
    pub fn vertical_capture_grid_resolution_denominator(&self) -> u16 {
        u16::from_be_bytes(self.vertical_capture_grid_resolution_denominator)
    }
    pub fn horizontal_capture_grid_resolution_numerator(&self) -> u16 {
        u16::from_be_bytes(self.horizontal_capture_grid_resolution_numerator)
    }
    pub fn horizontal_capture_grid_resolution_denominator(&self) -> u16 {
        u16::from_be_bytes(self.horizontal_capture_grid_resolution_denominator)
    }
    pub fn vertical_capture_grid_resolution_exponent(&self) -> i8 {
        self.vertical_capture_grid_resolution_exponent[0] as i8
    }
    pub fn horizontal_capture_grid_resolution_exponent(&self) -> i8 {
        self.horizontal_capture_grid_resolution_exponent[0] as i8
    }

    // VRc = (VRcN / VRcD) * 10^VRcE
    // The values VRc and HRc are always in reference grid points per meter.
    fn vertical_resolution_capture(&self) -> f64 {
        let mut vertical_resolution_capture: f64 = self.vertical_capture_grid_resolution_numerator()
            as f64
            / self.vertical_capture_grid_resolution_denominator() as f64;

        vertical_resolution_capture *=
            10_f64.powi(self.vertical_capture_grid_resolution_exponent() as i32);

        vertical_resolution_capture
    }

    // HRc = (HRcN / HRcD) * 10^HRcE
    // The values VRc and HRc are always in reference grid points per meter.
    fn horizontal_resolution_capture(&self) -> f64 {
        let mut horizontal_resolution_capture: f64 =
            self.horizontal_capture_grid_resolution_numerator() as f64
                / self.horizontal_capture_grid_resolution_denominator() as f64;

        horizontal_resolution_capture *=
            10_f64.powi(self.horizontal_capture_grid_resolution_exponent() as i32);

        horizontal_resolution_capture
    }
}

impl JBox for CaptureResolutionBox {
    // The type of a Capture Resolution box shall be ‘resc’ (0x7265 7363).
    fn identifier(&self) -> BoxType {
        BOX_TYPE_DEFAULT_DISPLAY_RESOLUTION
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn decode<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<(), Box<dyn error::Error>> {
        reader.read_exact(&mut self.vertical_capture_grid_resolution_numerator)?;
        reader.read_exact(&mut self.vertical_capture_grid_resolution_denominator)?;
        reader.read_exact(&mut self.horizontal_capture_grid_resolution_numerator)?;
        reader.read_exact(&mut self.horizontal_capture_grid_resolution_denominator)?;
        reader.read_exact(&mut self.vertical_capture_grid_resolution_exponent)?;
        reader.read_exact(&mut self.horizontal_capture_grid_resolution_exponent)?;

        Ok(())
    }
}

#[derive(Debug)]
pub struct JP2File {
    length: u64,
    signature: Option<SignatureBox>,
    file_type: Option<FileTypeBox>,
    header: Option<HeaderSuperBox>,
    contiguous_codestreams: Vec<ContiguousCodestreamBox>,
    xml: Vec<XMLBox>,
    uuid: Vec<UUIDBox>,
}

impl JP2File {
    pub fn length(&self) -> u64 {
        self.length
    }
    pub fn signature_box(&self) -> &Option<SignatureBox> {
        &self.signature
    }
    pub fn file_type_box(&self) -> &Option<FileTypeBox> {
        &self.file_type
    }
    pub fn header_box(&self) -> &Option<HeaderSuperBox> {
        &self.header
    }
    pub fn contiguous_codestreams_boxes(&self) -> &Vec<ContiguousCodestreamBox> {
        &self.contiguous_codestreams
    }
    pub fn xml_boxes(&self) -> &Vec<XMLBox> {
        &self.xml
    }
    pub fn uuid_boxes(&self) -> &Vec<UUIDBox> {
        &self.uuid
    }
}

struct BoxHeader {
    // Box Length
    //
    // This field specifies the length of the box, stored as a 4-byte big
    // endian unsigned integer.
    //
    // This value includes all of the fields of the box, including the length
    // and type.
    box_length: u64,

    // Box Type
    //
    // This field specifies the type of information found in the DBox field.
    //
    // The value of this field is encoded as a 4-byte big endian unsigned
    // integer. However, boxes are generally referred to by an ISO 646
    // character string translation of the integer value.
    //
    // For all box types defined box types will be indicated as both character
    // string (normative) and as 4-byte hexadecimal integers (informative).
    //
    // Also, a space character is shown in the character string translation of
    // the box type as “\040”.
    //
    // All values of TBox not defined are reserved for ISO use.
    box_type: [u8; 4],

    header_length: u8,
}

fn decode_box_header<R: io::Read + io::Seek>(
    reader: &mut R,
) -> Result<BoxHeader, Box<dyn error::Error>> {
    let mut header_length = 8;
    let mut box_length: [u8; 4] = [0; 4];
    let mut box_type: [u8; 4] = [0; 4];

    reader.read_exact(&mut box_length)?;

    let mut box_length_value = u32::from_be_bytes(box_length) as u64;
    if box_length_value == 0 {
        // If the value of this field is 0, then the length of the box was not known when the LBox field was written. In this case, this box contains all bytes up to the end of the file. If a box of length 0 is contained with in another box (its superbox), then the length of that superbox shall also be 0. This means that this box is the last box in the file.
        reader.read_exact(&mut box_type)?;
    } else if box_length_value == 1 {
        // If the value of this field is 1, then the XLBox field shall exist and the value of that field shall be the actual length of the box.
        reader.read_exact(&mut box_type)?;

        let mut xl_length: [u8; 8] = [0; 8];
        // This field specifies the actual length of the box if the value of the LBox field is 1.
        // This field is stored as an 8-byte big endian unsigned integer. The value includes all of the fields of thebox, including the LBox, TBox and XLBox fields
        reader.read_exact(&mut xl_length)?;

        box_length_value = u64::from_be_bytes(xl_length) - 16;
        header_length = 16;
    } else if box_length_value <= 7 {
        // The values 2–7 are reserved for ISO use.
        panic!("unsupported reserved box length {:?}", box_length_value);
    } else {
        reader.read_exact(&mut box_type)?;

        // Subtract LBox and TBox from length
        box_length_value = box_length_value - 8;
    }

    Ok(BoxHeader {
        box_length: box_length_value,
        box_type,
        header_length,
    })
}

// TODO: Consider lazy parsing where possible
pub fn decode_jp2<R: io::Read + io::Seek>(
    reader: &mut R,
) -> Result<JP2File, Box<dyn error::Error>> {
    let BoxHeader {
        box_length,
        box_type,
        header_length: _,
    } = decode_box_header(reader)?;

    // TODO: Enforce the following
    // Check Image Headerbox (header, width) with codestream and allow user to read it otherwise
    // If resolution box is not present, then a header shall assume that reference grid points are square.

    let mut signature_box = SignatureBox::default();
    // The Signature box shall be the first box
    if box_type != signature_box.identifier() {
        return Err(JP2Error::BoxUnexpected {
            box_type,
            offset: reader.stream_position()?,
        }
        .into());
    }
    signature_box.length = box_length;
    signature_box.offset = reader.stream_position().unwrap();
    info!("SignatureBox start at {:?}", signature_box.length);
    signature_box.decode(reader)?;
    info!("SignatureBox finish at {:?}", reader.stream_position()?);

    let BoxHeader {
        box_length,
        box_type,
        header_length: _,
    } = decode_box_header(reader)?;
    // The File Type box shall immediately follow the Signature box
    let mut file_type_box = FileTypeBox {
        length: box_length,
        offset: reader.stream_position().unwrap(),
        brand: [0; 4],
        min_version: [0; 4],
        compatibility_list: vec![],
    };
    if box_type != file_type_box.identifier() {
        return Err(JP2Error::BoxUnexpected {
            box_type,
            offset: reader.stream_position()?,
        }
        .into());
    }
    info!("FileTypeBox start at {:?}", file_type_box.offset);
    file_type_box.decode(reader)?;
    info!("FileTypeBox finish at {:?}", reader.stream_position()?);

    let mut header_box_option: Option<HeaderSuperBox> = None;
    let mut contiguous_codestream_boxes: Vec<ContiguousCodestreamBox> = vec![];

    let mut xml_boxes: Vec<XMLBox> = vec![];
    let mut uuid_boxes: Vec<UUIDBox> = vec![];
    let mut uuid_info_boxes: Vec<UUIDInfoSuperBox> = vec![];
    let mut current_uuid_info_box: Option<UUIDInfoSuperBox> = None;

    loop {
        let BoxHeader {
            box_length,
            box_type,
            header_length: _,
        } = match decode_box_header(reader) {
            Ok(value) => value,
            Err(derr) => {
                // TODO: Improve check for EOF
                if let Some(e) = derr.downcast_ref::<io::Error>() {
                    if e.kind() == io::ErrorKind::UnexpectedEof {
                        break;
                    }
                }
                return Err(derr);
            }
        };

        match BoxTypes::new(box_type) {
            BoxTypes::Header => {
                // The header box must be at the same level as the Signature
                // and File Type boxes it shall not be inside any other
                // superbox within the file)
                info!("HeaderSuperBox start at {:?}", reader.stream_position()?);
                let mut header_box = HeaderSuperBox::default();
                header_box.length = box_length;
                header_box.offset = reader.stream_position()?;
                header_box.decode(reader)?;
                header_box_option = Some(header_box);
                info!("HeaderSuperBox finish at {:?}", reader.stream_position()?);
            }
            BoxTypes::IntellectualProperty => {
                let mut intellectual_property_box = IntellectualPropertyBox {
                    length: box_length,
                    offset: reader.stream_position()?,
                    data: vec![0; box_length as usize],
                };
                info!(
                    "IntellectualPropertyBox start at {:?}",
                    intellectual_property_box.offset
                );
                intellectual_property_box.decode(reader)?;
                info!(
                    "IntellectualPropertyBox finish at {:?}",
                    reader.stream_position()
                );
            }
            BoxTypes::XML => {
                let mut xml_box = XMLBox {
                    length: box_length,
                    offset: reader.stream_position()?,
                    xml: Vec::with_capacity(box_length as usize).to_owned(),
                };
                info!("XMLBox start at {:?}", xml_box.offset);
                xml_box.decode(reader)?;
                xml_boxes.push(xml_box);
                info!("XMLBox finish at {:?}", reader.stream_position()?);
            }
            BoxTypes::UUID => {
                let mut uuid_box = UUIDBox::default();
                uuid_box.length = box_length;
                uuid_box.offset = reader.stream_position()?;
                info!("UUIDBox start at {:?}", uuid_box.offset);
                uuid_box.decode(reader)?;
                uuid_boxes.push(uuid_box);
                info!("UUIDBox finish at {:?}", reader.stream_position()?);
            }
            BoxTypes::UUIDInfo => {
                let mut uuid_info_box = UUIDInfoSuperBox::default();
                uuid_info_box.length = box_length;
                uuid_info_box.offset = reader.stream_position()?;
                info!("UUIDInfoBox start at {:?}", uuid_info_box.offset);
                uuid_info_box.decode(reader)?;

                if current_uuid_info_box.is_some() {
                    uuid_info_boxes.push(current_uuid_info_box.unwrap());
                }
                current_uuid_info_box = Some(uuid_info_box);
                info!("UUIDInfoBox finish at {:?}", reader.stream_position()?);
            }
            BoxTypes::UUIDList => {
                let mut uuid_list_box = UUIDListBox::default();
                uuid_list_box.length = box_length;
                uuid_list_box.offset = reader.stream_position()?;
                info!("UUIDListBox start at {:?}", uuid_list_box.offset);
                uuid_list_box.decode(reader)?;
                match &mut current_uuid_info_box {
                    Some(uuid_info_box) => {
                        uuid_info_box.uuid_list.push(uuid_list_box);
                    }
                    None => {
                        return Err(JP2Error::BoxMissing {
                            box_type: BOX_TYPE_UUID_INFO,
                        }
                        .into());
                    }
                }
                info!("UUIDListBox finish at {:?}", reader.stream_position()?);
            }
            BoxTypes::DataEntryURL => {
                let mut data_entry_url_box = DataEntryURLBox {
                    length: box_length,
                    offset: reader.stream_position()?,
                    version: [0; 1],
                    flags: [0; 3],
                    location: Vec::with_capacity(box_length as usize - 4).to_owned(),
                };

                data_entry_url_box.length = box_length;
                data_entry_url_box.offset = reader.stream_position()?;
                info!("DataEntryURLBox start at {:?}", data_entry_url_box.offset);
                data_entry_url_box.decode(reader)?;
                match &mut current_uuid_info_box {
                    Some(uuid_info_box) => {
                        uuid_info_box.data_entry_url_box.push(data_entry_url_box)
                    }
                    None => {
                        return Err(JP2Error::BoxMissing {
                            box_type: BOX_TYPE_UUID_INFO,
                        }
                        .into());
                    }
                }
                info!("DataEntryURLBox finish at {:?}", reader.stream_position()?);
            }
            BoxTypes::ContiguousCodestream => {
                // The Header box shall fall before the Contiguous Codestream box
                if header_box_option.is_none() {
                    return Err(JP2Error::BoxUnexpected {
                        box_type,
                        offset: reader.stream_position()?,
                    }
                    .into());
                }

                let mut continuous_codestream_box = ContiguousCodestreamBox {
                    length: box_length,
                    offset: reader.stream_position()?,
                };
                info!(
                    "ContiguousCodestreamBox start at {:?}",
                    continuous_codestream_box.offset
                );
                continuous_codestream_box.decode(reader)?;
                info!(
                    "ContiguousCodestreamBox finish at {:?}",
                    reader.stream_position()?
                );
                contiguous_codestream_boxes.push(continuous_codestream_box);
            }

            _ => {
                panic!(
                    "Unexpected box type {:?} {:?}",
                    reader.stream_position(),
                    box_type
                );
            }
        }
    }

    if current_uuid_info_box.is_some() {
        uuid_info_boxes.push(current_uuid_info_box.unwrap());
    }

    let result = JP2File {
        length: reader.stream_position()?,
        signature: Some(signature_box),
        file_type: Some(file_type_box),
        header: header_box_option,
        contiguous_codestreams: contiguous_codestream_boxes,
        xml: xml_boxes,
        uuid: uuid_boxes,
    };

    Ok(result)
}
