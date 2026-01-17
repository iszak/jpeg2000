use std::{fs::File, io::BufReader, path::Path};

use jp2::{
    colour_specification::{ColourSpecificationMethods, EnumeratedColourSpaces},
    decode_jp2,
    extension::{ExtensionFeatureRequirement, FeatureFlag},
    BitDepth, ChannelTypes, JBox as _, JP2File,
};

struct ExpectedConfiguration {
    compatibility_list: Vec<String>,
    width: u32,
    height: u32,
    num_components: u16,
    bit_depth: u8,
    colour_specification_method: ColourSpecificationMethods,
    has_unexpected_approx_set: bool,
}

#[test]
fn test_hazard() {
    let boxes = test_jp2_file(
        "hazard.jp2",
        ExpectedConfiguration {
            compatibility_list: vec!["jp2 ".into()],
            width: 64,
            height: 128,
            num_components: 3,
            bit_depth: 16,
            colour_specification_method: ColourSpecificationMethods::EnumeratedColourSpace {
                code: EnumeratedColourSpaces::sRGB,
            },
            has_unexpected_approx_set: false,
        },
    );

    assert_eq!(boxes.xml_boxes().len(), 0);

    assert_eq!(boxes.uuid_boxes().len(), 0);
}

#[test]
fn test_sample_file1() {
    let boxes = test_sample_jp2_file(
        "file1.jp2",
        ExpectedConfiguration {
            compatibility_list: vec!["\0\0\0\u{1}".into(), "jp2 ".into()],
            width: 768,
            height: 512,
            num_components: 3,
            bit_depth: 8,
            colour_specification_method: ColourSpecificationMethods::EnumeratedColourSpace {
                code: EnumeratedColourSpaces::sRGB,
            },
            has_unexpected_approx_set: true,
        },
    );

    let header_box = boxes.header_box().as_ref().unwrap();
    assert!(header_box.channel_definition_box.is_none());
    assert!(header_box.palette_box.is_none());
    assert!(header_box.component_mapping_box.is_none());

    assert_eq!(boxes.xml_boxes().len(), 2);

    assert_eq!(boxes.uuid_boxes().len(), 0);
}

#[test]
fn test_sample_file2() {
    let boxes = test_sample_jp2_file(
        "file2.jp2",
        ExpectedConfiguration {
            compatibility_list: vec!["\0\0\0\u{1}".into(), "jp2 ".into()],
            width: 480,
            height: 640,
            num_components: 3,
            bit_depth: 8,
            colour_specification_method: ColourSpecificationMethods::EnumeratedColourSpace {
                code: EnumeratedColourSpaces::sYCC,
            },
            has_unexpected_approx_set: true,
        },
    );

    let header_box = boxes.header_box().as_ref().unwrap();
    assert!(header_box.channel_definition_box.is_some());
    let cdef = header_box.channel_definition_box.as_ref().unwrap();
    assert_eq!(cdef.identifier(), *b"cdef");
    /*
     From the associated description file (file2.txt):

     Sub box: "cdef" Channel Definition box
       Channel     #0: 0
       Type        #0: color
       Association #0: 3
       Channel     #1: 1
       Type        #1: color
       Association #1: 2
       Channel     #2: 2
       Type        #2: color
       Association #2: 1
    */
    assert_eq!(cdef.channels().len(), 3);
    assert_eq!(cdef.channels()[0].channel_index(), 0);
    assert_eq!(cdef.channels()[0].channel_type_u16(), 0);
    assert_eq!(
        cdef.channels()[0].channel_type(),
        ChannelTypes::ColourImageData
    );
    assert_eq!(cdef.channels()[0].channel_association(), 3);
    assert_eq!(cdef.channels()[1].channel_index(), 1);
    assert_eq!(cdef.channels()[1].channel_type_u16(), 0);
    assert_eq!(
        cdef.channels()[1].channel_type(),
        ChannelTypes::ColourImageData
    );
    assert_eq!(cdef.channels()[1].channel_association(), 2);
    assert_eq!(cdef.channels()[2].channel_index(), 2);
    assert_eq!(cdef.channels()[2].channel_type_u16(), 0);
    assert_eq!(
        cdef.channels()[2].channel_type(),
        ChannelTypes::ColourImageData
    );
    assert_eq!(cdef.channels()[2].channel_association(), 1);

    assert!(header_box.palette_box.is_none());
    assert!(header_box.component_mapping_box.is_none());

    assert_eq!(boxes.xml_boxes().len(), 0);

    assert_eq!(boxes.uuid_boxes().len(), 0);
}

#[test]
fn test_sample_file3() {
    let boxes = test_sample_jp2_file(
        "file3.jp2",
        ExpectedConfiguration {
            compatibility_list: vec!["\0\0\0\u{1}".into(), "jp2 ".into()],
            width: 480,
            height: 640,
            num_components: 3,
            bit_depth: 8,
            colour_specification_method: ColourSpecificationMethods::EnumeratedColourSpace {
                code: EnumeratedColourSpaces::sYCC,
            },
            has_unexpected_approx_set: true,
        },
    );

    let header_box = boxes.header_box().as_ref().unwrap();
    assert!(header_box.channel_definition_box.is_none());
    assert!(header_box.palette_box.is_none());
    assert!(header_box.component_mapping_box.is_none());

    assert_eq!(boxes.xml_boxes().len(), 0);

    assert_eq!(boxes.uuid_boxes().len(), 0);
}

#[test]
fn test_sample_file4() {
    let boxes = test_sample_jp2_file(
        "file4.jp2",
        ExpectedConfiguration {
            compatibility_list: vec!["\0\0\0\u{1}".into(), "jp2 ".into()],
            width: 768,
            height: 512,
            num_components: 1,
            bit_depth: 8,
            colour_specification_method: ColourSpecificationMethods::EnumeratedColourSpace {
                code: EnumeratedColourSpaces::Greyscale,
            },
            has_unexpected_approx_set: true,
        },
    );

    let header_box = boxes.header_box().as_ref().unwrap();
    assert!(header_box.channel_definition_box.is_none());
    assert!(header_box.palette_box.is_none());
    assert!(header_box.component_mapping_box.is_none());

    assert_eq!(boxes.xml_boxes().len(), 0);

    assert_eq!(boxes.uuid_boxes().len(), 0);
}

#[test]
fn test_sample_file5() {
    let boxes = test_sample_jpx_file(
        "file5.jp2",
        ExpectedConfiguration {
            compatibility_list: vec![
                "\0\0\0\u{3}".into(),
                "jp2 ".into(),
                "jpx ".into(),
                "jpbx".into(),
            ],
            width: 768,
            height: 512,
            num_components: 3,
            bit_depth: 8,
            // fallback method for JPX
            colour_specification_method: ColourSpecificationMethods::EnumeratedColourSpace {
                code: EnumeratedColourSpaces::ROMMRGB,
            },
            has_unexpected_approx_set: true,
        },
    );

    assert!(boxes.reader_requirements_box().is_some());
    let reader_requirements = boxes.reader_requirements_box().as_ref().unwrap();
    assert_eq!(reader_requirements.standard_flags().len(), 3);
    assert_eq!(
        *reader_requirements.standard_flags().get(0).unwrap(),
        ExtensionFeatureRequirement {
            flag: FeatureFlag::UnrestrictedJPEG2000CoreCodingSystemCodestream,
            mask: 128
        }
    );
    assert_eq!(
        *reader_requirements.standard_flags().get(1).unwrap(),
        ExtensionFeatureRequirement {
            flag: FeatureFlag::RommRgbEnumeratedColourspace,
            mask: 96
        }
    );
    assert_eq!(
        *reader_requirements.standard_flags().get(2).unwrap(),
        ExtensionFeatureRequirement {
            flag: FeatureFlag::Deprecated43,
            mask: 64
        }
    );
    let header_box = boxes.header_box().as_ref().unwrap();
    assert!(header_box.channel_definition_box.is_none());
    assert!(header_box.palette_box.is_none());
    assert!(header_box.component_mapping_box.is_none());
    let primary_colour_specification_box = header_box.colour_specification_boxes.first().unwrap();
    assert_eq!(primary_colour_specification_box.precedence(), 0);
    assert_eq!(
        primary_colour_specification_box.colourspace_approximation(),
        1
    );
    assert_eq!(
        primary_colour_specification_box.method(),
        &ColourSpecificationMethods::RestrictedICCProfile {
            profile_data: include_bytes!("file5_colr_ricc.dat").to_vec()
        }
    );

    assert_eq!(boxes.xml_boxes().len(), 0);

    assert_eq!(boxes.uuid_boxes().len(), 0);
}

#[test]
fn test_sample_file6() {
    let boxes = test_sample_jp2_file(
        "file6.jp2",
        ExpectedConfiguration {
            compatibility_list: vec!["\0\0\0\u{1}".into(), "jp2 ".into()],
            width: 768,
            height: 512,
            num_components: 1,
            bit_depth: 12,
            colour_specification_method: ColourSpecificationMethods::EnumeratedColourSpace {
                code: EnumeratedColourSpaces::Greyscale,
            },
            has_unexpected_approx_set: true,
        },
    );

    let header_box = boxes.header_box().as_ref().unwrap();
    assert!(header_box.channel_definition_box.is_none());
    assert!(header_box.palette_box.is_none());
    assert!(header_box.component_mapping_box.is_none());

    assert_eq!(boxes.xml_boxes().len(), 0);

    assert_eq!(boxes.uuid_boxes().len(), 0);
}

#[test]
fn test_sample_file7() {
    let boxes = test_sample_jpx_file(
        "file7.jp2",
        ExpectedConfiguration {
            compatibility_list: vec![
                "\0\0\0\u{3}".into(),
                "jp2 ".into(),
                "jpx ".into(),
                "jpxb".into(),
            ],
            width: 480,
            height: 640,
            num_components: 3,
            bit_depth: 16,
            // fallback method for JPX
            colour_specification_method: ColourSpecificationMethods::EnumeratedColourSpace {
                code: EnumeratedColourSpaces::esRGB,
            },
            has_unexpected_approx_set: true,
        },
    );
    assert!(boxes.reader_requirements_box().is_some());
    let reader_requirements = boxes.reader_requirements_box().as_ref().unwrap();
    assert_eq!(reader_requirements.standard_flags().len(), 3);
    assert_eq!(
        *reader_requirements.standard_flags().get(0).unwrap(),
        ExtensionFeatureRequirement {
            flag: FeatureFlag::UnrestrictedJPEG2000CoreCodingSystemCodestream,
            mask: 128
        }
    );
    assert_eq!(
        *reader_requirements.standard_flags().get(1).unwrap(),
        ExtensionFeatureRequirement {
            flag: FeatureFlag::EsrgbEnumeratedColourspace,
            mask: 96
        }
    );
    assert_eq!(
        *reader_requirements.standard_flags().get(2).unwrap(),
        ExtensionFeatureRequirement {
            flag: FeatureFlag::Deprecated43,
            mask: 64
        }
    );

    let header_box = boxes.header_box().as_ref().unwrap();
    assert!(header_box.channel_definition_box.is_none());
    assert!(header_box.palette_box.is_none());
    assert!(header_box.component_mapping_box.is_none());
    let primary_colour_specification_box = header_box.colour_specification_boxes.first().unwrap();
    assert_eq!(primary_colour_specification_box.precedence(), 0);
    assert_eq!(
        primary_colour_specification_box.colourspace_approximation(),
        1
    );
    assert_eq!(
        primary_colour_specification_box.method(),
        &ColourSpecificationMethods::RestrictedICCProfile {
            profile_data: include_bytes!("file7_colr_ricc.dat").to_vec()
        }
    );

    assert_eq!(boxes.xml_boxes().len(), 0);

    assert_eq!(boxes.uuid_boxes().len(), 0);
}

#[test]
fn test_sample_file8() {
    let boxes = test_sample_jp2_file(
        "file8.jp2",
        ExpectedConfiguration {
            compatibility_list: vec!["\0\0\0\u{1}".into(), "jp2 ".into()],
            width: 700,
            height: 400,
            num_components: 1,
            bit_depth: 8,
            colour_specification_method: ColourSpecificationMethods::RestrictedICCProfile {
                profile_data: vec![
                    0, 0, 1, 158, 0, 0, 0, 0, 2, 32, 0, 0, 115, 99, 110, 114, 71, 82, 65, 89, 88,
                    89, 90, 32, 7, 210, 0, 1, 0, 23, 0, 9, 0, 26, 0, 16, 97, 99, 115, 112, 0, 0, 0,
                    0, 0, 0, 0, 1, 75, 79, 68, 65, 71, 82, 69, 89, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 246, 214, 0, 1, 0, 0, 0, 0, 211, 45, 74, 80, 69, 71, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 100, 101, 115, 99, 0, 0, 0,
                    180, 0, 0, 0, 154, 99, 112, 114, 116, 0, 0, 1, 80, 0, 0, 0, 42, 119, 116, 112,
                    116, 0, 0, 1, 124, 0, 0, 0, 20, 107, 84, 82, 67, 0, 0, 1, 144, 0, 0, 0, 14,
                    100, 101, 115, 99, 0, 0, 0, 0, 0, 0, 0, 64, 82, 101, 115, 116, 114, 105, 99,
                    116, 101, 100, 32, 73, 67, 67, 32, 112, 114, 111, 102, 105, 108, 101, 32, 100,
                    101, 115, 99, 114, 105, 98, 105, 110, 103, 32, 103, 114, 101, 121, 115, 99, 97,
                    108, 101, 32, 118, 101, 114, 115, 105, 111, 110, 32, 111, 102, 32, 82, 79, 77,
                    77, 45, 82, 71, 66, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 116, 101, 120, 116, 0, 0, 0, 0, 67, 111, 112, 121,
                    114, 105, 103, 104, 116, 32, 50, 48, 48, 49, 32, 69, 75, 67, 45, 82, 73, 67,
                    67, 32, 82, 101, 102, 101, 114, 101, 110, 99, 101, 0, 0, 0, 88, 89, 90, 32, 0,
                    0, 0, 0, 0, 0, 246, 214, 0, 1, 0, 0, 0, 0, 211, 45, 99, 117, 114, 118, 0, 0, 0,
                    0, 0, 0, 0, 1, 1, 205,
                ],
            },
            has_unexpected_approx_set: true,
        },
    );

    let header_box = boxes.header_box().as_ref().unwrap();
    assert!(header_box.channel_definition_box.is_none());
    assert!(header_box.palette_box.is_none());
    assert!(header_box.component_mapping_box.is_none());

    assert_eq!(boxes.xml_boxes().len(), 2);
    let xml0 = boxes.xml_boxes().first().unwrap();
    assert_eq!(xml0.identifier(), *b"xml ");

    assert_eq!(boxes.uuid_boxes().len(), 0);
}

#[test]
fn test_sample_file9() {
    let boxes = test_sample_jp2_file(
        "file9.jp2",
        ExpectedConfiguration {
            compatibility_list: vec!["\0\0\0\u{1}".into(), "jp2 ".into()],
            width: 768,
            height: 512,
            num_components: 1,
            bit_depth: 8,
            colour_specification_method: ColourSpecificationMethods::EnumeratedColourSpace {
                code: EnumeratedColourSpaces::sRGB,
            },
            has_unexpected_approx_set: true,
        },
    );

    let header_box = boxes.header_box().as_ref().unwrap();
    assert!(header_box.channel_definition_box.is_none());
    assert!(header_box.palette_box.is_some());
    /* From the description text (file9.txt):
    Sub box: "pclr" (Palette box)

    Entries: 256
    Created Channels: 3
    Depth  #0: 8
    Signed #0: no
    Depth  #1: 8
    Signed #1: no
    Depth  #2: 8
    Signed #2: no
    Entry #000: 0x0000000000 0x0000000000 0x0000000000
    Entry #001: 0x00000000ff 0x00000000ff 0x00000000ff
    Entry #002: 0x0000000017 0x000000000c 0x0000000015
    ...
    Entry #213: 0x000000006a 0x0000000055 0x000000003b
    Entry #214: 0x00000000a5 0x0000000084 0x000000005c
    Entry #215: 0x0000000079 0x0000000069 0x0000000056
    ...
    Entry #252: 0x0000000016 0x000000000b 0x0000000009
    Entry #253: 0x00000000fd 0x00000000f5 0x00000000f5
    Entry #254: 0x00000000fd 0x00000000fd 0x00000000fd
    Entry #255: 0x00000000f5 0x00000000f5 0x00000000f5
     */
    let pclr = header_box.palette_box.as_ref().unwrap();
    assert_eq!(pclr.identifier(), *b"pclr");
    assert_eq!(pclr.num_components(), 3);
    assert_eq!(pclr.num_entries(), 256);

    assert!(pclr.bit_depth(0).is_some());
    assert_eq!(*pclr.bit_depth(0).unwrap(), BitDepth::Unsigned { value: 8 });
    assert!(pclr.bit_depth(1).is_some());
    assert_eq!(*pclr.bit_depth(1).unwrap(), BitDepth::Unsigned { value: 8 });
    assert!(pclr.bit_depth(2).is_some());
    assert_eq!(*pclr.bit_depth(2).unwrap(), BitDepth::Unsigned { value: 8 });
    assert!(pclr.bit_depth(3).is_none());

    assert!(pclr.entry(0, 0).is_some());
    assert_eq!(*pclr.entry(0, 0).unwrap(), 0);
    assert_eq!(*pclr.entry(0, 1).unwrap(), 0);
    assert_eq!(*pclr.entry(0, 2).unwrap(), 0);
    assert!(pclr.entry(0, 3).is_none());

    assert!(pclr.entry(1, 0).is_some());
    assert_eq!(*pclr.entry(1, 0).unwrap(), 0xff);
    assert_eq!(*pclr.entry(1, 1).unwrap(), 0xff);
    assert_eq!(*pclr.entry(1, 2).unwrap(), 0xff);
    assert!(pclr.entry(1, 3).is_none());

    assert!(pclr.entry(2, 0).is_some());
    assert_eq!(*pclr.entry(2, 0).unwrap(), 0x17);
    assert_eq!(*pclr.entry(2, 1).unwrap(), 0x0c);
    assert_eq!(*pclr.entry(2, 2).unwrap(), 0x15);
    assert!(pclr.entry(2, 3).is_none());

    assert!(pclr.entry(214, 0).is_some());
    assert_eq!(*pclr.entry(214, 0).unwrap(), 0xa5);
    assert_eq!(*pclr.entry(214, 1).unwrap(), 0x84);
    assert_eq!(*pclr.entry(214, 2).unwrap(), 0x5c);
    assert!(pclr.entry(214, 3).is_none());

    assert!(pclr.entry(252, 0).is_some());
    assert_eq!(*pclr.entry(252, 0).unwrap(), 0x16);
    assert_eq!(*pclr.entry(252, 1).unwrap(), 0x0b);
    assert_eq!(*pclr.entry(252, 2).unwrap(), 0x09);
    assert!(pclr.entry(252, 3).is_none());

    assert!(pclr.entry(255, 0).is_some());
    assert_eq!(*pclr.entry(255, 0).unwrap(), 0xf5);
    assert_eq!(*pclr.entry(255, 1).unwrap(), 0xf5);
    assert_eq!(*pclr.entry(255, 2).unwrap(), 0xf5);
    assert!(pclr.entry(255, 3).is_none());

    assert!(pclr.entry(256, 0).is_none());
    assert!(pclr.entry(256, 1).is_none());
    assert!(pclr.entry(256, 2).is_none());
    assert!(pclr.entry(256, 3).is_none());

    assert!(header_box.component_mapping_box.is_some());
    /* From the description text (file9.txt):

       Sub box: "cmap" Component Mapping box
           Component      #0: 0
           Mapping Type   #0: palette mapping
           Palette Column #0: 0
           Component      #1: 0
           Mapping Type   #1: palette mapping
           Palette Column #1: 1
           Component      #2: 0
           Mapping Type   #2: palette mapping
           Palette Column #2: 2
    */
    let cmap = header_box.component_mapping_box.as_ref().unwrap();
    assert_eq!(cmap.identifier(), *b"cmap");
    assert_eq!(cmap.component_map().len(), 3);
    assert_eq!(cmap.component_map()[0].component(), 0);
    assert_eq!(cmap.component_map()[0].mapping_type(), 1);
    assert_eq!(cmap.component_map()[0].palette(), 0);
    assert_eq!(cmap.component_map()[1].component(), 0);
    assert_eq!(cmap.component_map()[1].mapping_type(), 1);
    assert_eq!(cmap.component_map()[1].palette(), 1);
    assert_eq!(cmap.component_map()[2].component(), 0);
    assert_eq!(cmap.component_map()[2].mapping_type(), 1);
    assert_eq!(cmap.component_map()[2].palette(), 2);

    assert_eq!(boxes.xml_boxes().len(), 0);

    assert_eq!(boxes.uuid_boxes().len(), 0);
}

#[test]
fn test_sample_subsampling1() {
    let boxes = test_sample_jp2_file(
        "subsampling_1.jp2",
        ExpectedConfiguration {
            compatibility_list: vec!["jp2 ".into()],
            width: 1280,
            height: 1024,
            num_components: 3,
            bit_depth: 8,
            colour_specification_method: ColourSpecificationMethods::EnumeratedColourSpace {
                code: EnumeratedColourSpaces::sYCC,
            },
            has_unexpected_approx_set: false,
        },
    );

    let header_box = boxes.header_box().as_ref().unwrap();
    assert!(header_box.channel_definition_box.is_none());
    assert!(header_box.palette_box.is_none());
    assert!(header_box.component_mapping_box.is_none());

    assert_eq!(boxes.xml_boxes().len(), 0);

    assert_eq!(boxes.uuid_boxes().len(), 1);
}

#[test]
fn test_sample_subsampling2() {
    let boxes = test_sample_jp2_file(
        "subsampling_2.jp2",
        ExpectedConfiguration {
            compatibility_list: vec!["jp2 ".into()],
            width: 1280,
            height: 1024,
            num_components: 3,
            bit_depth: 8,
            colour_specification_method: ColourSpecificationMethods::EnumeratedColourSpace {
                code: EnumeratedColourSpaces::sRGB,
            },
            has_unexpected_approx_set: false,
        },
    );

    let header_box = boxes.header_box().as_ref().unwrap();
    assert!(header_box.channel_definition_box.is_none());
    assert!(header_box.palette_box.is_none());
    assert!(header_box.component_mapping_box.is_none());

    assert_eq!(boxes.xml_boxes().len(), 0);

    assert_eq!(boxes.uuid_boxes().len(), 1);
    let uuid = boxes.uuid_boxes().first().unwrap();
    assert_eq!(uuid.identifier(), *b"uuid");
}

#[test]
fn test_sample_zoo1() {
    let boxes = test_sample_jp2_file(
        "zoo1.jp2",
        ExpectedConfiguration {
            compatibility_list: vec!["jp2 ".into()],
            width: 3906,
            height: 2602,
            num_components: 3,
            bit_depth: 8,
            colour_specification_method: ColourSpecificationMethods::EnumeratedColourSpace {
                code: EnumeratedColourSpaces::sYCC,
            },
            has_unexpected_approx_set: false,
        },
    );

    let header_box = boxes.header_box().as_ref().unwrap();
    assert!(header_box.channel_definition_box.is_none());
    assert!(header_box.palette_box.is_none());
    assert!(header_box.component_mapping_box.is_none());

    assert_eq!(boxes.xml_boxes().len(), 0);

    assert_eq!(boxes.uuid_boxes().len(), 1);
}

#[test]
fn test_sample_zoo2() {
    let boxes = test_sample_jp2_file(
        "zoo2.jp2",
        ExpectedConfiguration {
            compatibility_list: vec!["jp2 ".into()],
            width: 3906,
            height: 2602,
            num_components: 3,
            bit_depth: 8,
            colour_specification_method: ColourSpecificationMethods::EnumeratedColourSpace {
                code: EnumeratedColourSpaces::sRGB,
            },
            has_unexpected_approx_set: false,
        },
    );

    let header_box = boxes.header_box().as_ref().unwrap();
    assert!(header_box.channel_definition_box.is_none());
    assert!(header_box.palette_box.is_none());
    assert!(header_box.component_mapping_box.is_none());

    assert_eq!(boxes.xml_boxes().len(), 0);

    assert_eq!(boxes.uuid_boxes().len(), 1);
}

fn test_sample_jp2_file(filename: &str, expected: ExpectedConfiguration) -> JP2File {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../samples")
        .join(filename);
    let file = File::open(path).expect("file should exist");
    let mut reader = BufReader::new(file);
    let result = decode_jp2(&mut reader);
    assert!(result.is_ok());
    let boxes = result.unwrap();
    assert!(boxes.length() > 0);

    check_signature_box(&boxes);
    assert!(boxes.reader_requirements_box().is_none());
    check_file_type_box("jp2 ".into(), &expected.compatibility_list, &boxes);

    assert!(boxes.reader_requirements_box().is_none());

    check_jp2_header_box(expected, &boxes);

    check_contiguous_codestream_box(&boxes);

    boxes
}

fn check_jp2_header_box_jpx(expected: ExpectedConfiguration, boxes: &JP2File) {
    assert!(boxes.header_box().is_some());
    let header_box = boxes.header_box().as_ref().unwrap();
    check_image_header_box(&expected, header_box);

    assert_eq!(header_box.colour_specification_boxes.len(), 2);
    let fallback_colour_specification_box = header_box.colour_specification_boxes.last().unwrap();
    let colourspace_approximation = if expected.has_unexpected_approx_set {
        1u8
    } else {
        0u8
    };
    check_part1_colour_specification_box(
        1,
        colourspace_approximation,
        &expected.colour_specification_method,
        fallback_colour_specification_box,
    );

    assert!(header_box.resolution_box.is_none());
}

fn check_signature_box(boxes: &JP2File) {
    assert!(boxes.signature_box().is_some());
    let signature = boxes.signature_box().as_ref().unwrap();
    assert_eq!(signature.identifier(), *b"jP  ");
    assert_eq!(signature.signature(), *b"\x0d\x0a\x87\x0a");
}

fn check_file_type_box(
    expected_major_brand: String,
    expected_compatibility_list: &Vec<String>,
    boxes: &JP2File,
) {
    assert!(boxes.file_type_box().is_some());
    let file_type = boxes.file_type_box().as_ref().unwrap();
    assert_eq!(file_type.identifier(), *b"ftyp");
    assert_eq!(file_type.brand(), expected_major_brand);
    assert_eq!(file_type.min_version(), 0);
    assert_eq!(file_type.compatibility_list(), *expected_compatibility_list);
}

fn check_jp2_header_box(expected: ExpectedConfiguration, boxes: &JP2File) {
    assert!(boxes.header_box().is_some());
    let header_box = boxes.header_box().as_ref().unwrap();
    assert_eq!(header_box.identifier(), *b"jp2h");

    check_image_header_box(&expected, header_box);

    assert!(header_box.bits_per_component_box.is_none());
    assert_eq!(header_box.colour_specification_boxes.len(), 1);
    let colour_specification_box = header_box.colour_specification_boxes.first().unwrap();

    let colourspace_approximation = if expected.has_unexpected_approx_set {
        1u8
    } else {
        0u8
    };

    check_part1_colour_specification_box(
        0,
        colourspace_approximation,
        &expected.colour_specification_method,
        colour_specification_box,
    );

    assert!(header_box.resolution_box.is_none());
}

fn check_image_header_box(expected: &ExpectedConfiguration, header_box: &jp2::HeaderSuperBox) {
    let image_header_box = &header_box.image_header_box;
    assert_eq!(image_header_box.identifier(), *b"ihdr");
    assert_eq!(image_header_box.height(), expected.height);
    assert_eq!(image_header_box.width(), expected.width);
    assert_eq!(image_header_box.components_num(), expected.num_components);
    assert_eq!(image_header_box.compression_type(), 7);
    assert_eq!(image_header_box.colourspace_unknown(), 0);
    assert_eq!(image_header_box.intellectual_property(), 0);
    assert_eq!(image_header_box.components_bits(), expected.bit_depth);
    assert_eq!(image_header_box.values_are_signed(), false);
}

fn check_part1_colour_specification_box(
    expected_precedence: i8,
    expected_approximation: u8,
    expected_colour_specification_method: &ColourSpecificationMethods,
    colour_specification_box: &jp2::colour_specification::ColourSpecificationBox,
) {
    assert_eq!(colour_specification_box.identifier(), *b"colr");
    assert_eq!(colour_specification_box.precedence(), expected_precedence);
    assert_eq!(
        colour_specification_box.colourspace_approximation(),
        expected_approximation
    );
    assert_eq!(
        colour_specification_box.method(),
        expected_colour_specification_method
    );
}

fn check_contiguous_codestream_box(boxes: &JP2File) {
    assert_eq!(boxes.contiguous_codestreams_boxes().len(), 1);
    let codestream_box = boxes.contiguous_codestreams_boxes().first().unwrap();
    assert_eq!(codestream_box.identifier(), *b"jp2c");
    assert!(codestream_box.length() > 0);
    assert!(codestream_box.offset() > 0);
}

fn test_sample_jpx_file(filename: &str, expected: ExpectedConfiguration) -> JP2File {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../samples")
        .join(filename);
    let file = File::open(path).expect("file should exist");
    let mut reader = BufReader::new(file);
    let result = decode_jp2(&mut reader);
    assert!(result.is_ok());
    let boxes = result.unwrap();
    assert!(boxes.length() > 0);

    check_signature_box(&boxes);
    assert!(boxes.reader_requirements_box().is_some());
    check_file_type_box("jpx ".into(), &expected.compatibility_list, &boxes);

    assert!(boxes.reader_requirements_box().is_some());

    check_jp2_header_box_jpx(expected, &boxes);

    check_contiguous_codestream_box(&boxes);

    boxes
}

#[test]
fn test_geojp2() {
    // GeoJP2, as implemented by GDAL
    // Tests UUID and XML boxes
    let boxes = test_jp2_file(
        "geojp2.jp2",
        ExpectedConfiguration {
            compatibility_list: vec!["jp2 ".into()],
            width: 100,
            height: 24,
            num_components: 1,
            bit_depth: 8,
            colour_specification_method: ColourSpecificationMethods::EnumeratedColourSpace {
                code: EnumeratedColourSpaces::Greyscale,
            },
            has_unexpected_approx_set: false,
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

    assert!(boxes.reader_requirements_box().is_none());

    assert!(boxes.file_type_box().is_some());
    let file_type = boxes.file_type_box().as_ref().unwrap();
    assert_eq!(file_type.brand(), "jp2 ");
    assert_eq!(file_type.min_version(), 0);
    assert_eq!(file_type.compatibility_list(), expected.compatibility_list);

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
        *colour_specification_box.method(),
        expected.colour_specification_method,
    );
    assert_eq!(colour_specification_box.precedence(), 0);
    assert_eq!(colour_specification_box.colourspace_approximation(), 0u8);

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

#[test]
fn test_j2pi() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("j2pi.jp2");
    let file = File::open(path).expect("file should exist");
    let mut reader = BufReader::new(file);
    let result = decode_jp2(&mut reader);
    assert!(result.is_ok());
    let boxes = result.unwrap();

    assert!(boxes.reader_requirements_box().is_none());

    assert!(boxes.header_box().is_some());
    let header_box = boxes.header_box().as_ref().unwrap();
    let image_header_box = &header_box.image_header_box;
    assert_eq!(image_header_box.height(), 2);
    assert_eq!(image_header_box.width(), 3);
    assert_eq!(image_header_box.components_num(), 1);
    assert_eq!(image_header_box.intellectual_property(), 1);
    assert_eq!(image_header_box.components_bits(), 8);
    assert_eq!(image_header_box.values_are_signed(), false);

    assert_eq!(boxes.contiguous_codestreams_boxes().len(), 1);

    assert!(boxes.intellectual_property_box().is_some());
    let jp2i = boxes.intellectual_property_box().as_ref().unwrap();
    assert_eq!(jp2i.identifier(), *b"jp2i");
    assert_eq!(jp2i.length(), 469);
    assert_eq!(jp2i.format(), "<?xml version=\"1.0\"?>\n<!-- markings are for test purposes only, content is public release -->\n<jp:IPR xmlns:jp=\"http://www.jpeg.org/jpx/1.0/xml\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\n<jp:IPR_EXPLOITATION>\n<jp:IPR_USE_RESTRICTION>unclassified</jp:IPR_USE_RESTRICTION>\n<jp:IPR_MGMT_SYS>\n<jp:IPR_MGMT_TYPE>SWE</jp:IPR_MGMT_TYPE>\n</jp:IPR_MGMT_SYS>\n<jp:IPR_PROTECTION>SWE;FRA;USA;GBR;ARE;ZAF;DEU;ITA;CZE</jp:IPR_PROTECTION>\n</jp:IPR_EXPLOITATION>\n</jp:IPR>");

    assert_eq!(boxes.xml_boxes().len(), 0);

    assert_eq!(boxes.uuid_boxes().len(), 0);
}

#[test]
fn test_res_boxes() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("res_boxes.jp2");
    let file = File::open(path).expect("file should exist");
    let mut reader = BufReader::new(file);
    let result = decode_jp2(&mut reader);
    assert!(result.is_ok());
    let boxes = result.unwrap();

    assert!(boxes.header_box().is_some());
    let header_box = boxes.header_box().as_ref().unwrap();
    let image_header_box = &header_box.image_header_box;
    assert_eq!(image_header_box.height(), 200);
    assert_eq!(image_header_box.width(), 200);
    assert_eq!(image_header_box.components_num(), 1);
    assert_eq!(image_header_box.intellectual_property(), 0);
    assert_eq!(image_header_box.components_bits(), 8);
    assert_eq!(image_header_box.values_are_signed(), false);

    assert!(header_box.channel_definition_box.is_none());
    assert!(header_box.resolution_box.is_some());
    let res = header_box.resolution_box.as_ref().unwrap();
    assert_eq!(res.identifier(), *b"res ");
    assert!(res.capture_resolution_box().is_some());
    let resc = res.capture_resolution_box().as_ref().unwrap();
    assert_eq!(resc.identifier(), *b"resc");
    /* From jpylyzer:
        <vRcN>20</vRcN>
        <vRcD>1</vRcD>
        <hRcN>25</hRcN>
        <hRcD>1</hRcD>
        <vRcE>0</vRcE>
        <hRcE>0</hRcE>
    */
    assert_eq!(resc.vertical_capture_grid_resolution_numerator(), 20);
    assert_eq!(resc.vertical_capture_grid_resolution_denominator(), 1);
    assert_eq!(resc.horizontal_capture_grid_resolution_numerator(), 25);
    assert_eq!(resc.horizontal_capture_grid_resolution_denominator(), 1);
    assert_eq!(resc.vertical_capture_grid_resolution_exponent(), 0);
    assert_eq!(resc.horizontal_capture_grid_resolution_exponent(), 0);
    assert_eq!(resc.vertical_resolution_capture(), 20.0);
    assert_eq!(resc.horizontal_resolution_capture(), 25.0);

    assert!(res.default_display_resolution_box().is_some());
    let resd = res.default_display_resolution_box().as_ref().unwrap();
    assert_eq!(resd.identifier(), *b"resd");
    /* From jpylyzer:
        <vRdN>300</vRdN>
        <vRdD>1</vRdD>
        <hRdN>375</hRdN>
        <hRdD>1</hRdD>
        <vRdE>0</vRdE>
        <hRdE>0</hRdE>
    */
    assert_eq!(resd.vertical_display_grid_resolution_numerator(), 300);
    assert_eq!(resd.vertical_display_grid_resolution_denominator(), 1);
    assert_eq!(resd.horizontal_display_grid_resolution_numerator(), 375);
    assert_eq!(resd.horizontal_display_grid_resolution_denominator(), 1);
    assert_eq!(resd.vertical_display_grid_resolution_exponent(), 0);
    assert_eq!(resd.horizontal_display_grid_resolution_exponent(), 0);
    assert_eq!(resd.vertical_display_grid_resolution(), 300.0);
    assert_eq!(resd.horizontal_display_grid_resolution(), 375.0);

    assert_eq!(boxes.contiguous_codestreams_boxes().len(), 1);

    assert_eq!(boxes.xml_boxes().len(), 0);

    assert_eq!(boxes.uuid_boxes().len(), 0);
}

#[test]
fn test_hirise_modified() {
    // HIRISE image of Mars.
    // Original image is from https://www.uahirise.org/catalog/
    // Replaced the body of the codestream with a single 0x00 byte to make size reasonable
    // Tests UUID info box (and child boxes)

    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("hirise_modified.jp2");
    let file = File::open(path).expect("file should exist");
    let mut reader = BufReader::new(file);
    let result = decode_jp2(&mut reader);
    assert!(result.is_ok());
    let boxes = result.unwrap();
    assert!(boxes.length() > 0);

    assert!(boxes.signature_box().is_some());
    assert!(boxes.reader_requirements_box().is_none());
    assert!(boxes.file_type_box().is_some());
    assert!(boxes.header_box().is_some());
    let header_box = boxes.header_box().as_ref().unwrap();
    let image_header_box = &header_box.image_header_box;
    assert_eq!(image_header_box.height(), 16754);
    assert_eq!(image_header_box.width(), 4246);
    assert_eq!(image_header_box.components_num(), 3);
    assert_eq!(image_header_box.compression_type(), 7);
    assert_eq!(image_header_box.colourspace_unknown(), 1);
    assert_eq!(image_header_box.intellectual_property(), 0);
    assert_eq!(image_header_box.components_bits(), 10);
    assert_eq!(image_header_box.values_are_signed(), false);

    assert!(header_box.bits_per_component_box.is_none());

    assert_eq!(header_box.colour_specification_boxes.len(), 1);
    let colour_specification_box = header_box.colour_specification_boxes.first().unwrap();
    assert_eq!(
        *colour_specification_box.method(),
        ColourSpecificationMethods::EnumeratedColourSpace {
            code: EnumeratedColourSpaces::sRGB
        },
    );
    assert_eq!(colour_specification_box.precedence(), 0);
    assert_eq!(colour_specification_box.colourspace_approximation(), 0u8);

    assert!(header_box.palette_box.is_none());

    assert!(header_box.component_mapping_box.is_none());

    assert!(header_box.channel_definition_box.is_none());

    assert!(header_box.resolution_box.is_none());

    assert_eq!(boxes.contiguous_codestreams_boxes().len(), 1);
    let codestream_box = boxes.contiguous_codestreams_boxes().first().unwrap();
    assert!(codestream_box.length() > 0);
    assert!(codestream_box.offset() > 0);
    assert_eq!(boxes.xml_boxes().len(), 0);
    assert_eq!(boxes.uuid_boxes().len(), 1);
    let uuid = boxes.uuid_boxes().first().unwrap();
    assert_eq!(uuid.length(), 515);
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
    assert_eq!(uuid.data().len(), 499);

    assert_eq!(boxes.uuid_info_boxes().len(), 1);
    let uuid_info = boxes.uuid_info_boxes().first().unwrap();
    assert!(uuid_info.uuid_list_box().is_some());
    assert!(uuid_info.data_entry_url_box().is_some());
    /* jyplyzer results:
            <uuidInfoBox>
            <uuidListBox>
                <nU>1</nU>
                <uuid>2b0d7e97-aa2e-317d-9a33-e53161a2f7d0</uuid>
            </uuidListBox>
            <urlBox>
                <version>0</version>
                <loc>ESP_053795_1905_COLOR.LBL</loc>
            </urlBox>
        </uuidInfoBox>
    */
    let ulst = uuid_info.uuid_list_box().as_ref().unwrap();
    assert_eq!(ulst.number_of_uuids(), 1);
    assert_eq!(ulst.ids().len(), 1);
    assert_eq!(
        *ulst.ids().first().unwrap(),
        [
            0x2b, 0x0d, 0x7e, 0x97, 0xaa, 0x2e, 0x31, 0x7d, 0x9a, 0x33, 0xe5, 0x31, 0x61, 0xa2,
            0xf7, 0xd0
        ]
    );
    let url = uuid_info.data_entry_url_box().as_ref().unwrap();
    assert_eq!(url.version(), 0);
    assert_eq!(*url.flags(), [0u8, 0u8, 0u8]);
    assert!(url.location().is_ok());
    assert_eq!(url.location().unwrap(), "ESP_053795_1905_COLOR.LBL");
}
