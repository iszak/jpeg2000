use std::{
    fs::File,
    io::{BufReader, Seek as _, SeekFrom},
    path::Path,
};

use jp2::{decode_jp2, JBox as _, Methods};
use jpc::{
    CodingBlockStyle, MultipleComponentTransformation, ProgressionOrder, TransformationFilter,
};

struct ExpectedConfiguration {
    file_length: u64,
    coding_style: u8,
    has_tlm: bool,
    progression_order: ProgressionOrder,
    comments: Vec<String>,
}

#[test]
fn test_hazard() {
    let filename = "hazard.jp2";
    let expected_configuration = ExpectedConfiguration {
        file_length: 17298,
        coding_style: 0,
        has_tlm: false,
        progression_order: ProgressionOrder::LRLCPP,
        comments: vec!["Created by OpenJPEG version 2.5.0".to_string()],
    };
    test_jp2_file(filename, expected_configuration);
}

#[test]
fn test_pcrl() {
    let filename = "pcrl.jp2";
    let expected_configuration = ExpectedConfiguration {
        file_length: 17479,
        coding_style: 6,
        has_tlm: true,
        progression_order: ProgressionOrder::PCRLLP,
        comments: vec!["test data for rust JPEG 2000".to_string()],
    };
    test_jp2_file(filename, expected_configuration);
}

fn test_jp2_file(filename: &str, expected: ExpectedConfiguration) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join(filename);
    let file = File::open(path).expect("file should exist");
    let mut reader = BufReader::new(file);
    let result = decode_jp2(&mut reader);
    assert!(result.is_ok());
    let boxes = result.unwrap();
    assert_eq!(boxes.length(), expected.file_length);

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
    assert_eq!(image_header_box.values_are_signed(), false);

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
    assert_eq!(codestream_box.length(), expected.file_length - 85);
    assert_eq!(codestream_box.offset(), 85);

    assert!(reader.seek(SeekFrom::Start(codestream_box.offset)).is_ok());
    let codestream_decode_result = jpc::decode_jpc(&mut reader);
    assert!(codestream_decode_result.is_ok());
    let codestream = codestream_decode_result.unwrap();

    let header = codestream.header();

    // SIZ
    let siz = header.image_and_tile_size_marker_segment();
    assert_eq!(siz.reference_grid_width(), 64);
    assert_eq!(siz.reference_grid_height(), 128);
    assert_eq!(siz.image_horizontal_offset(), 0);
    assert_eq!(siz.image_vertical_offset(), 0);
    assert_eq!(siz.offset(), 89);
    assert_eq!(siz.length(), 47);
    assert_eq!(siz.decoder_capabilities(), 0);
    assert_eq!(siz.image_horizontal_offset(), 0);
    assert_eq!(siz.image_vertical_offset(), 0);
    assert_eq!(siz.reference_tile_width(), 64);
    assert_eq!(siz.reference_tile_height(), 128);
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
    assert_eq!(cod.coding_style(), expected.coding_style);
    // SGcod
    assert_eq!(cod.progression_order(), expected.progression_order);
    assert_eq!(cod.no_layers(), 1);
    assert_eq!(
        cod.multiple_component_transformation(),
        MultipleComponentTransformation::Multiple
    );
    // SPcod
    assert_eq!(cod.coding_style_parameters().no_decomposition_levels(), 5);
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
    assert_eq!(cod.coding_style_parameters().has_precinct_size(), false);
    assert!(cod.coding_style_parameters().precinct_sizes().is_none());

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
    if expected.has_tlm {
        assert!(header.tile_part_lengths_segment().is_some());
    } else {
        assert!(header.tile_part_lengths_segment().is_none());
    }

    // PLM
    assert!(header.packet_lengths_segments().is_empty());

    // CRG
    assert!(header.component_registration_segment().is_none());

    // COM
    assert_eq!(
        header.comment_marker_segments().len(),
        expected.comments.len()
    );
    for i in 0..header.comment_marker_segments().len() {
        let com = header.comment_marker_segments().get(i).unwrap();
        assert_eq!(
            com.registration_value(),
            jpc::CommentRegistrationValue::Latin
        );
        assert!(com.comment_utf8().is_ok());
        assert_eq!(
            com.comment_utf8().unwrap(),
            expected.comments.get(i).unwrap()
        );
    }

    assert_eq!(boxes.xml_boxes().len(), 0);

    assert_eq!(boxes.uuid_boxes().len(), 0);
}
