//! Compliance test cases

#![cfg(feature = "compliance-tests")]
const DATA_FOLDER: &str = "openjpeg-data-39524bd3a601d90ed8e0177559400d23945f96a9";

use std::{io::Cursor, path::PathBuf};

use jpc::decode_jpc;

fn get_compliance_data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("compliance-data-cache")
        .join(DATA_FOLDER)
}

#[test]
fn test_data_load() {
    let data_path = get_compliance_data_dir().join("input/conformance/p0_03.j2k");
    let content = std::fs::read(data_path).expect("Failed to read test data");

    assert!(!content.is_empty());
    // grab the first four bytes, should be SOC and SIZ markers
    assert_eq!(content[..4], vec![0xFF, 0x4F, 0xFF, 0x51]);
}

/// Test p0 compliance tests. Only verify pass/fail to parse.
///
/// Treat as a ratchet and try to improve results.
#[test]
fn test_parse_p0_j2k_files() {
    let files = [
        // (Expect parse?, file_name)
        (true, "./input/conformance/p0_01.j2k"),
        (false, "./input/conformance/p0_02.j2k"),
        (false, "./input/conformance/p0_03.j2k"),
        (true, "./input/conformance/p0_04.j2k"),
        (false, "./input/conformance/p0_05.j2k"),
        (true, "./input/conformance/p0_06.j2k"),
        (false, "./input/conformance/p0_07.j2k"),
        (true, "./input/conformance/p0_08.j2k"),
        (true, "./input/conformance/p0_09.j2k"),
        (false, "./input/conformance/p0_10.j2k"),
        (true, "./input/conformance/p0_11.j2k"),
        (true, "./input/conformance/p0_12.j2k"),
        (false, "./input/conformance/p0_13.j2k"),
        (true, "./input/conformance/p0_14.j2k"),
        (false, "./input/conformance/p0_15.j2k"),
        (true, "./input/conformance/p0_16.j2k"),
    ];

    for (pass, file) in files {
        let p = get_compliance_data_dir().join(file);
        println!(
            "Trying to parse: {:?} expecting {}",
            p.file_name(),
            if pass { "pass" } else { "fail" }
        );
        let content = std::fs::read(&p).expect("Failed to read test data");
        assert!(!content.is_empty());

        let parse = decode_jpc(&mut Cursor::new(content));
        assert_eq!(
            parse.is_ok(),
            pass,
            "Unexpected result, update test for {}",
            p.file_name().unwrap().display()
        );
    }
}

/// Test p1 compliance tests. Only verify pass/fail to parse.
///
/// Treat as a ratchet and try to improve results.
#[test]
fn test_parse_p1_j2k_files() {
    let files = [
        // (Expect parse?, file_name)
        (true, "./input/conformance/p1_01.j2k"),
        (true, "./input/conformance/p1_02.j2k"),
        (false, "./input/conformance/p1_03.j2k"),
        (false, "./input/conformance/p1_04.j2k"),
        (false, "./input/conformance/p1_05.j2k"),
        (false, "./input/conformance/p1_06.j2k"),
        (true, "./input/conformance/p1_07.j2k"),
    ];

    for (pass, file) in files {
        let p = get_compliance_data_dir().join(file);
        println!(
            "Trying to parse: {:?} expecting {}",
            p.file_name(),
            if pass { "pass" } else { "fail" }
        );
        let content = std::fs::read(&p).expect("Failed to read test data");
        assert!(!content.is_empty());

        let parse = decode_jpc(&mut Cursor::new(content));
        assert_eq!(
            parse.is_ok(),
            pass,
            "Unexpected result, update test for {}",
            p.file_name().unwrap().display()
        );
    }
}
