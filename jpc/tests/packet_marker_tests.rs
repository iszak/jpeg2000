use std::{fs::File, io::BufReader, path::Path};

use jpc::{
    decode_jpc, CodingBlockStyle, CommentRegistrationValue, MultipleComponentTransformation,
    ProgressionOrder, TransformationFilter,
};

#[test]
fn test_eph() {
    let filename = "eph.j2k";
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
    assert_eq!(siz.reference_grid_width(), 2);
    assert_eq!(siz.reference_grid_height(), 1);
    assert_eq!(siz.image_horizontal_offset(), 0);
    assert_eq!(siz.image_vertical_offset(), 0);
    assert_eq!(siz.offset(), 4);
    assert_eq!(siz.length(), 47);
    assert_eq!(siz.decoder_capabilities(), 0);
    assert_eq!(siz.image_horizontal_offset(), 0);
    assert_eq!(siz.image_vertical_offset(), 0);
    assert_eq!(siz.reference_tile_width(), 2);
    assert_eq!(siz.reference_tile_height(), 1);
    assert_eq!(siz.no_components(), 3);
    assert_eq!(siz.precision(0).unwrap(), 16);
    assert_eq!(siz.values_are_signed(0).unwrap(), false);
    assert_eq!(siz.precision(1).unwrap(), 16);
    assert_eq!(siz.values_are_signed(1).unwrap(), false);
    assert_eq!(siz.precision(2).unwrap(), 16);
    assert_eq!(siz.values_are_signed(2).unwrap(), false);
    assert_eq!(siz.horizontal_separation(0).unwrap(), 1);
    assert_eq!(siz.horizontal_separation(1).unwrap(), 1);
    assert_eq!(siz.horizontal_separation(2).unwrap(), 1);
    assert_eq!(siz.vertical_separation(0).unwrap(), 1);
    assert_eq!(siz.vertical_separation(1).unwrap(), 1);
    assert_eq!(siz.vertical_separation(2).unwrap(), 1);

    // TODO: CAP

    // TODO: PRF

    // COD
    let cod = header.coding_style_marker_segment();
    // Scod
    assert_eq!(cod.coding_style(), 4);
    // SGcod
    assert_eq!(cod.progression_order(), ProgressionOrder::RLLCPP);
    assert_eq!(cod.no_layers(), 1);
    assert_eq!(
        cod.multiple_component_transformation(),
        MultipleComponentTransformation::Multiple
    );
    // SPcod
    assert_eq!(cod.coding_style_parameters().no_decomposition_levels(), 0);
    assert_eq!(cod.coding_style_parameters().code_block_width(), 64);
    assert_eq!(cod.coding_style_parameters().code_block_height(), 64);
    assert_eq!(cod.coding_style_parameters().code_block_style(), 0);
    assert_eq!(
        cod.coding_style_parameters().coding_block_styles(),
        vec![
            CodingBlockStyle::NoSelectiveArithmeticCodingBypass,
            CodingBlockStyle::NoResetOfContextProbabilities,
            CodingBlockStyle::NoTerminationOnEachCodingPass,
            CodingBlockStyle::NoVerticallyCausalContext,
            CodingBlockStyle::NoPredictableTermination,
            CodingBlockStyle::NoSegmentationSymbolsAreUsed
        ]
    );

    assert_eq!(
        cod.coding_style_parameters().transformation(),
        TransformationFilter::Reversible
    );

    assert_eq!(
        cod.coding_style_parameters().has_defined_precinct_size(),
        false
    );
    assert_eq!(
        cod.coding_style_parameters().has_default_precinct_size(),
        true
    );
    assert!(cod.coding_style_parameters().precinct_sizes().is_some());

    // COC
    assert!(header.coding_style_component_segment().is_empty());

    // TODO: QCD

    // QCC
    assert!(header.quantization_component_segments().is_empty());

    // RGN
    assert!(header.region_of_interest_segments().is_empty());

    // POC
    assert!(header.progression_order_change_segment().is_none());

    // PPM
    assert!(header.packed_packet_headers_segments().is_empty());

    // TLM
    assert!(header.tile_part_lengths_segment().is_none());

    // PLM
    assert!(header.packet_lengths_segments().is_empty());

    // CRG
    assert!(header.component_registration_segment().is_none());

    // COM
    assert_eq!(header.comment_marker_segments().len(), 1);
    let com = header.comment_marker_segments().first().unwrap();
    assert_eq!(com.registration_value(), CommentRegistrationValue::Latin);
    assert!(com.comment_utf8().is_ok());
    assert_eq!(
        com.comment_utf8().unwrap(),
        "Created by OpenJPEG version 2.5.0"
    );
}
