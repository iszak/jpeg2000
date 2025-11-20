use std::{fs::File, io::BufReader, path::Path};

use jp2::{decode_jp2, ColourSpecificationMethods, JBox as _, JP2File};

struct ExpectedConfiguration {
    width: u32,
    height: u32,
    num_components: u16,
    bit_depth: u8,
    grayscale: bool,
}
#[test]
fn test_hazard() {
    let boxes = test_jp2_file(
        "hazard.jp2",
        ExpectedConfiguration {
            width: 64,
            height: 128,
            num_components: 3,
            bit_depth: 16,
            grayscale: false,
        },
    );

    assert_eq!(boxes.xml_boxes().len(), 0);

    assert_eq!(boxes.uuid_boxes().len(), 0);
}

#[test]
fn test_geojp2() {
    // GeoJP2, as implemented by GDAL
    // Tests UUID and XML boxes
    let boxes = test_jp2_file(
        "geojp2.jp2",
        ExpectedConfiguration {
            width: 100,
            height: 24,
            num_components: 1,
            bit_depth: 8,
            grayscale: true,
        },
    );

    assert_eq!(boxes.xml_boxes().len(), 1);
    let xml = boxes.xml_boxes().first().unwrap();
    assert_eq!(xml.length(), 127);
    assert_eq!(xml.offset(), 465);
    assert_eq!(xml.format(), "<GDALMultiDomainMetadata>\n  <Metadata>\n    <MDI key=\"Comment\">Created with GIMP</MDI>\n  </Metadata>\n</GDALMultiDomainMetadata>\n");

    assert_eq!(boxes.uuid_boxes().len(), 1);
    let uuid = boxes.uuid_boxes().first().unwrap();
    assert_eq!(uuid.length(), 372);
    assert_eq!(uuid.offset(), 85);
    // The UUID is for GeoJP2
    assert_eq!(
        *uuid.uuid(),
        [
            0xb1, 0x4b, 0xf8, 0xbd, 0x08, 0x3d, 0x4b, 0x43, 0xa5, 0xae, 0x8c, 0xd7, 0xd5, 0xa6,
            0xce, 0x03
        ]
    );
    // The body is a degenerate GeoTIFF file, starts with TIFF signature
    assert_eq!(uuid.data()[0], b'I');
    assert_eq!(uuid.data()[1], b'I');
    assert_eq!(uuid.data().len(), 356);
}

fn test_jp2_file(filename: &str, expected: ExpectedConfiguration) -> JP2File {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join(filename);
    let file = File::open(path).expect("file should exist");
    let mut reader = BufReader::new(file);
    let result = decode_jp2(&mut reader);
    assert!(result.is_ok());
    let boxes = result.unwrap();
    assert!(boxes.length() > 0);

    assert!(boxes.signature_box().is_some());
    let signature = boxes.signature_box().as_ref().unwrap();
    assert_eq!(signature.signature(), *b"\x0d\x0a\x87\x0a");

    assert!(boxes.file_type_box().is_some());
    let file_type = boxes.file_type_box().as_ref().unwrap();
    assert_eq!(file_type.brand(), "jp2 ");
    assert_eq!(file_type.min_version(), 0);
    assert_eq!(file_type.compatibility_list(), vec!["jp2 "]);

    assert!(boxes.header_box().is_some());
    let header_box = boxes.header_box().as_ref().unwrap();
    let image_header_box = &header_box.image_header_box;
    assert_eq!(image_header_box.height(), expected.height);
    assert_eq!(image_header_box.width(), expected.width);
    assert_eq!(image_header_box.components_num(), expected.num_components);
    assert_eq!(image_header_box.compression_type(), 7);
    assert_eq!(image_header_box.colourspace_unknown(), 0);
    assert_eq!(image_header_box.intellectual_property(), 0);
    assert_eq!(image_header_box.components_bits(), expected.bit_depth);
    assert_eq!(image_header_box.values_are_signed(), false);

    assert!(header_box.bits_per_component_box.is_none());

    assert_eq!(header_box.colour_specification_boxes.len(), 1);
    let colour_specification_box = header_box.colour_specification_boxes.first().unwrap();
    assert_eq!(
        colour_specification_box.method(),
        ColourSpecificationMethods::EnumeratedColourSpace
    );
    assert_eq!(colour_specification_box.precedence(), 0);
    assert_eq!(colour_specification_box.colourspace_approximation(), 0u8);
    assert!(colour_specification_box.enumerated_colour_space().is_some());
    assert_eq!(
        colour_specification_box.enumerated_colour_space().unwrap(),
        match expected.grayscale {
            true => 17u32,
            false => 16u32,
        }
    );

    assert!(header_box.palette_box.is_none());

    assert!(header_box.component_mapping_box.is_none());

    assert!(header_box.channel_definition_box.is_none());

    assert!(header_box.resolution_box.is_none());

    assert_eq!(boxes.contiguous_codestreams_boxes().len(), 1);
    let codestream_box = boxes.contiguous_codestreams_boxes().first().unwrap();
    assert!(codestream_box.length() > 0);
    assert!(codestream_box.offset() > 0);

    boxes
}
