#![allow(dead_code)]

use core::fmt::Write;
use jp2::colour_specification::ColourSpecificationBox;
use jp2::{
    decode_jp2, BitsPerComponentBox, CaptureResolutionBox, ChannelDefinitionBox,
    ComponentMappingBox, ContiguousCodestreamBox, DefaultDisplayResolutionBox, FileTypeBox,
    HeaderSuperBox, JBox, PaletteBox, ResolutionSuperBox, SignatureBox, UUIDBox, XMLBox,
};
use jpc::{
    decode_jpc, CodingStyleMarkerSegment, CodingStyleParameters, ContiguousCodestream, Header,
    ImageAndTileSizeMarkerSegment, QuantizationDefaultMarkerSegment,
};
use std::error;
use std::fmt;
use std::fs::File;
use std::io::{self, BufReader, Seek};
use std::str;

fn to_hex<'a, I>(iter: I) -> Result<String, Box<dyn error::Error>>
where
    I: Iterator<Item = &'a u8>,
{
    let mut hex = String::new();
    for byte in iter {
        write!(hex, "{:02x}", byte)?;
    }
    Ok(hex)
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
    writeln!(
        writer,
        "  <xjp:jP__ type=\"box\" length=\"{}\" offset=\"{}\">",
        signature_box.length(),
        signature_box.offset()
    )?;
    writeln!(
        writer,
        "    <xjp:signature length=\"8\" type=\"hexbyte\">{}</xjp:signature>",
        to_hex(signature_box.signature().iter())?
    )?;
    writer.write_all(b"  </xjp:jP__>\n")?;
    Ok(())
}

fn encode_file_type_box<W: io::Write>(
    writer: &mut W,
    file_type_box: &FileTypeBox,
) -> Result<(), Box<dyn error::Error>> {
    writeln!(
        writer,
        "  <xjp:ftyp type=\"box\" length=\"{}\" offset=\"{}\">",
        file_type_box.length(),
        file_type_box.offset()
    )?;
    writeln!(
        writer,
        "    <xjp:brand length=\"4\" type=\"fourcc\">{}</xjp:brand>",
        file_type_box.brand()
    )?;
    writeln!(
        writer,
        "    <xjp:version length=\"4\" type=\"integer\">{}</xjp:version>",
        file_type_box.min_version()
    )?;

    for compatibility in file_type_box.compatibility_list() {
        writeln!(
            writer,
            "    <xjp:compatibility length=\"4\" type=\"fourcc\">{}</xjp:compatibility>",
            compatibility
        )?;
    }
    writer.write_all(b"  </xjp:ftyp>\n")?;
    Ok(())
}

fn encode_header_super_box<W: io::Write>(
    writer: &mut W,
    header_super_box: &HeaderSuperBox,
) -> Result<(), Box<dyn error::Error>> {
    let image_header_box = &header_super_box.image_header_box;

    writeln!(
        writer,
        "  <xjp:jp2h type=\"box\" length=\"{}\" offset=\"{}\">",
        header_super_box.length(),
        header_super_box.offset()
    )?;
    writeln!(
        writer,
        "    <xjp:ihdr type=\"box\" length=\"{}\" offset=\"{}\">",
        image_header_box.length(),
        image_header_box.offset()
    )?;
    writeln!(
        writer,
        "      <xjp:height type=\"integer\" length=\"4\">{}</xjp:height>",
        image_header_box.height()
    )?;
    writeln!(
        writer,
        "      <xjp:width type=\"integer\" length=\"4\">{}</xjp:width>",
        image_header_box.width()
    )?;
    writeln!(
        writer,
        "      <xjp:num_components type=\"integer\" length=\"1\">{}</xjp:num_components>",
        image_header_box.components_num()
    )?;
    writeln!(
        writer,
        "      <xjp:depth type=\"integer\" length=\"2\">{}</xjp:depth>",
        image_header_box.components_bits()
    )?;
    writeln!(
        writer,
        "      <xjp:compression type=\"integer\" length=\"1\">{}</xjp:compression>",
        image_header_box.compression_type()
    )?;
    writeln!(
        writer,
        "      <xjp:colour_unknown type=\"integer\" length=\"1\">{}</xjp:colour_unknown>",
        image_header_box.colourspace_unknown()
    )?;
    writeln!(
        writer,
        "      <xjp:ipr type=\"integer\" length=\"1\">{}</xjp:ipr>",
        image_header_box.intellectual_property()
    )?;
    writer.write_all(b"    </xjp:ihdr>\n")?;

    if let Some(bits_per_component_box) = &header_super_box.bits_per_component_box {
        encode_bits_per_component_box(writer, bits_per_component_box)?;
    }

    for colour_specification_box in &header_super_box.colour_specification_boxes {
        encode_colour_specification_box(writer, colour_specification_box)?;
    }

    if let Some(palette_box) = &header_super_box.palette_box {
        encode_palette_box(writer, palette_box)?;
    }
    if let Some(component_mapping_box) = &header_super_box.component_mapping_box {
        encode_component_mapping_box(writer, component_mapping_box)?;
    }
    if let Some(channel_definition_box) = &header_super_box.channel_definition_box {
        encode_channel_definition_box(writer, channel_definition_box)?;
    }
    if let Some(resolution_box) = &header_super_box.resolution_box {
        encode_resolution_box(writer, resolution_box)?;
    }

    writer.write_all(b"  </xjp:jp2h>\n")?;
    Ok(())
}

fn encode_bits_per_component_box<W: io::Write>(
    writer: &mut W,
    bits_per_component_box: &BitsPerComponentBox,
) -> Result<(), Box<dyn error::Error>> {
    writeln!(
        writer,
        "  <xjp:bpcc type=\"box\" length=\"{}\" offset=\"{}\">",
        bits_per_component_box.length(),
        bits_per_component_box.offset(),
    )?;

    for component_bit_depth in bits_per_component_box.bits_per_component() {
        writeln!(
            writer,
            "    <xjp:depth length=\"1\" type=\"integer\">{}</xjp:depth>",
            component_bit_depth.value()
        )?;
    }

    writeln!(writer, "    </xjp:ihdr>")?;
    Ok(())
}

fn encode_colour_specification_box<W: io::Write>(
    writer: &mut W,
    colour_specification_box: &ColourSpecificationBox,
) -> Result<(), Box<dyn error::Error>> {
    let method = colour_specification_box.method();
    writeln!(
        writer,
        "    <xjp:colr type=\"box\" length=\"{}\" offset=\"{}\">",
        colour_specification_box.length(),
        colour_specification_box.offset(),
    )?;
    writeln!(
        writer,
        "      <xjp:method length=\"1\" type=\"integer\">{}</xjp:method>",
        method.encoded_meth()[0]
    )?;
    writeln!(
        writer,
        "      <xjp:precedence length=\"1\" type=\"integer\">{}</xjp:precedence>",
        colour_specification_box.precedence()
    )?;
    writeln!(
        writer,
        "      <xjp:approx length=\"1\" type=\"integer\">{}</xjp:approx>",
        colour_specification_box.colourspace_approximation()
    )?;
    // TODO: add support for enumerated and vendor colourspace elements in T.813 A.3.3
    let methdat = method.encoded_methdat();
    writeln!(
        writer,
        "      <xjp:colour length=\"{}\" type=\"hexbyte\">{}</xjp:colour>",
        methdat.len(),
        to_hex(methdat.iter())?
    )?;
    writer.write_all(b"    </xjp:colr>\n")?;
    Ok(())
}

fn encode_palette_box<W: io::Write>(
    writer: &mut W,
    palette_box: &PaletteBox,
) -> Result<(), Box<dyn error::Error>> {
    writeln!(
        writer,
        "    <xjp:pclr type=\"box\" length=\"{}\" offset=\"{}\">",
        palette_box.length(),
        palette_box.offset(),
    )?;
    writeln!(
        writer,
        "      <xjp:num_entries length=\"2\" type=\"integer\">{}</xjp:num_entries>",
        palette_box.num_entries()
    )?;
    writeln!(
        writer,
        "      <xjp:num_components length=\"1\" type=\"integer\">{}</xjp:num_components>",
        palette_box.num_components()
    )?;
    for column_index in 0..palette_box.num_components() {
        writeln!(
            writer,
            "      <xjp:depth length=\"1\" type=\"integer\">{}</xjp:depth>",
            palette_box.bit_depth(column_index).unwrap().encoded()
        )?;
    }
    for entry_index in 0..palette_box.num_entries() {
        for column_index in 0..palette_box.num_components() {
            writeln!(
                writer,
                "      <xjp:data length=\"{}\" type=\"integer\">{}</xjp:data>",
                palette_box.bit_depth(column_index).unwrap().num_bytes(),
                palette_box.entry(entry_index, column_index).unwrap()
            )?;
        }
    }
    writer.write_all(b"    </xjp:pclr>\n")?;
    Ok(())
}

fn encode_component_mapping_box<W: io::Write>(
    writer: &mut W,
    component_mapping_box: &ComponentMappingBox,
) -> Result<(), Box<dyn error::Error>> {
    writeln!(
        writer,
        "    <xjp:cmap type=\"box\" length=\"{}\" offset=\"{}\">",
        component_mapping_box.length(),
        component_mapping_box.offset(),
    )?;

    for component_map in component_mapping_box.component_map() {
        // TODO: Verify schema
        writeln!(writer, "      <xjp:mapc type=\"xjp:mapc\">")?;
        writeln!(
            writer,
            "        <xjp:component length=\"2\" type=\"integer\">{}</xjp:component>",
            component_map.component()
        )?;
        writeln!(
            writer,
            "        <xjp:mtype length=\"1\" type=\"integer\">{}</xjp:mtype>",
            component_map.mapping_type()
        )?;
        writeln!(
            writer,
            "        <xjp:palette length=\"1\" type=\"integer\">{}</xjp:palette>",
            component_map.palette()
        )?;
        writeln!(writer, "      </xjp:mapc>")?;
    }

    writer.write_all(b"    </xjp:cmap>\n")?;
    Ok(())
}
fn encode_channel_definition_box<W: io::Write>(
    writer: &mut W,
    channel_definition_box: &ChannelDefinitionBox,
) -> Result<(), Box<dyn error::Error>> {
    writeln!(
        writer,
        "    <xjp:cdef type=\"box\" length=\"{}\" offset=\"{}\">",
        channel_definition_box.length(),
        channel_definition_box.offset()
    )?;
    writeln!(
        writer,
        "      <xjp:num_entries length=\"2\" type=\"integer\">{}</xjp:num_entries>",
        channel_definition_box.channels().len()
    )?;
    for channel in channel_definition_box.channels() {
        writeln!(
            writer,
            "      <xjp:index length=\"2\" type=\"integer\">{}</xjp:index>",
            channel.channel_index()
        )?;
        writeln!(
            writer,
            "      <xjp:type length=\"2\" type=\"integer\">{}</xjp:type>",
            channel.channel_type_u16()
        )?;
        writeln!(
            writer,
            "      <xjp:assoc length=\"2\" type=\"integer\">{}</xjp:assoc>",
            channel.channel_association()
        )?;
    }
    writer.write_all(b"    </xjp:cdef>\n")?;
    Ok(())
}

fn encode_resolution_box<W: io::Write>(
    writer: &mut W,
    resolution_box: &ResolutionSuperBox,
) -> Result<(), Box<dyn error::Error>> {
    writeln!(
        writer,
        "  <xjp:res_ type=\"box\" length=\"{}\" offset=\"{}\">",
        resolution_box.length(),
        resolution_box.offset()
    )?;

    if let Some(capture_resolution_box) = resolution_box.capture_resolution_box() {
        encode_capture_resolution_box(writer, capture_resolution_box)?;
    }
    if let Some(default_display_resolution_box) = resolution_box.default_display_resolution_box() {
        encode_default_display_resolution_box(writer, default_display_resolution_box)?;
    }

    writer.write_all(b"  </xjp:res_>\n")?;
    Ok(())
}

fn encode_capture_resolution_box<W: io::Write>(
    writer: &mut W,
    capture_resolution_box: &CaptureResolutionBox,
) -> Result<(), Box<dyn error::Error>> {
    writeln!(
        writer,
        "    <xjp:resc type=\"box\" length=\"{}\" offset=\"{}\">",
        capture_resolution_box.length(),
        capture_resolution_box.offset()
    )?;
    writeln!(
        writer,
        "      <xjp:vert_num length=\"2\" type=\"integer\">{}</xjp:vert_num>",
        capture_resolution_box.vertical_capture_grid_resolution_numerator()
    )?;
    writeln!(
        writer,
        "      <xjp:vert_den length=\"2\" type=\"integer\">{}</xjp:vert_den>",
        capture_resolution_box.vertical_capture_grid_resolution_denominator()
    )?;
    writeln!(
        writer,
        "      <xjp:hori_num length=\"2\" type=\"integer\">{}</xjp:hori_num>",
        capture_resolution_box.horizontal_capture_grid_resolution_numerator()
    )?;
    writeln!(
        writer,
        "      <xjp:hori_den length=\"2\" type=\"integer\">{}</xjp:hori_den>",
        capture_resolution_box.horizontal_capture_grid_resolution_denominator()
    )?;
    writeln!(
        writer,
        "      <xjp:vert_exp length=\"1\" type=\"integer\">{}</xjp:vert_exp>",
        capture_resolution_box.vertical_capture_grid_resolution_exponent()
    )?;
    writeln!(
        writer,
        "      <xjp:hori_exp length=\"1\" type=\"integer\">{}</xjp:hori_exp>",
        capture_resolution_box.horizontal_capture_grid_resolution_exponent()
    )?;
    writer.write_all(b"    </xjp:resc>\n")?;
    Ok(())
}

fn encode_default_display_resolution_box<W: io::Write>(
    writer: &mut W,
    default_display_resolution_box: &DefaultDisplayResolutionBox,
) -> Result<(), Box<dyn error::Error>> {
    writeln!(
        writer,
        "    <xjp:resd type=\"box\" length=\"{}\" offset=\"{}\">",
        default_display_resolution_box.length(),
        default_display_resolution_box.offset()
    )?;
    writeln!(
        writer,
        "      <xjp:vert_num length=\"2\" type=\"integer\">{}</xjp:vert_num>",
        default_display_resolution_box.vertical_display_grid_resolution_numerator()
    )?;
    writeln!(
        writer,
        "      <xjp:vert_den length=\"2\" type=\"integer\">{}</xjp:vert_den>",
        default_display_resolution_box.vertical_display_grid_resolution_denominator()
    )?;
    writeln!(
        writer,
        "      <xjp:hori_num length=\"2\" type=\"integer\">{}</xjp:hori_num>",
        default_display_resolution_box.horizontal_display_grid_resolution_numerator()
    )?;
    writeln!(
        writer,
        "      <xjp:hori_den length=\"2\" type=\"integer\">{}</xjp:hori_den>",
        default_display_resolution_box.horizontal_display_grid_resolution_denominator()
    )?;
    writeln!(
        writer,
        "      <xjp:vert_exp length=\"1\" type=\"integer\">{}</xjp:vert_exp>",
        default_display_resolution_box.vertical_display_grid_resolution_exponent()
    )?;
    writeln!(
        writer,
        "      <xjp:hori_exp length=\"1\" type=\"integer\">{}</xjp:hori_exp>",
        default_display_resolution_box.horizontal_display_grid_resolution_exponent()
    )?;

    writer.write_all(b"    </xjp:resd>\n")?;
    Ok(())
}

fn encode_siz<W: io::Write>(
    writer: &mut W,
    segment: &ImageAndTileSizeMarkerSegment,
) -> Result<(), Box<dyn error::Error>> {
    writeln!(
        writer,
        "    <xjp:SIZ type=\"marker\" length=\"{}\" offset=\"{}\">",
        segment.length(),
        segment.offset()
    )?;
    writeln!(
        writer,
        "      <xjp:Rsiz>{}</xjp:Rsiz>",
        segment.decoder_capabilities()
    )?;
    writeln!(
        writer,
        "      <xjp:Xsiz>{}</xjp:Xsiz>",
        segment.reference_grid_width()
    )?;
    writeln!(
        writer,
        "      <xjp:Ysiz>{}</xjp:Ysiz>",
        segment.reference_grid_height()
    )?;
    writeln!(
        writer,
        "      <xjp:OXsiz>{}</xjp:OXsiz>",
        segment.image_horizontal_offset()
    )?;
    writeln!(
        writer,
        "      <xjp:OYsiz>{}</xjp:OYsiz>",
        segment.image_vertical_offset()
    )?;
    writeln!(
        writer,
        "      <xjp:XTsiz>{}</xjp:XTsiz>",
        segment.reference_tile_width()
    )?;
    writeln!(
        writer,
        "      <xjp:YTsiz>{}</xjp:YTsiz>",
        segment.reference_tile_height()
    )?;
    writeln!(
        writer,
        "      <xjp:XTOsiz>{}</xjp:XTOsiz>",
        segment.tile_horizontal_offset()
    )?;
    writeln!(
        writer,
        "      <xjp:YTOsiz>{}</xjp:YTOsiz>",
        segment.tile_vertical_offset()
    )?;
    writeln!(
        writer,
        "      <xjp:Csiz>{}</xjp:Csiz>",
        segment.no_components()
    )?;

    let no_components = segment.no_components() as usize;

    let mut i = 0;
    loop {
        writeln!(
            writer,
            "      <xjp:Ssiz>{}</xjp:Ssiz>",
            segment.precision(i)?
        )?;
        writeln!(
            writer,
            "      <xjp:XRsiz>{}</xjp:XRsiz>",
            segment.horizontal_separation(i)?
        )?;
        writeln!(
            writer,
            "      <xjp:YRsiz>{}</xjp:YRsiz>",
            segment.vertical_separation(i)?
        )?;

        i += 1;
        if i == no_components {
            break;
        }
    }
    writeln!(writer, "    </xjp:SIZ>",)?;

    Ok(())
}

fn encode_coding_style_parameters<W: io::Write>(
    writer: &mut W,
    coding_style_parameters: &CodingStyleParameters,
) -> Result<(), Box<dyn error::Error>> {
    writeln!(
        writer,
        "        <xjp:num_levels>{}</xjp:num_levels>",
        coding_style_parameters.no_decomposition_levels()
    )?;
    writeln!(
        writer,
        "        <xjp:xcb>{}</xjp:xcb>",
        coding_style_parameters.code_block_width()
    )?;
    writeln!(
        writer,
        "        <xjp:ycb>{}</xjp:ycb>",
        coding_style_parameters.code_block_height()
    )?;
    writeln!(
        writer,
        "        <xjp:style>{}</xjp:style>",
        coding_style_parameters.code_block_style()
    )?;
    writeln!(
        writer,
        "        <xjp:wavelet>{:?}</xjp:wavelet>",
        coding_style_parameters.transformation()
    )?;

    if let Some(precinct_sizes) = coding_style_parameters.precinct_sizes() {
        for precinct_size in precinct_sizes {
            writeln!(
                writer,
                "        <xjp:ppx>{}</xjp:ppx>",
                precinct_size.width_exponent()
            )?;
            writeln!(
                writer,
                "        <xjp:ppy>{}</xjp:ppy>",
                precinct_size.height_exponent()
            )?;
        }
    }

    Ok(())
}

fn encode_cod<W: io::Write>(
    writer: &mut W,
    segment: &CodingStyleMarkerSegment,
) -> Result<(), Box<dyn error::Error>> {
    writeln!(
        writer,
        "    <xjp:COD type=\"marker\" length=\"{}\" offset=\"{}\">",
        segment.length(),
        segment.offset()
    )?;
    writeln!(
        writer,
        "      <xjp:Scod>{}</xjp:Scod>",
        segment.coding_style()
    )?;

    writeln!(writer, "      <xjp:SGcod>",)?;
    writeln!(
        writer,
        "        <xjp:progression>{:?}</xjp:progression>",
        segment.progression_order()
    )?;
    writeln!(
        writer,
        "        <xjp:num_layers>{}</xjp:num_layers>",
        segment.no_layers()
    )?;
    writeln!(
        writer,
        "        <xjp:colour_conv>{:?}</xjp:colour_conv>",
        segment.multiple_component_transformation()
    )?;
    writeln!(writer, "      </xjp:SGcod>",)?;

    writeln!(writer, "      <xjp:SPcod>",)?;
    encode_coding_style_parameters(writer, segment.coding_style_parameters())?;
    writeln!(writer, "      </xjp:SPcod>",)?;

    // Scod length = 1, SGcod length = 4, SPcod (loop) length = 5 - 43, hexbyte
    writeln!(writer, "    </xjp:COD>",)?;

    Ok(())
}

fn encode_qcd<W: io::Write>(
    writer: &mut W,
    segment: &QuantizationDefaultMarkerSegment,
) -> Result<(), Box<dyn error::Error>> {
    writeln!(writer, "    <xjp:QCD>",)?;
    writeln!(
        writer,
        "      <xjp:Sqcd>{}</xjp:Sqcd>",
        segment.quantization_style_u8()
    )?;

    for value in segment.quantization_values().iter() {
        writeln!(writer, "      <xjp:SPqcd>{}</xjp:SPqcd>", value)?;
    }
    writeln!(writer, "    </xjp:QCD>",)?;

    Ok(())
}

fn encode_coc<W: io::Write>(
    writer: &mut W,
    _segment: &CodingStyleMarkerSegment,
) -> Result<(), Box<dyn error::Error>> {
    writeln!(writer, "    <xjp:COC>",)?;
    writeln!(writer, "    </xjp:COC>",)?;
    todo!();
}

fn encode_contiguous_codestream_header<W: io::Write>(
    writer: &mut W,
    header: &Header,
) -> Result<(), Box<dyn error::Error>> {
    encode_siz(writer, header.image_and_tile_size_marker_segment())?;
    encode_cod(writer, header.coding_style_marker_segment())?;
    encode_qcd(writer, header.quantization_default_marker_segment())?;
    // QCC
    // RGN
    // POC
    // PPM
    // TLM
    // PLM
    // CRG
    // COM

    Ok(())
}

fn encode_contiguous_codestream<W: io::Write>(
    writer: &mut W,
    representation: &Representation,
    contiguous_codestream: &ContiguousCodestream,
    contiguous_codestream_box: Option<&ContiguousCodestreamBox>,
) -> Result<(), Box<dyn error::Error>> {
    match contiguous_codestream_box {
        Some(cc_box) => {
            writeln!(
                writer,
                "  <xjp:jp2c type=\"box\" length=\"{}\" offset=\"{}\">",
                cc_box.length(),
                cc_box.offset()
            )?;
        }
        None => {
            writeln!(writer, "  <xjp:jp2c type=\"box\">",)?;
        }
    }

    encode_contiguous_codestream_header(writer, contiguous_codestream.header())?;

    if *representation != Representation::Skeleton {
        todo!();
    }

    writer.write_all(b"  </xjp:jp2c>\n")?;
    Ok(())
}

fn encode_xml_box<W: io::Write>(
    writer: &mut W,
    xml_box: &XMLBox,
) -> Result<(), Box<dyn error::Error>> {
    writeln!(
        writer,
        "  <xjp:_xml_ type=\"box\" length=\"{}\" offset=\"{}\">",
        xml_box.length(),
        xml_box.offset()
    )?;

    let value = xml_box.format();
    writeln!(
        writer,
        "    <xjp:text length=\"{}\" type=\"string\">",
        value.len(),
    )?;
    writer.write_all(b"    <![CDATA[")?;
    write!(writer, "{}", value)?;
    writer.write_all(b"]]>\n")?;
    writer.write_all(b"    </xjp:text>\n")?;
    writer.write_all(b"  </xjp:_xml_>\n")?;
    Ok(())
}

fn encode_uuid_box<W: io::Write>(
    writer: &mut W,
    uuid_box: &UUIDBox,
) -> Result<(), Box<dyn error::Error>> {
    writeln!(
        writer,
        "  <xjp:uuid type=\"box\" length=\"{}\" offset=\"{}\">",
        uuid_box.length(),
        uuid_box.offset()
    )?;
    writeln!(
        writer,
        "    <xjp:id length=\"16\" type=\"integer\">{}</xjp:id>",
        u128::from_be_bytes(*uuid_box.uuid())
    )?;
    writeln!(
        writer,
        "    <xjp:data length=\"{}\" type=\"hexbyte\">{}</xjp:data>",
        uuid_box.data().len(),
        to_hex(uuid_box.data().iter())?
    )?;
    writer.write_all(b"  </xjp:uuid>\n")?;
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
pub fn encode_jp2<W: io::Write>(
    writer: &mut W,
    file: &File,
    representation: Representation,
    name: &str,
) -> Result<(), Box<dyn error::Error>> {
    let mut reader = BufReader::new(file);

    let jp2 = decode_jp2(&mut reader)?;

    writer.write_all(b"<?xml version=\"1.0\"?>\n")?;
    writer.write_all(b"<xjp:jpxml xmlns:xjp=\"http://www.jpeg.org/jpxml/1.0\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\"")?;
    // Length is required?
    if !name.is_empty() {
        write!(writer, " length=\"{}\"", jp2.length())?;
        write!(writer, " name=\"{}\"", name)?;
    }
    writer.write_all(b">\n")?;

    if let Some(signature_box) = jp2.signature_box() {
        encode_signature_box(writer, signature_box)?;
    }

    if let Some(file_type_box) = jp2.file_type_box() {
        encode_file_type_box(writer, file_type_box)?;
    }

    // TODO: Check if header box is optional
    if let Some(header_box) = jp2.header_box() {
        encode_header_super_box(writer, header_box)?;
    }

    for xml_box in jp2.xml_boxes() {
        encode_xml_box(writer, xml_box)?;
    }
    for uuid_box in jp2.uuid_boxes() {
        encode_uuid_box(writer, uuid_box)?;
    }

    for contiguous_codestream_box in jp2.contiguous_codestreams_boxes() {
        reader.seek(io::SeekFrom::Start(contiguous_codestream_box.offset))?;
        let contiguous_codestream = decode_jpc(&mut reader)?;

        encode_contiguous_codestream(
            writer,
            &representation,
            &contiguous_codestream,
            Some(contiguous_codestream_box),
        )?;
    }
    writer.write_all(b"</xjp:jpxml>\n")?;

    Ok(())
}

pub fn encode_jpc<W: io::Write>(
    writer: &mut W,
    file: &File,
    representation: Representation,
) -> Result<(), Box<dyn error::Error>> {
    let mut reader = BufReader::new(file);

    writer.write_all(b"<?xml version=\"1.0\"?>\n")?;
    writer.write_all(b"<xjp:jpxml xmlns:xjp=\"http://www.jpeg.org/jpxml/1.0\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\"")?;
    let contiguous_codestream = decode_jpc(&mut reader)?;
    encode_contiguous_codestream(writer, &representation, &contiguous_codestream, None)?;
    writer.write_all(b"</xjp:jpxml>\n")?;
    Ok(())
}
