use std::{fs::File, io::BufReader, path::Path};

use jpc::{CommentRegistrationValue, decode_jpc};

#[test]
fn test_blue() {
    let filename = "blue.j2k";
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join(filename);
    let file = File::open(path).expect("file should exist");
    let mut reader = BufReader::new(file);
    let result = decode_jpc(&mut reader);
    assert!(result.is_ok());
    let codestream = result.unwrap();
    assert_eq!(codestream.length(), 0);
    assert_eq!(codestream.offset(), 0);

    let header = codestream.header();
    println!("\nheader: {header:?}");

    let siz = header.image_and_tile_size_marker_segment();
    assert_eq!(siz.reference_grid_width(), 128);
    assert_eq!(siz.reference_grid_height(), 64);
    assert_eq!(siz.image_horizontal_offset(), 0);
    assert_eq!(siz.image_vertical_offset(), 0);
    assert_eq!(siz.offset(), 4);
    assert_eq!(siz.length(), 47);
    assert_eq!(siz.decoder_capabilities(), 0);
    assert_eq!(siz.image_horizontal_offset(), 0);
    assert_eq!(siz.image_vertical_offset(), 0);
    assert_eq!(siz.reference_tile_width(), 128);
    assert_eq!(siz.reference_tile_height(), 64);
    assert_eq!(siz.no_components(), 3);
    assert_eq!(siz.precision(0).unwrap(), 8);
    assert_eq!(siz.values_are_signed(0).unwrap(), false);
    assert_eq!(siz.precision(1).unwrap(), 8);
    assert_eq!(siz.values_are_signed(1).unwrap(), false);
    assert_eq!(siz.precision(2).unwrap(), 8);
    assert_eq!(siz.values_are_signed(2).unwrap(), false);
    assert_eq!(siz.horizontal_separation(0).unwrap(), 1);
    assert_eq!(siz.horizontal_separation(1).unwrap(), 1);
    assert_eq!(siz.horizontal_separation(2).unwrap(), 1);
    assert_eq!(siz.vertical_separation(0).unwrap(), 1);
    assert_eq!(siz.vertical_separation(1).unwrap(), 1);
    assert_eq!(siz.vertical_separation(2).unwrap(), 1);

    assert_eq!(header.comment_marker_segments().len(), 1);
    let com = header.comment_marker_segments().first().unwrap();
    assert_eq!(com.registration_value(), CommentRegistrationValue::Latin);
    assert!(com.comment_utf8().is_ok());
    assert_eq!(com.comment_utf8().unwrap(), "Created by OpenJPEG version 2.5.0");

}
