use std::{fs::File, io::BufReader,path::Path};

use jp2::{JBox as _, Methods, decode_jp2};

#[test]
fn test_hazard() {
    let filename = "hazard.jp2";
    test_jp2_file(filename, 17298);
}

fn test_jp2_file(filename: &str, expected_len: u64) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join(filename);
    let file = File::open(path).expect("file should exist");
    let mut reader = BufReader::new(file);
    let result = decode_jp2(&mut reader);
    assert!(result.is_ok());
    let boxes = result.unwrap();
    assert_eq!(boxes.length(), expected_len);

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
    assert_eq!(image_header_box.height(), 128);
    assert_eq!(image_header_box.width(), 64);
    assert_eq!(image_header_box.components_num(), 3);
    assert_eq!(image_header_box.compression_type(), 7);
    assert_eq!(image_header_box.colourspace_unknown(), 0);
    assert_eq!(image_header_box.intellectual_property(), 0);
    assert_eq!(image_header_box.components_bits(), 16);
    // TODO: add this API
    // assert_eq!(image_header_box.values_are_signed(), false);

    assert!(header_box.bits_per_component_box.is_none());

    assert_eq!(header_box.colour_specification_boxes.len(), 1);
    let colour_specification_box = header_box.colour_specification_boxes.first().unwrap();
    assert_eq!(
        colour_specification_box.method(),
        Methods::EnumeratedColourSpace
    );
    assert_eq!(colour_specification_box.precedence(), 0);
    assert_eq!(colour_specification_box.colourspace_approximation(), 0u8);
    assert!(colour_specification_box.enumerated_colour_space().is_some());
    assert_eq!(
        colour_specification_box.enumerated_colour_space().unwrap(),
        16u32
    );

    assert!(header_box.palette_box.is_none());

    assert!(header_box.component_mapping_box.is_none());

    assert!(header_box.channel_definition_box.is_none());

    assert!(header_box.resolution_box.is_none());

    assert_eq!(boxes.contiguous_codestreams_boxes().len(), 1);
    let codestream_box = boxes.contiguous_codestreams_boxes().first().unwrap();
    assert_eq!(codestream_box.length(), expected_len - 85);
    assert_eq!(codestream_box.offset(), 85);

    assert_eq!(boxes.xml_boxes().len(), 0);

    assert_eq!(boxes.uuid_boxes().len(), 0);
}
