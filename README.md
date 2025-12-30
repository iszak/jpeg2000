# JPEG2000
This project aims to primarily implement decoding of the JPEG 2000 ISO 15444 
Part 1 Core coding system.

## Progress

### JP2 container
Decoding of ISO 15444 Part-1 JP2 file format, Annex I, is mostly complete, 
unless there are bugs. Encoding is not started. Improvements in performance and 
robustness of conformance checks can be made.

#### Decoding
- Signature box I.5.1 (100%)
- File type box I.5.2 (99%)
- JP2 header box I.5.3. (99%)
  - Image Header box I.5.3.1 (99%)
  - Bits Per Component box I.5.3.2 (100%)
  - Colour Specification box I.5.3.3 (100%)
  - Palette box I5.3.4. (100%)
  - Component Mapping box I.5.3.5 (100%)
  - Channel Definition box I.5.3.6 (99%)
  - Resolution box I5.3.7 (100%)
    - Capture Resolution box I.5.3.7.1 (100%)
    - Default Display Resolution box I.5.3.7.2 (100%)
  - Contiguous Codestream box I.5.4 (100%)
  - Intellectual Property box I.6 (100%)
  - XML box I.7.1 (100%)
  - UUID box I7.2 (100%)
  - UUID Info box I7.3 (100%)
    - UUID List box I.7.3.1 (100%)
    - URL box I.7.3.2 (100%)

### Codestream
Decoding of ISO 15444 Part-1 Codestream, Annex A, is in progress. Encoding is
not started.

#### Decoding

- Start of codestream A.4.1 SOC (100%)
- Start of tile A.4.2 SOC (50%)
- Start of data A.4.3 SOD (100%)
- End of codestream A.4.4 EOC (100%)
- Image and tile size SIZ A.5.1 (90%)
- Coding style default COD A.6.1 (90%)
- Coding style component COC A.6.2 (90%)
- Region of interest RGN A.6.3 (90%)
- Quantization default QCD A.6.4 (90%)
- Quantization component QCC A.6.5 (90%)
- Progression order change POC A.6.6 (90%)
- Tile-part lengths TLM A.7.1 (90%)
- Packet length, main header PLM A.7.2 (80%)
- Packet length, tile-part header PLT A.7.3 (90%)
- Packed packet headers, main header PPM A.7.4 (10%)
- Packed packet headers, tile-part header PPM A.7.5 (15%)
- Start of packet SOP A.8.1 (0%)
- End of packet header EPH A.8.2 (100%)
- Component registration CRG A.9.1 (90%)
- Comment COM A.9.2 (90%)


### JPXML
Encoding of JP2 and JPC into ISO 16444 Part-14 XML representation. This is 
mostly used for debugging purposes.

#### Encoding


### ICC
ICC support is needed as an embedded colourspace which contains a restricted
subset of ICC Input and Display profiles can be used. Current support is 
minimal to allow further decoding of the JP2 file format. See ISO 15444-1 
I.3.2 and ISO 15075-1.

### Arithmetic entropy coding
Started but redumentary implementation, see Annex C

### Quantization
Not started, see Annex E

### Discrete wavelet transformation of tile-components
Not started, see Annex F

### DC level shifting and multiple component transformations
Not started, see Annex G


## TODO
- add tests
- add benchmarks
- add fuzzing


## Quick Start (for Contributors)

### Clone, Build, Test

```bash
git clone https://github.com/iszak/jpeg2000.git
cd jpeg2000
cargo build
cargo test --workspace
```

### Code Quality

Use rustfmt and clippy.
```bash
cargo fmt
cargo clippy -- -D warnings
```

### Documentation

Documentation could be improved.

- Use `cargo doc --open` to preview documentation locally

### Testing

Reference decoding can be obtained by using other jpeg 2000 parsing tools, such as opj_dump from [openjpeg](https://www.openjpeg.org/) and [jpylyzer](https://jpylyzer.openpreservation.org/).

```
opj_dump -v -i samples/file1.jp2
jpylyzer --verbose samples/file1.jp2
```


### Running Compliance Tests

The project includes an optional compliance test suite that uses external reference data (~130MB).

To run compliance tests:
```bash
cargo test --features compliance-tests
```

The test data will be automatically downloaded and cached on first run. The download requires `curl` and `unzip` to be installed.

**Note:** The cached data persists through `cargo clean`. To force a re-download, delete the `compliance-data-cache/` directory.

