use std::{fs::File, io::BufReader, path::Path};

use jp2::{decode_jp2, BitDepth, ChannelTypes, ColourSpecificationMethods, JBox as _, JP2File};

struct ExpectedConfiguration {
    compatibility_list: Vec<String>,
    width: u32,
    height: u32,
    num_components: u16,
    bit_depth: u8,
    colourspace: u32,
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
            colourspace: 16,
            colour_specification_method: ColourSpecificationMethods::EnumeratedColourSpace,
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
            colourspace: 16,
            colour_specification_method: ColourSpecificationMethods::EnumeratedColourSpace,
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
            colourspace: 18,
            colour_specification_method: ColourSpecificationMethods::EnumeratedColourSpace,
            has_unexpected_approx_set: true,
        },
    );

    let header_box = boxes.header_box().as_ref().unwrap();
    assert!(header_box.channel_definition_box.is_some());
    let cdef = header_box.channel_definition_box.as_ref().unwrap();
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
            colourspace: 18,
            colour_specification_method: ColourSpecificationMethods::EnumeratedColourSpace,
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
            colourspace: 17,
            colour_specification_method: ColourSpecificationMethods::EnumeratedColourSpace,
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

#[ignore = "uses unsupported Part 2 extensions"]
#[test]
fn test_sample_file5() {
    let boxes = test_sample_jp2_file(
        "file5.jp2",
        ExpectedConfiguration {
            compatibility_list: vec!["\0\0\0\u{1}".into(), "jp2 ".into()],
            width: 640,
            height: 480,
            num_components: 3,
            bit_depth: 16,
            colourspace: 16,
            colour_specification_method: ColourSpecificationMethods::EnumeratedColourSpace,
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
fn test_sample_file6() {
    let boxes = test_sample_jp2_file(
        "file6.jp2",
        ExpectedConfiguration {
            compatibility_list: vec!["\0\0\0\u{1}".into(), "jp2 ".into()],
            width: 768,
            height: 512,
            num_components: 1,
            bit_depth: 12,
            colourspace: 17,
            colour_specification_method: ColourSpecificationMethods::EnumeratedColourSpace,
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

#[ignore = "uses unsupported Part 2 extensions"]
#[test]
fn test_sample_file7() {
    let boxes = test_sample_jp2_file(
        "file7.jp2",
        ExpectedConfiguration {
            compatibility_list: vec!["\0\0\0\u{1}".into(), "jp2 ".into()],
            width: 640,
            height: 480,
            num_components: 3,
            bit_depth: 16,
            colourspace: 16,
            colour_specification_method: ColourSpecificationMethods::EnumeratedColourSpace,
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
fn test_sample_file8() {
    let boxes = test_sample_jp2_file(
        "file8.jp2",
        ExpectedConfiguration {
            compatibility_list: vec!["\0\0\0\u{1}".into(), "jp2 ".into()],
            width: 700,
            height: 400,
            num_components: 1,
            bit_depth: 8,
            colourspace: 0, // Not present
            colour_specification_method: ColourSpecificationMethods::RestrictedICCProfile,
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
fn test_sample_file9() {
    let boxes = test_sample_jp2_file(
        "file9.jp2",
        ExpectedConfiguration {
            compatibility_list: vec!["\0\0\0\u{1}".into(), "jp2 ".into()],
            width: 768,
            height: 512,
            num_components: 1,
            bit_depth: 8,
            colourspace: 16,
            colour_specification_method: ColourSpecificationMethods::EnumeratedColourSpace,
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
    Entry #003: 0x0000000025 0x0000000021 0x0000000025
    Entry #004: 0x00000000f5 0x00000000f5 0x00000000fd
    Entry #005: 0x000000007a 0x00000000ac 0x00000000e6
    Entry #006: 0x0000000004 0x000000006c 0x00000000d5
    Entry #007: 0x0000000068 0x00000000a8 0x00000000e9
    Entry #008: 0x000000008d 0x00000000b8 0x00000000e4
    Entry #009: 0x0000000004 0x000000007b 0x00000000e5
    Entry #010: 0x0000000004 0x0000000074 0x00000000dd
    Entry #011: 0x0000000004 0x000000006c 0x00000000cc
    Entry #012: 0x000000000c 0x000000001d 0x000000002c
    Entry #013: 0x000000004b 0x00000000a1 0x00000000ec
    Entry #014: 0x000000001b 0x0000000029 0x0000000036
    Entry #015: 0x0000000004 0x0000000085 0x00000000ed
    Entry #016: 0x0000000004 0x0000000074 0x00000000d5
    Entry #017: 0x000000000c 0x0000000085 0x00000000ed
    Entry #018: 0x000000000c 0x0000000084 0x00000000e5
    Entry #019: 0x000000000c 0x000000007c 0x00000000dd
    Entry #020: 0x000000000f 0x0000000074 0x00000000c8
    Entry #021: 0x0000000056 0x00000000ac 0x00000000f5
    Entry #022: 0x0000000057 0x00000000a9 0x00000000ec
    Entry #023: 0x0000000023 0x000000003a 0x000000004d
    Entry #024: 0x0000000098 0x00000000c5 0x00000000ec
    Entry #025: 0x0000000004 0x000000008d 0x00000000f5
    Entry #026: 0x0000000004 0x0000000085 0x00000000e5
    Entry #027: 0x0000000004 0x000000007c 0x00000000dd
    Entry #028: 0x0000000004 0x000000007c 0x00000000d5
    Entry #029: 0x0000000004 0x0000000074 0x00000000cd
    Entry #030: 0x000000000c 0x000000008d 0x00000000ed
    Entry #031: 0x000000000c 0x000000007c 0x00000000d5
    Entry #032: 0x000000000d 0x0000000085 0x00000000dd
    Entry #033: 0x0000000010 0x000000007c 0x00000000cd
    Entry #034: 0x0000000016 0x000000008b 0x00000000e7
    Entry #035: 0x0000000021 0x0000000082 0x00000000cd
    Entry #036: 0x0000000048 0x00000000aa 0x00000000f5
    Entry #037: 0x0000000058 0x00000000a1 0x00000000d7
    Entry #038: 0x0000000068 0x00000000b8 0x00000000f5
    Entry #039: 0x000000007c 0x00000000c1 0x00000000f3
    Entry #040: 0x000000007a 0x00000000a1 0x00000000bd
    Entry #041: 0x000000003b 0x0000000049 0x0000000054
    Entry #042: 0x0000000004 0x000000008d 0x00000000ed
    Entry #043: 0x0000000004 0x0000000085 0x00000000dd
    Entry #044: 0x0000000004 0x000000007c 0x00000000cd
    Entry #045: 0x0000000004 0x0000000074 0x00000000c4
    Entry #046: 0x000000000c 0x000000008d 0x00000000e5
    Entry #047: 0x0000000010 0x0000000086 0x00000000d5
    Entry #048: 0x0000000014 0x0000000068 0x00000000a5
    Entry #049: 0x0000000023 0x0000000092 0x00000000dd
    Entry #050: 0x0000000028 0x000000009d 0x00000000ef
    Entry #051: 0x0000000037 0x00000000a7 0x00000000f1
    Entry #052: 0x0000000037 0x000000009f 0x00000000ea
    Entry #053: 0x0000000034 0x0000000090 0x00000000d0
    Entry #054: 0x000000003f 0x000000009a 0x00000000da
    Entry #055: 0x0000000059 0x00000000b6 0x00000000f5
    Entry #056: 0x0000000055 0x0000000093 0x00000000be
    Entry #057: 0x0000000050 0x0000000059 0x000000005f
    Entry #058: 0x0000000004 0x0000000095 0x00000000ed
    Entry #059: 0x0000000004 0x000000008d 0x00000000e5
    Entry #060: 0x0000000004 0x0000000085 0x00000000d5
    Entry #061: 0x0000000004 0x000000007c 0x00000000c5
    Entry #062: 0x0000000007 0x0000000099 0x00000000f5
    Entry #063: 0x000000000c 0x000000008d 0x00000000dd
    Entry #064: 0x000000000e 0x0000000095 0x00000000ed
    Entry #065: 0x0000000018 0x00000000a0 0x00000000f5
    Entry #066: 0x000000002d 0x000000007d 0x00000000b0
    Entry #067: 0x0000000044 0x00000000ac 0x00000000ed
    Entry #068: 0x0000000039 0x0000000071 0x0000000097
    Entry #069: 0x0000000032 0x000000005a 0x0000000072
    Entry #070: 0x0000000004 0x0000000095 0x00000000e5
    Entry #071: 0x0000000004 0x000000008d 0x00000000dd
    Entry #072: 0x0000000004 0x0000000086 0x00000000cd
    Entry #073: 0x000000000d 0x0000000095 0x00000000e5
    Entry #074: 0x0000000012 0x0000000094 0x00000000dc
    Entry #075: 0x0000000017 0x000000009d 0x00000000ec
    Entry #076: 0x0000000021 0x000000009f 0x00000000e4
    Entry #077: 0x0000000028 0x00000000a7 0x00000000ef
    Entry #078: 0x0000000048 0x00000000b6 0x00000000f5
    Entry #079: 0x0000000058 0x00000000b7 0x00000000ed
    Entry #080: 0x000000006a 0x000000008a 0x000000009b
    Entry #081: 0x0000000004 0x000000009e 0x00000000ed
    Entry #082: 0x0000000004 0x0000000095 0x00000000dd
    Entry #083: 0x0000000004 0x000000008d 0x00000000d5
    Entry #084: 0x000000000c 0x000000009f 0x00000000ed
    Entry #085: 0x000000000c 0x000000009e 0x00000000e5
    Entry #086: 0x0000000018 0x00000000a6 0x00000000ed
    Entry #087: 0x0000000048 0x00000000b5 0x00000000ed
    Entry #088: 0x0000000004 0x000000009d 0x00000000e5
    Entry #089: 0x0000000017 0x00000000a5 0x00000000e5
    Entry #090: 0x0000000017 0x0000000089 0x00000000bb
    Entry #091: 0x000000000d 0x0000000098 0x00000000d0
    Entry #092: 0x0000000056 0x000000006a 0x0000000071
    Entry #093: 0x000000009d 0x00000000b4 0x00000000bc
    Entry #094: 0x000000005b 0x0000000063 0x0000000066
    Entry #095: 0x0000000009 0x000000000c 0x000000000d
    Entry #096: 0x0000000067 0x0000000078 0x000000007d
    Entry #097: 0x000000002d 0x0000000036 0x0000000038
    Entry #098: 0x0000000085 0x0000000099 0x000000009c
    Entry #099: 0x00000000ed 0x00000000fc 0x00000000fd
    Entry #100: 0x00000000f5 0x00000000fd 0x00000000fd
    Entry #101: 0x000000000a 0x0000000018 0x0000000017
    Entry #102: 0x0000000038 0x0000000041 0x000000003f
    Entry #103: 0x000000004e 0x0000000053 0x0000000050
    Entry #104: 0x0000000045 0x0000000049 0x0000000046
    Entry #105: 0x0000000028 0x000000002f 0x0000000029
    Entry #106: 0x000000000b 0x0000000017 0x000000000b
    Entry #107: 0x00000000f5 0x00000000fd 0x00000000f5
    Entry #108: 0x000000003a 0x0000000059 0x0000000035
    Entry #109: 0x0000000033 0x0000000059 0x0000000029
    Entry #110: 0x000000005a 0x0000000065 0x0000000057
    Entry #111: 0x0000000022 0x0000000045 0x0000000018
    Entry #112: 0x000000003b 0x0000000065 0x000000002c
    Entry #113: 0x000000002b 0x0000000047 0x000000001f
    Entry #114: 0x000000004b 0x0000000075 0x0000000038
    Entry #115: 0x0000000069 0x0000000070 0x0000000066
    Entry #116: 0x0000000030 0x0000000054 0x000000001d
    Entry #117: 0x000000009d 0x00000000a4 0x0000000099
    Entry #118: 0x000000001e 0x0000000036 0x000000000f
    Entry #119: 0x0000000049 0x0000000075 0x000000002b
    Entry #120: 0x000000003a 0x0000000056 0x0000000023
    Entry #121: 0x000000004a 0x0000000067 0x0000000034
    Entry #122: 0x0000000052 0x000000006c 0x000000003d
    Entry #123: 0x000000001a 0x0000000027 0x000000000f
    Entry #124: 0x0000000042 0x000000005c 0x000000002c
    Entry #125: 0x000000003a 0x000000005c 0x000000001a
    Entry #126: 0x0000000005 0x0000000006 0x0000000004
    Entry #127: 0x0000000038 0x000000004a 0x0000000025
    Entry #128: 0x0000000049 0x000000005c 0x0000000035
    Entry #129: 0x0000000051 0x0000000063 0x000000003e
    Entry #130: 0x0000000028 0x0000000038 0x0000000016
    Entry #131: 0x000000004a 0x0000000065 0x000000002b
    Entry #132: 0x000000005a 0x0000000075 0x000000003b
    Entry #133: 0x0000000041 0x0000000053 0x000000002c
    Entry #134: 0x0000000077 0x000000007d 0x0000000070
    Entry #135: 0x0000000030 0x0000000046 0x0000000013
    Entry #136: 0x0000000044 0x000000005e 0x0000000023
    Entry #137: 0x0000000054 0x000000006d 0x0000000034
    Entry #138: 0x00000000a8 0x00000000b0 0x000000009e
    Entry #139: 0x0000000085 0x000000008a 0x000000007d
    Entry #140: 0x00000000da 0x00000000e4 0x00000000ca
    Entry #141: 0x0000000037 0x0000000048 0x000000001b
    Entry #142: 0x000000005b 0x0000000075 0x000000002b
    Entry #143: 0x0000000067 0x000000007c 0x0000000041
    Entry #144: 0x0000000070 0x0000000086 0x0000000048
    Entry #145: 0x0000000044 0x0000000054 0x0000000024
    Entry #146: 0x000000004c 0x000000005c 0x000000002c
    Entry #147: 0x0000000054 0x0000000064 0x0000000034
    Entry #148: 0x000000005e 0x000000006c 0x000000003d
    Entry #149: 0x000000004c 0x000000005c 0x0000000023
    Entry #150: 0x0000000055 0x0000000066 0x000000002b
    Entry #151: 0x000000005e 0x000000006c 0x0000000033
    Entry #152: 0x00000000d2 0x00000000de 0x00000000ad
    Entry #153: 0x0000000066 0x0000000074 0x000000003a
    Entry #154: 0x00000000c9 0x00000000ce 0x00000000b8
    Entry #155: 0x000000002f 0x0000000038 0x000000000c
    Entry #156: 0x0000000016 0x0000000019 0x000000000b
    Entry #157: 0x000000008b 0x0000000098 0x0000000056
    Entry #158: 0x00000000ee 0x00000000f6 0x00000000cf
    Entry #159: 0x000000004c 0x0000000053 0x000000002d
    Entry #160: 0x00000000b6 0x00000000ba 0x00000000a4
    Entry #161: 0x0000000045 0x000000004e 0x0000000019
    Entry #162: 0x0000000026 0x0000000029 0x0000000016
    Entry #163: 0x0000000056 0x000000005f 0x000000001b
    Entry #164: 0x0000000025 0x0000000029 0x000000000c
    Entry #165: 0x000000004d 0x0000000054 0x0000000023
    Entry #166: 0x0000000081 0x000000008a 0x0000000049
    Entry #167: 0x00000000e0 0x00000000e6 0x00000000bb
    Entry #168: 0x0000000036 0x000000003a 0x0000000019
    Entry #169: 0x0000000056 0x000000005b 0x000000002a
    Entry #170: 0x000000005e 0x0000000064 0x0000000033
    Entry #171: 0x00000000c2 0x00000000c5 0x00000000a9
    Entry #172: 0x0000000046 0x0000000049 0x0000000023
    Entry #173: 0x0000000076 0x000000007c 0x0000000040
    Entry #174: 0x00000000d6 0x00000000d8 0x00000000c0
    Entry #175: 0x0000000057 0x0000000059 0x0000000037
    Entry #176: 0x000000005e 0x0000000060 0x000000003f
    Entry #177: 0x0000000097 0x0000000098 0x0000000086
    Entry #178: 0x0000000067 0x000000006b 0x0000000021
    Entry #179: 0x0000000068 0x0000000068 0x0000000058
    Entry #180: 0x000000005a 0x000000005a 0x0000000052
    Entry #181: 0x0000000019 0x0000000019 0x0000000018
    Entry #182: 0x00000000fd 0x00000000fd 0x00000000f5
    Entry #183: 0x0000000056 0x0000000050 0x000000001e
    Entry #184: 0x0000000071 0x000000006b 0x0000000033
    Entry #185: 0x00000000df 0x00000000db 0x00000000ba
    Entry #186: 0x0000000079 0x0000000076 0x000000005f
    Entry #187: 0x00000000d2 0x00000000cd 0x00000000a9
    Entry #188: 0x00000000d9 0x00000000d4 0x00000000b1
    Entry #189: 0x000000006a 0x0000000065 0x000000004a
    Entry #190: 0x00000000e6 0x00000000e1 0x00000000c5
    Entry #191: 0x00000000c0 0x00000000b9 0x0000000099
    Entry #192: 0x00000000cd 0x00000000c4 0x000000009e
    Entry #193: 0x00000000a9 0x00000000a3 0x0000000088
    Entry #194: 0x00000000b3 0x00000000ad 0x0000000093
    Entry #195: 0x0000000067 0x0000000059 0x0000000027
    Entry #196: 0x0000000089 0x0000000081 0x0000000068
    Entry #197: 0x00000000c1 0x00000000b2 0x000000008a
    Entry #198: 0x0000000095 0x000000008c 0x0000000071
    Entry #199: 0x00000000a2 0x0000000097 0x0000000079
    Entry #200: 0x0000000082 0x000000006e 0x0000000042
    Entry #201: 0x00000000b3 0x00000000a3 0x000000007e
    Entry #202: 0x000000008b 0x0000000075 0x000000004c
    Entry #203: 0x00000000c0 0x00000000a6 0x0000000077
    Entry #204: 0x00000000a9 0x0000000094 0x000000006d
    Entry #205: 0x0000000048 0x0000000042 0x0000000037
    Entry #206: 0x000000008a 0x0000000076 0x0000000057
    Entry #207: 0x000000009a 0x0000000085 0x0000000062
    Entry #208: 0x000000008d 0x000000006d 0x0000000043
    Entry #209: 0x0000000059 0x0000000045 0x000000002b
    Entry #210: 0x00000000b8 0x0000000096 0x0000000066
    Entry #211: 0x0000000085 0x000000006c 0x000000004c
    Entry #212: 0x000000007a 0x0000000065 0x0000000049
    Entry #213: 0x000000006a 0x0000000055 0x000000003b
    Entry #214: 0x00000000a5 0x0000000084 0x000000005c
    Entry #215: 0x0000000079 0x0000000069 0x0000000056
    Entry #216: 0x000000009a 0x0000000075 0x000000004c
    Entry #217: 0x00000000af 0x0000000089 0x000000005c
    Entry #218: 0x0000000085 0x0000000064 0x0000000043
    Entry #219: 0x000000008d 0x000000006c 0x000000004c
    Entry #220: 0x0000000099 0x0000000078 0x0000000056
    Entry #221: 0x000000006a 0x0000000057 0x0000000045
    Entry #222: 0x00000000a8 0x0000000078 0x0000000051
    Entry #223: 0x0000000089 0x000000006c 0x0000000055
    Entry #224: 0x0000000059 0x0000000047 0x0000000038
    Entry #225: 0x0000000039 0x0000000032 0x000000002c
    Entry #226: 0x0000000078 0x0000000056 0x000000003b
    Entry #227: 0x0000000098 0x000000006c 0x000000004b
    Entry #228: 0x000000005b 0x000000004e 0x0000000044
    Entry #229: 0x0000000068 0x0000000044 0x000000002b
    Entry #230: 0x00000000a1 0x000000006a 0x0000000046
    Entry #231: 0x0000000091 0x0000000063 0x0000000043
    Entry #232: 0x0000000078 0x0000000059 0x0000000044
    Entry #233: 0x0000000085 0x0000000064 0x000000004d
    Entry #234: 0x0000000077 0x0000000049 0x000000002c
    Entry #235: 0x0000000056 0x0000000036 0x0000000022
    Entry #236: 0x0000000087 0x0000000058 0x000000003b
    Entry #237: 0x000000008f 0x0000000064 0x000000004c
    Entry #238: 0x0000000068 0x0000000049 0x0000000038
    Entry #239: 0x0000000097 0x000000006c 0x0000000054
    Entry #240: 0x0000000078 0x000000005c 0x000000004c
    Entry #241: 0x0000000042 0x0000000028 0x000000001b
    Entry #242: 0x0000000087 0x000000005b 0x0000000045
    Entry #243: 0x000000004f 0x000000003a 0x000000002f
    Entry #244: 0x0000000035 0x0000000021 0x0000000018
    Entry #245: 0x0000000076 0x000000004b 0x0000000038
    Entry #246: 0x0000000066 0x000000005b 0x0000000056
    Entry #247: 0x000000005e 0x000000003c 0x000000002f
    Entry #248: 0x0000000046 0x0000000031 0x0000000029
    Entry #249: 0x0000000038 0x0000000029 0x0000000024
    Entry #250: 0x0000000027 0x000000001a 0x0000000017
    Entry #251: 0x000000006e 0x000000004b 0x0000000044
    Entry #252: 0x0000000016 0x000000000b 0x0000000009
    Entry #253: 0x00000000fd 0x00000000f5 0x00000000f5
    Entry #254: 0x00000000fd 0x00000000fd 0x00000000fd
    Entry #255: 0x00000000f5 0x00000000f5 0x00000000f5
     */
    let pclr = header_box.palette_box.as_ref().unwrap();
    assert_eq!(pclr.num_components(), 3);
    assert_eq!(pclr.num_entries(), 256);
    assert_eq!(pclr.generated_components().len(), 3);
    assert_eq!(
        pclr.generated_components()[0].bit_depth(),
        BitDepth::Unsigned { value: 8 }
    );
    assert_eq!(pclr.generated_components()[0].values().len(), 256);
    assert_eq!(pclr.generated_components()[0].values()[0], 0);
    // TODO: fix these
    // assert_eq!(pclr.generated_components()[0].values()[1], 0xff);
    // assert_eq!(pclr.generated_components()[0].values()[2], 0x17);
    // assert_eq!(pclr.generated_components()[0].values()[252], 0x16);
    assert_eq!(
        pclr.generated_components()[1].bit_depth(),
        BitDepth::Unsigned { value: 8 }
    );
    assert_eq!(pclr.generated_components()[1].values().len(), 256);
    // assert_eq!(pclr.generated_components()[1].values()[0], 0);
    // assert_eq!(pclr.generated_components()[1].values()[1], 0xff);
    // assert_eq!(pclr.generated_components()[1].values()[2], 0x0c);
    // assert_eq!(pclr.generated_components()[1].values()[252], 0x0b);
    assert_eq!(
        pclr.generated_components()[2].bit_depth(),
        BitDepth::Unsigned { value: 8 }
    );
    assert_eq!(pclr.generated_components()[2].values().len(), 256);
    // assert_eq!(pclr.generated_components()[2].values()[0], 0);
    // assert_eq!(pclr.generated_components()[2].values()[1], 0xff);
    // assert_eq!(pclr.generated_components()[3].values()[2], 0x15);
    // assert_eq!(pclr.generated_components()[2].values()[252], 0x09);
    // assert_eq!(pclr.generated_components()[2].values()[255], 0xf5);

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
            colourspace: 18,
            colour_specification_method: ColourSpecificationMethods::EnumeratedColourSpace,
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
            colourspace: 16,
            colour_specification_method: ColourSpecificationMethods::EnumeratedColourSpace,
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
fn test_sample_zoo1() {
    let boxes = test_sample_jp2_file(
        "zoo1.jp2",
        ExpectedConfiguration {
            compatibility_list: vec!["jp2 ".into()],
            width: 3906,
            height: 2602,
            num_components: 3,
            bit_depth: 8,
            colourspace: 18,
            colour_specification_method: ColourSpecificationMethods::EnumeratedColourSpace,
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
            colourspace: 16,
            colour_specification_method: ColourSpecificationMethods::EnumeratedColourSpace,
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

    assert!(boxes.signature_box().is_some());
    let signature = boxes.signature_box().as_ref().unwrap();
    assert_eq!(signature.signature(), *b"\x0d\x0a\x87\x0a");

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
        colour_specification_box.method(),
        expected.colour_specification_method,
    );
    assert_eq!(colour_specification_box.precedence(), 0);
    if expected.has_unexpected_approx_set {
        assert_eq!(colour_specification_box.colourspace_approximation(), 1u8);
    } else {
        assert_eq!(colour_specification_box.colourspace_approximation(), 0u8);
    }
    assert!(colour_specification_box.enumerated_colour_space().is_some());
    assert_eq!(
        colour_specification_box.enumerated_colour_space().unwrap(),
        expected.colourspace
    );

    assert!(header_box.resolution_box.is_none());

    assert_eq!(boxes.contiguous_codestreams_boxes().len(), 1);
    let codestream_box = boxes.contiguous_codestreams_boxes().first().unwrap();
    assert!(codestream_box.length() > 0);
    assert!(codestream_box.offset() > 0);

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
            colourspace: 17,
            colour_specification_method: ColourSpecificationMethods::EnumeratedColourSpace,
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
        colour_specification_box.method(),
        expected.colour_specification_method,
    );
    assert_eq!(colour_specification_box.precedence(), 0);
    assert_eq!(colour_specification_box.colourspace_approximation(), 0u8);
    assert!(colour_specification_box.enumerated_colour_space().is_some());
    assert_eq!(
        colour_specification_box.enumerated_colour_space().unwrap(),
        expected.colourspace
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
    let j2ki = boxes.intellectual_property_box().as_ref().unwrap();
    assert_eq!(j2ki.length(), 469);
    assert_eq!(j2ki.format(), "<?xml version=\"1.0\"?>\n<!-- markings are for test purposes only, content is public release -->\n<jp:IPR xmlns:jp=\"http://www.jpeg.org/jpx/1.0/xml\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\n<jp:IPR_EXPLOITATION>\n<jp:IPR_USE_RESTRICTION>unclassified</jp:IPR_USE_RESTRICTION>\n<jp:IPR_MGMT_SYS>\n<jp:IPR_MGMT_TYPE>SWE</jp:IPR_MGMT_TYPE>\n</jp:IPR_MGMT_SYS>\n<jp:IPR_PROTECTION>SWE;FRA;USA;GBR;ARE;ZAF;DEU;ITA;CZE</jp:IPR_PROTECTION>\n</jp:IPR_EXPLOITATION>\n</jp:IPR>");

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
    assert!(res.capture_resolution_box().is_some());
    let resc = res.capture_resolution_box().as_ref().unwrap();
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
