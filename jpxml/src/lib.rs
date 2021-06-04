#![allow(dead_code)]

use core::fmt::Write;
use jp2::{
    BitsPerComponentBox, CaptureResolutionBox, ChannelDefinitionBox, ColourSpecificationBox,
    ComponentMappingBox, ContiguousCodestreamBox, DefaultDisplayResolutionBox, FileTypeBox,
    HeaderBox, JBox, JP2File, PaletteBox, ResolutionBox, SignatureBox, UUIDBox, XMLBox,
};
use std::error;
use std::fmt;
use std::io;
use std::str;

fn to_hex<'a, I>(iter: I) -> Result<String, Box<dyn error::Error>>
where
    I: Iterator<Item = &'a u8>,
{
    let mut hex = String::new();
    for byte in iter {
        write!(hex, "{:02x}", byte)?;
    }
    return Ok(hex);
}

#[derive(Debug)]
enum JPXMLError {
    InvalidRepresentation { representation: String },
}

impl error::Error for JPXMLError {}
impl fmt::Display for JPXMLError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::InvalidRepresentation { representation } => {
                write!(f, "invalid representation {:?}", representation)
            }
        }
    }
}

// The JPXML document is generated from an image file format and/or codestreams,
// and its kind varies from none property to including codestream data
// representations.
//
// When kinds of image property representation are included, the JPXML document
// is categorized with three levels of representation:
// - "skeleton"
// - "fat-skeleton"
// - and "fat" representations.
#[derive(Debug, PartialEq)]
pub enum Representation {
    // The first-level representation, the skeleton representation, shall
    // express only the structure of the image itself, and may contain an
    // attribute for the absolute offset or the location path to the element
    // block.
    //
    // The skeleton shall have no text node in the JPXML elements.
    //
    // This representation is used for a location path that is comparatively
    // robust for changing the box structure of the image and/or marker
    // segment structure of the codestream.
    Skeleton,

    // The second-level representation, the fat-skeleton representation,
    // expresses the image structure and some variables of box and/or marker
    // contents.
    //
    // The fat skeleton is an intermediate representation between skeleton and
    // fat representations. Consequently, it also has the skeleton's attribute
    // and the same text node value of JPXML elements, but no binary data
    // (such as a coded codestream).
    //
    // This representation is used for a location path and also some image
    // transformation with XSLT.
    FatSkeleton,

    // The third-and final level representation, the fat representation,
    // expresses the image structure and whole image property values. This
    // whole property may represent a binarized format for use of some
    // applications, such as secure purpose.
    //
    // The binarized contents are translated with MIME's base64 encoding.
    //
    // As this representation requires more data space than the original image
    // data, it is unsuited for use in a storage file format for image data.
    Fat,
}

impl str::FromStr for Representation {
    type Err = Box<dyn error::Error>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "skeleton" => Ok(Representation::Skeleton),
            "fat-skeleton" => Ok(Representation::FatSkeleton),
            "fat" => Ok(Representation::Fat),
            _ => Err(JPXMLError::InvalidRepresentation {
                representation: s.to_owned(),
            }
            .into()),
        }
    }
}

// The "jP\040\040" box type is converted to a " jP__" element name, and
// other 4CC box types are used for the element names.
fn encode_signature_box<W: io::Write>(
    writer: &mut W,
    signature_box: &SignatureBox,
) -> Result<(), Box<dyn error::Error>> {
    write!(
        writer,
        "  <xjp:jP__ type=\"xjp:box\" length=\"{}\" offset=\"{}\">\n",
        signature_box.length(),
        signature_box.offset()
    )?;
    write!(
        writer,
        "    <xjp:signature length=\"8\" type=\"xs:hexbyte\">{}</xjp:signature>\n",
        to_hex(signature_box.signature().iter())?
    )?;
    writer.write(b"  </xjp:jP__>\n")?;
    Ok(())
}

fn encode_file_type_box<W: io::Write>(
    writer: &mut W,
    file_type_box: &FileTypeBox,
) -> Result<(), Box<dyn error::Error>> {
    write!(
        writer,
        "  <xjp:ftyp type=\"xjp:box\" length=\"{}\" offset=\"{}\">\n",
        file_type_box.length(),
        file_type_box.offset()
    )?;
    write!(
        writer,
        "    <xjp:brand length=\"4\" type=\"xs:hexbyte\">{}</xjp:brand>\n",
        file_type_box.brand()
    )?;
    write!(
        writer,
        "    <xjp:version length=\"4\" type=\"xs:int\">{}</xjp:version>\n",
        file_type_box.min_version()
    )?;

    for compatibility in file_type_box.compatibility_list() {
        write!(
            writer,
            "    <xjp:compatibility length=\"4\" type=\"xs:string\">{}</xjp:commpatibility>\n",
            compatibility
        )?;
    }
    writer.write(b"  </xjp:ftyp>\n")?;
    Ok(())
}

fn encode_header_box<W: io::Write>(
    writer: &mut W,
    header_box: &HeaderBox,
) -> Result<(), Box<dyn error::Error>> {
    let image_header_box = &header_box.image_header_box;

    write!(
        writer,
        "  <xjp:jp2h type=\"xjp:box\" length=\"{}\" offset=\"{}\">\n",
        header_box.length(),
        header_box.offset()
    )?;
    write!(
        writer,
        "    <xjp:ihdr type=\"xjp:box\" length=\"{}\" offset=\"{}\">\n",
        image_header_box.length(),
        image_header_box.offset()
    )?;
    write!(
        writer,
        "      <xjp:height type=\"xs:int\" length=\"4\">{}</xjp:height>\n",
        image_header_box.height()
    )?;
    write!(
        writer,
        "      <xjp:width type=\"xs:int\" length=\"4\">{}</xjp:width>\n",
        image_header_box.width()
    )?;
    write!(
        writer,
        "      <xjp:num_components type=\"xs:int\" length=\"1\">{}</xjp:num_components>\n",
        image_header_box.components_num()
    )?;
    write!(
        writer,
        "      <xjp:depth type=\"xs:int\" length=\"2\">{}</xjp:depth>\n",
        image_header_box.components_bits()
    )?;
    write!(
        writer,
        "      <xjp:compression type=\"xs:int\" length=\"1\">{}</xjp:compression>\n",
        image_header_box.compression_type()
    )?;
    write!(
        writer,
        "      <xjp:colour_unknown type=\"xs:int\" length=\"1\">{}</xjp:colour_unknown>\n",
        image_header_box.colourspace_unknown()
    )?;
    write!(
        writer,
        "      <xjp:ipr type=\"xs:boolean\" length=\"1\">{}</xjp:ipr>\n",
        image_header_box.intellectual_property()
    )?;
    writer.write(b"    </xjp:ihdr>\n")?;

    if let Some(bits_per_component_box) = &header_box.bits_per_component_box {
        encode_bits_per_component_box(writer, bits_per_component_box)?;
    }

    for colour_specification_box in &header_box.colour_specification_boxes {
        encode_colour_specification_box(writer, colour_specification_box)?;
    }

    if let Some(palette_box) = &header_box.palette_box {
        encode_palette_box(writer, palette_box)?;
    }
    if let Some(component_mapping_box) = &header_box.component_mapping_box {
        encode_component_mapping_box(writer, component_mapping_box)?;
    }
    if let Some(channel_definition_box) = &header_box.channel_definition_box {
        encode_channel_definition_box(writer, channel_definition_box)?;
    }
    if let Some(resolution_box) = &header_box.resolution_box {
        encode_resolution_box(writer, resolution_box)?;
    }

    writer.write(b"  </xjp:jp2h>\n")?;
    Ok(())
}

fn encode_bits_per_component_box<W: io::Write>(
    writer: &mut W,
    bits_per_component_box: &BitsPerComponentBox,
) -> Result<(), Box<dyn error::Error>> {
    write!(
        writer,
        "  <xjp:bpcc type=\"xjp:box\" length=\"{}\" offset=\"{}\">\n",
        bits_per_component_box.length(),
        bits_per_component_box.offset(),
    )?;

    for component_bit_depth in bits_per_component_box.bits_per_component() {
        write!(
            writer,
            "    <xjp:depth length=\"1\" type=\"xs:int\">{}</xjp:depth>\n",
            component_bit_depth.value()
        )?;
    }

    write!(writer, "    </xjp:ihdr>\n")?;
    Ok(())
}

fn encode_colour_specification_box<W: io::Write>(
    writer: &mut W,
    colour_specification_box: &ColourSpecificationBox,
) -> Result<(), Box<dyn error::Error>> {
    write!(
        writer,
        "    <xjp:colr type=\"xjp:box\" length=\"{}\" offset=\"{}\">\n",
        colour_specification_box.length(),
        colour_specification_box.offset(),
    )?;
    write!(
        writer,
        "      <xjp:method length=\"1\" type=\"xs:int\">{}</xjp:method>\n",
        colour_specification_box.method()
    )?;
    write!(
        writer,
        "      <xjp:precedence length=\"1\" type=\"xs:int\">{}</xjp:precedence>\n",
        colour_specification_box.precedence()
    )?;
    write!(
        writer,
        "      <xjp:approx length=\"1\" type=\"xs:int\">{}</xjp:approx>\n",
        colour_specification_box.colourspace_approximation()
    )?;
    if let Some(enumerated_colour_space) = colour_specification_box.enumerated_colour_space() {
        write!(
            writer,
            "      <xjp:colour length=\"4\" type=\"xs:int\">{}</xjp:colour>\n",
            enumerated_colour_space
        )?;
    }
    writer.write(b"    </xjp:colr>\n")?;
    Ok(())
}

fn encode_palette_box<W: io::Write>(
    writer: &mut W,
    palette_box: &PaletteBox,
) -> Result<(), Box<dyn error::Error>> {
    write!(
        writer,
        "    <xjp:pclr type=\"xjp:box\" length=\"{}\" offset=\"{}\">\n",
        palette_box.length(),
        palette_box.offset(),
    )?;
    write!(
        writer,
        "      <xjp:num_entries length=\"2\" type=\"xs:int\">{}</xjp:num_entries>\n",
        palette_box.num_entries()
    )?;
    write!(
        writer,
        "      <xjp:num_components length=\"1\" type=\"xs:int\">{}</xjp:num_components>\n",
        palette_box.num_components()
    )?;

    for generated_component in palette_box.generated_components() {
        write!(
            writer,
            "      <xjp:depth length=\"1\" type=\"xs:int\">{}</xjp:depth>\n",
            generated_component.bit_depth().value()
        )?;
        write!(
            writer,
            "      <xjp:data length=\"1\" type=\"xs:int\">{}</xjp:data>\n",
            to_hex(generated_component.values().iter())?
        )?;
    }
    writer.write(b"    </xjp:pclr>\n")?;
    Ok(())
}

fn encode_component_mapping_box<W: io::Write>(
    writer: &mut W,
    component_mapping_box: &ComponentMappingBox,
) -> Result<(), Box<dyn error::Error>> {
    write!(
        writer,
        "    <xjp:cmap type=\"xjp:box\" length=\"{}\" offset=\"{}\">\n",
        component_mapping_box.length(),
        component_mapping_box.offset(),
    )?;

    for component_map in component_mapping_box.component_map() {
        // TODO: Verify schema
        write!(writer, "      <xjp:mapc type=\"xjp:mapc\">\n")?;
        write!(
            writer,
            "        <xjp:component length=\"2\" type=\"xs:int\">{}</xjp:component>\n",
            component_map.component()
        )?;
        write!(
            writer,
            "        <xjp:mtype length=\"1\" type=\"xs:int\">{}</xjp:mtype>\n",
            component_map.mapping_type()
        )?;
        write!(
            writer,
            "        <xjp:palette length=\"1\" type=\"xs:int\">{}</xjp:palette>\n",
            component_map.palette()
        )?;
        write!(writer, "      </xjp:mapc>\n")?;
    }

    writer.write(b"    </xjp:cmap>\n")?;
    Ok(())
}
fn encode_channel_definition_box<W: io::Write>(
    writer: &mut W,
    channel_definition_box: &ChannelDefinitionBox,
) -> Result<(), Box<dyn error::Error>> {
    write!(
        writer,
        "    <xjp:cdef type=\"xjp:box\" length=\"{}\" offset=\"{}\">\n",
        channel_definition_box.length(),
        channel_definition_box.offset()
    )?;
    write!(
        writer,
        "      <xjp:num_entries length=\"2\" type=\"xs:int\">{}</xjp:num_entries>\n",
        channel_definition_box.channels().len()
    )?;
    for channel in channel_definition_box.channels() {
        write!(
            writer,
            "      <xjp:index length=\"2\" type=\"xs:int\">{}</xjp:index>\n",
            channel.channel_index()
        )?;
        write!(
            writer,
            "      <xjp:type length=\"2\" type=\"xs:int\">{}</xjp:type>\n",
            channel.channel_type_u16()
        )?;
        write!(
            writer,
            "      <xjp:assoc length=\"2\" type=\"xs:int\">{}</xjp:assoc>\n",
            channel.channel_association()
        )?;
    }
    writer.write(b"    </xjp:cdef>\n")?;
    Ok(())
}

fn encode_resolution_box<W: io::Write>(
    writer: &mut W,
    resolution_box: &ResolutionBox,
) -> Result<(), Box<dyn error::Error>> {
    write!(
        writer,
        "  <xjp:res_ type=\"xjp:box\" length=\"{}\" offset=\"{}\">\n",
        resolution_box.length(),
        resolution_box.offset()
    )?;

    if let Some(capture_resolution_box) = resolution_box.capture_resolution_box() {
        encode_capture_resolution_box(writer, capture_resolution_box)?;
    }
    if let Some(default_display_resolution_box) = resolution_box.default_display_resolution_box() {
        encode_default_display_resolution_box(writer, default_display_resolution_box)?;
    }

    writer.write(b"  </xjp:res_>\n")?;
    Ok(())
}

fn encode_capture_resolution_box<W: io::Write>(
    writer: &mut W,
    capture_resolution_box: &CaptureResolutionBox,
) -> Result<(), Box<dyn error::Error>> {
    write!(
        writer,
        "    <xjp:resc type=\"xjp:box\" length=\"{}\" offset=\"{}\">\n",
        capture_resolution_box.length(),
        capture_resolution_box.offset()
    )?;
    write!(
        writer,
        "      <xjp:vert_num length=\"2\" type=\"xs:int\">{}</xjp:id>\n",
        capture_resolution_box.vertical_capture_grid_resolution_numerator()
    )?;
    write!(
        writer,
        "      <xjp:vert_den length=\"2\" type=\"xs:int\">{}</xjp:id>\n",
        capture_resolution_box.vertical_capture_grid_resolution_denominator()
    )?;
    write!(
        writer,
        "      <xjp:hori_num length=\"2\" type=\"xs:int\">{}</xjp:id>\n",
        capture_resolution_box.horizontal_capture_grid_resolution_numerator()
    )?;
    write!(
        writer,
        "      <xjp:hori_den length=\"2\" type=\"xs:int\">{}</xjp:id>\n",
        capture_resolution_box.horizontal_capture_grid_resolution_denominator()
    )?;
    write!(
        writer,
        "      <xjp:vert_exp length=\"1\" type=\"xs:int\">{}</xjp:id>\n",
        capture_resolution_box.vertical_capture_grid_resolution_exponent()
    )?;
    write!(
        writer,
        "      <xjp:hori_exp length=\"1\" type=\"xs:int\">{}</xjp:id>\n",
        capture_resolution_box.horizontal_capture_grid_resolution_exponent()
    )?;
    writer.write(b"    </xjp:resc>\n")?;
    Ok(())
}

fn encode_default_display_resolution_box<W: io::Write>(
    writer: &mut W,
    default_display_resolution_box: &DefaultDisplayResolutionBox,
) -> Result<(), Box<dyn error::Error>> {
    write!(
        writer,
        "    <xjp:resd type=\"xjp:box\" length=\"{}\" offset=\"{}\">\n",
        default_display_resolution_box.length(),
        default_display_resolution_box.offset()
    )?;
    write!(
        writer,
        "      <xjp:vert_num length=\"2\" type=\"xs:int\">{}</xjp:id>\n",
        default_display_resolution_box.vertical_display_grid_resolution_numerator()
    )?;
    write!(
        writer,
        "      <xjp:vert_den length=\"2\" type=\"xs:int\">{}</xjp:id>\n",
        default_display_resolution_box.vertical_display_grid_resolution_denominator()
    )?;
    write!(
        writer,
        "      <xjp:hori_num length=\"2\" type=\"xs:int\">{}</xjp:id>\n",
        default_display_resolution_box.horizontal_display_grid_resolution_numerator()
    )?;
    write!(
        writer,
        "      <xjp:hori_den length=\"2\" type=\"xs:int\">{}</xjp:id>\n",
        default_display_resolution_box.horizontal_display_grid_resolution_denominator()
    )?;
    write!(
        writer,
        "      <xjp:vert_exp length=\"1\" type=\"xs:int\">{}</xjp:id>\n",
        default_display_resolution_box.vertical_display_grid_resolution_exponent()
    )?;
    write!(
        writer,
        "      <xjp:hori_exp length=\"1\" type=\"xs:int\">{}</xjp:id>\n",
        default_display_resolution_box.horizontal_display_grid_resolution_exponent()
    )?;

    writer.write(b"    </xjp:resd>\n")?;
    Ok(())
}

fn encode_contiguous_codestream<W: io::Write>(
    writer: &mut W,
    representation: &Representation,
    contiguous_codestream: &ContiguousCodestreamBox,
) -> Result<(), Box<dyn error::Error>> {
    write!(
        writer,
        "  <xjp:jp2c type=\"xjp:box\" length=\"{}\" offset=\"{}\">\n",
        contiguous_codestream.length(),
        contiguous_codestream.offset()
    )?;

    if *representation != Representation::Skeleton {
        todo!();
    }

    writer.write(b"  </xjp:jp2c>\n")?;
    Ok(())
}

fn encode_xml_box<W: io::Write>(
    writer: &mut W,
    xml_box: &XMLBox,
) -> Result<(), Box<dyn error::Error>> {
    write!(
        writer,
        "  <xjp:_xml_ type=\"xjp:box\" length=\"{}\" offset=\"{}\">\n",
        xml_box.length(),
        xml_box.offset()
    )?;
    writer.write(b"    <![CDATA[")?;
    write!(writer, "{}", xml_box.format())?;
    writer.write(b"]]>\n")?;
    writer.write(b"  </xjp:_xml_>\n")?;
    Ok(())
}

fn encode_uuid_box<W: io::Write>(
    writer: &mut W,
    uuid_box: &UUIDBox,
) -> Result<(), Box<dyn error::Error>> {
    write!(
        writer,
        "  <xjp:uuid type=\"xjp:box\" length=\"{}\" offset=\"{}\">\n",
        uuid_box.length(),
        uuid_box.offset()
    )?;
    write!(
        writer,
        "    <xjp:id length=\"16\" type=\"xs:int\">{}</xjp:id>\n",
        u128::from_be_bytes(*uuid_box.uuid())
    )?;
    write!(
        writer,
        "    <xjp:data length=\"{}\" type=\"xs:hexbyte\">{}</xjp:data>\n",
        uuid_box.data().len(),
        to_hex(uuid_box.data().iter())?
    )?;
    writer.write(b"  </xjp:uuid>\n")?;
    Ok(())
}

// The JPXML document is described with three elements; a JPXML element, its
// attribute, and its content value.
//
// The JPXML element structure represents an image structure;box, marker
// segment, and content structure.
//
// This document namespace shall be "http://www.iso.org/jpeg/jpxml/1.0", and this document's root element name shall be 'jpxml'.
//
// The JPXML element has two types;
// - the first element is a container element which expresses a box or a marker segment itself
// - and the second one is a content element which expresses a container's property or a box content.
//
// Some containers, such as a superbox, contain other containers, and so a JPXML document will have a tree structure.
pub fn encode_jpxml<W: io::Write>(
    writer: &mut W,
    file: &JP2File,
    representation: Representation,
    name: &str,
) -> Result<(), Box<dyn error::Error>> {
    writer.write(b"<?xml version=\"1.0\"?>\n")?;
    writer.write(b"<xjp:jpxml xmlns:jpxml=\"http://www.jpeg.org/jpxml/1.0\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\"")?;
    // Length is required?
    if name.len() > 0 {
        write!(writer, " xjp:length=\"{}\"", file.length())?;
        write!(writer, " xjp:name=\"{}\"", name)?;
    }
    writer.write(b">\n")?;

    if let Some(signature_box) = file.signature_box() {
        encode_signature_box(writer, signature_box)?;
    }

    if let Some(file_type_box) = file.file_type_box() {
        encode_file_type_box(writer, file_type_box)?;
    }

    // TODO: Check if header box is optional
    if let Some(header_box) = file.header_box() {
        encode_header_box(writer, header_box)?;
    }

    for contiguous_codestream in file.contiguous_codestreams_boxes() {
        encode_contiguous_codestream(writer, &representation, contiguous_codestream)?;
    }

    for xml_box in file.xml_boxes() {
        encode_xml_box(writer, xml_box)?;
    }
    for uuid_box in file.uuid_boxes() {
        encode_uuid_box(writer, uuid_box)?;
    }
    writer.write(b"</xjp:jpxml>\n")?;

    // Each JPXML element shall have 'length' and' type' attributes, and these attributes denote the byte length and datatype of each data chunk, respectively.
    // The content value may be described with text, and its data type denoted with 'type' attribute.
    Ok(())
}
