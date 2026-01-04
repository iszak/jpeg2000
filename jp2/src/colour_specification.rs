use log::warn;
use std::error;
use std::fmt;
use std::io;

use crate::JBox;

type Method = u8;

const METHOD_ENUMERATED_COLOUR_SPACE: Method = 1;
const METHOD_ENUMERATED_RESTRICTED_ICC_PROFILE: Method = 2;
const METHOD_ENUMERATED_ANY_ICC_PROFILE: Method = 3;
const METHOD_ENUMERATED_VENDOR_METHOD: Method = 4;
const METHOD_ENUMERATED_PARAMETERIZED_COLOUR_SPACE: Method = 5;

#[derive(Debug, PartialEq)]
/// Colour specification methods (METH).
///
/// In ITU-T T.800 | ISO/IEC 15444-1, there are two supported colour specification
/// methods.
///
/// In ITU-T T.801 | ISO/IEC 15444-2, there area five supported colour specification
/// methods.
pub enum ColourSpecificationMethods {
    /// Enumerated colour space, using integer codes.
    ///
    /// This format is the same in both ITU-T T.800 | ISO/IEC 15444-1 and ITU-T T.801 | ISO/IEC 15444-2.
    /// However the JPX file format (ITU-T T.801 | ISO/IEC 15444-2) defines additional enumerated
    /// values and additional parameters for some enumerated colourspaces.
    EnumeratedColourSpace { code: EnumeratedColourSpaces },

    /// Restricted ICC method.
    ///
    /// The Colour Specification box contains an ICC profile in the PROFILE field. This profile shall
    /// specify the transformation needed to convert the decompressed image data into the PCS<sub>XYZ</sub>,
    /// and shall conform to either the Monochrome Input, the Three-Component Matrix-Based Input profile
    /// class, the Monochrome Display or the Three-Component Matrix-Based Display class and contain all
    /// the required tags specified therein, as defined in ISO 15076-1. As such, the value of the Profile
    /// Connection Space field in the profile header in the embedded profile shall be 'XYZ\040'
    /// (0x5859 5A20) indicating that the output colourspace of the profile is in the XYZ colourspace
    ///
    /// Any private tags in the ICC profile shall not change the visual appearance of an image processed
    /// using this ICC profile.
    ///
    /// The components from the codestream may have a range greater than the input range of the tone
    /// reproduction curve (TRC) of the ICC profile. Any decoded values should be clipped to the limits of
    /// the TRC before processing the image through the ICC profile. For example, negative sample values
    /// of signed components may be clipped to zero before processing the image data through the profile.
    ///
    /// See ITU-T T.800(V4) | ISO/IEC 15444-1:2024 J.8 for a more detailed description of the legal
    /// colourspace transforms, for how these transforms are stored in the file, and how to process an image
    /// using that transform without using an ICC colour management engine.
    ///
    /// If the value of METH is 2, then the PROFILE field shall immediately follow the APPROX field and the
    /// PROFILE field shall be the last field in the box.
    ///
    /// The definition of and format of this method is the same in both ITU-T T.800 | ISO/IEC 15444-1
    /// and ITU-T T.801 | ISO/IEC 15444-2.
    RestrictedICCProfile { profile_data: Vec<u8> },

    /// Any ICC method.
    ///
    /// This Colour Specification box indicates that the colourspace of the codestream is specified by an
    /// embedded input ICC profile. Contrary to the Restricted ICC method defined in the JP2 file format
    /// (ITU-T T.800 | ISO/IEC 15444-1), this method allows for any input ICC profile defined by ISO/IEC
    /// 15076-1.
    ///
    /// This method is from ITU-T T.801 | ISO/IEC 15444-2. It is also permitted in ITU-T T.814 | ISO/IEC 15444-15
    /// (High Throughput JPEG 2000) files. It is not permitted in ITU-T T.800 | ISO/IEC 15444-1 files.
    AnyICCProfile { profile_data: Vec<u8> },

    /// Vendor Colour method.
    ///
    /// The Colour Specification box indicates that the colourspace of the codestream is specified by a
    /// unique vendor defined code. The binary format of the METHDAT field is specified in
    /// ITU-T T.801(V4) | ISO/IEC 15444-2:2024 clause M.11.7.3.3.
    ///
    /// This method is from ITU-T T.801 | ISO/IEC 15444-2. It is not permitted in ITU-T T.800 | ISO/IEC 15444-1
    /// or ITU-T T.814 | ISO/IEC 15444-15 (High Throughput JPEG 2000) files.
    VendorColourMethod {
        vendor_defined_code: [u8; 16],
        vendor_parameters: Vec<u8>,
    },

    /// Parameterized colourspace
    ///
    /// The Colour Specification box indicates that the colourspace of the codestream is parameterized as
    /// specified in Rec. ITU-T H.273 | ISO/IEC 23091-2. The binary format of the METHDAT field is specified in
    /// ITU-T T.801(V4) | ISO/IEC 15444-2:2024 clause M.11.7.3.4.
    ///
    /// This method is from ITU-T T.801 | ISO/IEC 15444-2. It is also permitted in ITU-T T.814 | ISO/IEC 15444-15
    /// (High Throughput JPEG 2000) files. It is not permitted in ITU-T T.800 | ISO/IEC 15444-1 files.
    ParameterizedColourspace {
        colour_primaries: u16,
        transfer_characteristics: u16,
        matrix_coefficients: u16,
        video_full_range: bool,
    },

    /// Other value, reserved for use by ITU | ISO/IEC.
    ///
    /// For any value of the METH field, the length of the METHDAT field may not be 0, and applications shall
    /// not expect that the APPROX field be the last field in the box if the value of the METH field is not
    /// understood.
    ///
    /// In this case, a conforming reader shall ignore the entire Colour Specification box.
    Reserved { value: u8 },
}

impl ColourSpecificationMethods {
    pub fn encoded_meth(&self) -> [u8; 1] {
        match self {
            ColourSpecificationMethods::EnumeratedColourSpace { .. } => {
                [METHOD_ENUMERATED_COLOUR_SPACE]
            }
            ColourSpecificationMethods::RestrictedICCProfile { .. } => {
                [METHOD_ENUMERATED_RESTRICTED_ICC_PROFILE]
            }
            ColourSpecificationMethods::AnyICCProfile { .. } => [METHOD_ENUMERATED_ANY_ICC_PROFILE],
            ColourSpecificationMethods::VendorColourMethod { .. } => {
                [METHOD_ENUMERATED_VENDOR_METHOD]
            }
            ColourSpecificationMethods::ParameterizedColourspace { .. } => {
                [METHOD_ENUMERATED_PARAMETERIZED_COLOUR_SPACE]
            }
            ColourSpecificationMethods::Reserved { value } => [*value],
        }
    }

    pub fn encoded_methdat(&self) -> Vec<u8> {
        match self {
            ColourSpecificationMethods::EnumeratedColourSpace { code } => code.encoded_methdat(),
            ColourSpecificationMethods::RestrictedICCProfile { profile_data } => {
                profile_data.clone()
            }
            ColourSpecificationMethods::AnyICCProfile { profile_data } => profile_data.clone(),
            ColourSpecificationMethods::VendorColourMethod {
                vendor_defined_code,
                vendor_parameters,
            } => {
                let mut methdat = Vec::<u8>::with_capacity(16 + vendor_parameters.len());
                methdat.extend_from_slice(vendor_defined_code);
                methdat.extend_from_slice(vendor_parameters);
                methdat
            }
            ColourSpecificationMethods::ParameterizedColourspace {
                colour_primaries,
                transfer_characteristics,
                matrix_coefficients,
                video_full_range,
            } => {
                let mut methdat = Vec::<u8>::with_capacity(7); // 3 x u16, plus the flag byte
                methdat.extend_from_slice(&colour_primaries.to_be_bytes());
                methdat.extend_from_slice(&transfer_characteristics.to_be_bytes());
                methdat.extend_from_slice(&matrix_coefficients.to_be_bytes());
                let flags: u8 = if *video_full_range { 0x80 } else { 0x00 };
                methdat.push(flags);
                methdat
            }
            ColourSpecificationMethods::Reserved { value } => {
                vec![*value]
            }
        }
    }
}
impl Default for ColourSpecificationMethods {
    fn default() -> Self {
        ColourSpecificationMethods::Reserved { value: 0 }
    }
}
impl fmt::Display for ColourSpecificationMethods {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ColourSpecificationMethods::EnumeratedColourSpace { code } => {
                write!(f, "Enumerated colourspace: {code}")
            }
            ColourSpecificationMethods::RestrictedICCProfile { profile_data: _ } => {
                // TODO: could provide more info on the profile.
                write!(f, "Restricted ICC Profile")
            }
            ColourSpecificationMethods::AnyICCProfile { profile_data: _ } => {
                // TODO: could provide more info on the profile.
                write!(f, "\"Any\" ICC Profile")
            }
            ColourSpecificationMethods::VendorColourMethod {
                vendor_defined_code: _,
                vendor_parameters: _,
            } => {
                // TODO: could include the UUID.
                write!(f, "Vendor Colour")
            }
            ColourSpecificationMethods::ParameterizedColourspace {
                colour_primaries,
                transfer_characteristics,
                matrix_coefficients,
                video_full_range,
            } => {
                write!(f, "Parameterized colourspace, colour primaries: {colour_primaries}, transfer characteristics: {transfer_characteristics}, matrix coefficients: {matrix_coefficients}, video full range: {video_full_range}")
            }
            ColourSpecificationMethods::Reserved { value } => write!(f, "{}", value),
        }
    }
}

type EnumeratedColourSpace = [u8; 4];

const ENUMERATED_COLOUR_SPACE_BILEVEL: EnumeratedColourSpace = [0, 0, 0, 0];
const ENUMERATED_COLOUR_SPACE_YCBCR1: EnumeratedColourSpace = [0, 0, 0, 1];
// No entry for 2
const ENUMERATED_COLOUR_SPACE_YCBCR2: EnumeratedColourSpace = [0, 0, 0, 3];
const ENUMERATED_COLOUR_SPACE_YCBCR3: EnumeratedColourSpace = [0, 0, 0, 4];
// No entries for 5 to 8
const ENUMERATED_COLOUR_SPACE_PHOTO_YCC: EnumeratedColourSpace = [0, 0, 0, 9];
// No entry for 10
const ENUMERATED_COLOUR_SPACE_CMY: EnumeratedColourSpace = [0, 0, 0, 11];
const ENUMERATED_COLOUR_SPACE_CMYK: EnumeratedColourSpace = [0, 0, 0, 12];
const ENUMERATED_COLOUR_SPACE_YCCK: EnumeratedColourSpace = [0, 0, 0, 13];
const ENUMERATED_COLOUR_SPACE_CIELAB: EnumeratedColourSpace = [0, 0, 0, 14];
const ENUMERATED_COLOUR_SPACE_BILEVEL2: EnumeratedColourSpace = [0, 0, 0, 15];
const ENUMERATED_COLOUR_SPACE_SRGB: EnumeratedColourSpace = [0, 0, 0, 16];
const ENUMERATED_COLOUR_SPACE_GREYSCALE: EnumeratedColourSpace = [0, 0, 0, 17];
const ENUMERATED_COLOUR_SPACE_SYCC: EnumeratedColourSpace = [0, 0, 0, 18];
const ENUMERATED_COLOUR_SPACE_CIEJAB: EnumeratedColourSpace = [0, 0, 0, 19];
const ENUMERATED_COLOUR_SPACE_ESRGB: EnumeratedColourSpace = [0, 0, 0, 20];
const ENUMERATED_COLOUR_SPACE_ROMM_RGB: EnumeratedColourSpace = [0, 0, 0, 21];
const ENUMERATED_COLOUR_SPACE_YPBPR_1125_60: EnumeratedColourSpace = [0, 0, 0, 22];
const ENUMERATED_COLOUR_SPACE_YPBPR_1250_50: EnumeratedColourSpace = [0, 0, 0, 23];
const ENUMERATED_COLOUR_SPACE_ESYCC: EnumeratedColourSpace = [0, 0, 0, 24];
const ENUMERATED_COLOUR_SPACE_SCRGB: EnumeratedColourSpace = [0, 0, 0, 25];
const ENUMERATED_COLOUR_SPACE_SCRGB_GRAYSCALE: EnumeratedColourSpace = [0, 0, 0, 26];

#[derive(Clone, Copy, Debug, PartialEq)]
/// Enumerated colour space values (EnumCS)
///
/// See ITU-T T.800(V4) | ISO/IEC 15444-1:2024 Table I.10 for values allowed in core
/// coding system (JP2) files.
///
/// See ITU-T T.801(V3) | ISO/IEC 15444-2:2023 Table M.25 for values that may
/// occur in extended (JPX) files.
pub enum EnumeratedColourSpaces {
    /// Bi-level.
    ///
    /// This value shall be used to indicate bi-level images. Each image sample is
    /// one bit: 0 = white, 1 = black.
    ///
    /// This is an extension value from ITU-T T.801 | ISO/IEC 15444-2. This value
    /// is not permitted in ITU-T T.800 | ISO/IEC 15444-1 conformant files.
    BiLevel,

    /// YC<sub>b</sub>C<sub>r</sub>(1).
    ///
    /// This is a format often used for data that originated from a video signal.
    /// The colourspace is based on Rec. ITU-R BT.709-4. The valid ranges of the
    /// YC<sub>b</sub>C<sub>r</sub> components in this space is limited to less
    /// than the full range that could be represented given an 8-bit representation.
    /// Rec. ITU-R BT.601-5 specifies these ranges as well as defines a 3 x 3
    /// matrix transformation that can be used to convert these samples into RGB.
    ///
    /// This is an extension value from ITU-T T.801 | ISO/IEC 15444-2. This value
    /// is not permitted in ITU-T T.800 | ISO/IEC 15444-1 conformant files.
    YCbCr1,

    /// YC<sub>b</sub>C<sub>r</sub>(2).
    ///
    /// This is the most commonly used format for image data that was originally
    /// captured in RGB (uncalibrated format). The colourspace is based on Rec.
    /// ITU-R BT.601-5. The valid ranges of the YC<sub>b</sub>C<sub>r</sub>
    /// components in this space is [0, 255] for Y, and [–128, 127] for
    /// C<sub>b</sub> and C<sub>r</sub> (stored with an offset of 128 to convert
    /// the range to [0, 255]). These ranges are different from the ones defined
    /// in Rec. ITU-R BT.601-5. Rec. ITU-R BT.601-5 specifies a 3 x 3 matrix
    /// transformation that can be used to convert these samples into RGB.
    ///
    /// This is an extension value from ITU-T T.801 | ISO/IEC 15444-2. This value
    /// is not permitted in ITU-T T.800 | ISO/IEC 15444-1 conformant files.
    YCbCr2,

    /// YC<sub>b</sub>C<sub>r</sub>(3).
    ///
    /// This is a format often used for data that originated from a video signal.
    /// The colourspace is based on Rec. ITU-R BT.601-5. The valid ranges of the
    /// YC<sub>b</sub>C<sub>r</sub> components in this space is limited to less
    /// than the full range that could be represented given an 8-bit representation.
    /// Rec. ITU-R BT.601-5 specifies these ranges as well as defines a 3 x 3 matrix
    /// transformation that can be used to convert these samples into RGB.
    ///
    /// This is an extension value from ITU-T T.801 | ISO/IEC 15444-2. This value
    /// is not permitted in ITU-T T.800 | ISO/IEC 15444-1 conformant files.
    YCbCr3,

    /// PhotoYCC.
    ///
    /// This is the colour encoding method used in the Photo CD<sup>TM</sup>
    /// system. The colourspace is based on Rec. ITU-R BT.709 reference primaries.
    /// Rec. ITU-R BT.709 linear RGB image signals are transformed to non-linear R'G'B'
    /// values to YCC corresponding to Rec. ITU-R BT.601-5. Details of this encoding
    /// method can be found in Kodak Photo CD products, A Planning Guide for
    /// Developers, Eastman Kodak Company, Part No. DC1200R and also in Kodak Photo
    /// CD Information Bulletin PCD045.
    ///
    /// This is an extension value from ITU-T T.801 | ISO/IEC 15444-2. This value
    /// is not permitted in ITU-T T.800 | ISO/IEC 15444-1 conformant files.
    PhotoYCC,

    /// CMY.
    ///
    /// The encoded data consists of samples of Cyan, Magenta and Yellow samples,
    /// directly suitable for printing on typical CMY devices. A value of 0 shall
    /// indicate 0% ink coverages, whereas a value of 2<sup>BPS</sup>–1 shall
    /// indicate 100% in coverage for a given component sample.
    ///
    /// This is an extension value from ITU-T T.801 | ISO/IEC 15444-2. This value
    /// is not permitted in ITU-T T.800 | ISO/IEC 15444-1 conformant files.
    CMY,

    /// CMYK.
    ///
    /// As CMY above, except that there is also a black (K) ink component. Ink coverage
    /// is defined as above.
    ///
    /// This is an extension value from ITU-T T.801 | ISO/IEC 15444-2. This value
    /// is not permitted in ITU-T T.800 | ISO/IEC 15444-1 conformant files.
    CMYK,

    /// YCCK.
    ///
    /// This is the result of transforming original CMYK type data by computing
    /// R = (2<sup>BPS</sup>–1)–C, G = (2<sup>BPS</sup>–1)–M, and
    /// B = (2<sup>BPS</sup>–1)–Y, applying the RGB to YCC transformation specified
    /// for YC<sub>b</sub>C<sub>r</sub>(2) above, and then recombining the result
    /// with the unmodified K-sample. This transformation is intended to be the same
    /// as that specified in Adobe Postscript.
    ///
    /// This is an extension value from ITU-T T.801 | ISO/IEC 15444-2. This value
    /// is not permitted in ITU-T T.800 | ISO/IEC 15444-1 conformant files.
    YCCK,

    /// CIELab.
    ///
    /// CIELab: The CIE 1976 (L*a*b*) colourspace. A colourspace defined by the CIE
    /// (Commission Internationale de l'Eclairage), having approximately equal
    /// visually perceptible differences between equally spaced points throughout
    /// the space. The three components are L*, or Lightness, and a* and b* in
    /// chrominance. For this colourspace, additional Enumerated parameters are
    /// specified in the EP field as specified in ITU-T T.801 | ISO/IEC 15444-2
    /// clause M.11.7.4.1.
    CIELab {
        rl: u32,
        ol: u32,
        ra: u32,
        oa: u32,
        rb: u32,
        ob: u32,
        il: u32,
    },

    /// Bi-level(2).
    ///
    /// This value shall be used to indicate bi-level images. Each image sample is
    /// one bit: 1 = white, 0 = black.
    ///
    /// This is an extension value from ITU-T T.801 | ISO/IEC 15444-2. This value
    /// is not permitted in ITU-T T.800 | ISO/IEC 15444-1 conformant files.
    BiLevel2,

    /// sRGB.
    ///
    /// sRGB as defined by IEC 61966-2-1 with Lmin<sub>i</sub>=0 and Lmax<sub>i</sub>=255.
    /// This colourspace shall be used with channels carrying unsigned values only.
    #[allow(non_camel_case_types)]
    sRGB,

    /// Grey scale.
    ///
    /// A greyscale space where image luminance is related to code values using the sRGB non-linearity given
    /// in Equations (2) to (4) of IEC 61966-2-1 (sRGB) specification.
    /// This colourspace shall be used with channels carrying unsigned values only.
    Greyscale,

    /// sYCC.
    ///
    /// sYCC as defined by IEC 61966-2-1 / Amd.1 with Lmin<sub>i</sub>=0 and Lmax<sub>i</sub>=255.
    /// This colourspace shall be used with channels carrying unsigned values only.
    ///
    /// Note: it is not recommended to use the ICT or RCT specified in T.800 | ISO/IEC 15444-1 Annex G
    /// with sYCC image data. See T.800 | ISO/IEC 15444-1 J.14 for guidelines on handling YCC codestreams.
    #[allow(non_camel_case_types)]
    sYCC,

    /// CIEJab.
    ///
    /// As defined by CIE Colour Appearance Model 97s, CIE Publication 131. For this
    /// colourspace, additional Enumerated parameters are specified in the EP field as
    /// specified in ITU-T T.801 | ISO/IEC 15444-2 clause M.11.7.4.2.
    ///
    /// This is an extension value from ITU-T T.801 | ISO/IEC 15444-2. This value
    /// is not permitted in ITU-T T.800 | ISO/IEC 15444-1 conformant files.
    CIEJab {
        rj: u32,
        oj: u32,
        ra: u32,
        oa: u32,
        rb: u32,
        ob: u32,
    },

    /// e-sRGB.
    ///
    /// As defined by PIMA 7667.
    ///
    /// This is an extension value from ITU-T T.801 | ISO/IEC 15444-2. This value
    /// is not permitted in ITU-T T.800 | ISO/IEC 15444-1 conformant files.
    #[allow(non_camel_case_types)]
    esRGB,

    /// ROMM-RGB.
    ///
    /// As defined by ISO 22028-2.
    ///
    /// This is an extension value from ITU-T T.801 | ISO/IEC 15444-2. This value
    /// is not permitted in ITU-T T.800 | ISO/IEC 15444-1 conformant files.
    ROMMRGB,

    /// YPbPr(1125/60).
    ///
    /// This is the well-known colour space and value definition for the HDTV
    /// (1125/60/2:1) system for production and international program exchange
    /// specified by Rec. ITU-R BT.709-3. The Recommendation specifies the colour
    /// space conversion matrix from RGB to YPbPr(1125/60) and the range of values
    /// of each component. The matrix is different from the 1250/50 system. In the
    /// 8-bit/component case, the range of values of each component is [1, 254],
    /// the black level of Y is 16, the achromatic level of Pb/Pr is 128, the nominal
    /// peak of Y is 235, and the nominal extremes of Pb/Pr are 16 and 240. In the
    /// 10-bit case, these values are defined in a similar manner.
    ///
    /// This is an extension value from ITU-T T.801 | ISO/IEC 15444-2. This value
    /// is not permitted in ITU-T T.800 | ISO/IEC 15444-1 conformant files.
    YPbPr112560,

    /// YPbPr(1250/50).
    ///
    /// This is the well-known colour space and value definition for the HDTV
    /// (1250/50/2:1) system for production and international program exchange
    /// specified by Rec. ITU-R BT.709-3. The Recommendation specifies the
    /// colour space conversion matrix from RGB to YPbPr(1250/50) and the range
    /// of values of each component. The matrix is different from the 1125/60
    /// system. In the 8-bit/component case, the range of values of each component
    /// is [1, 254], the black level of Y is 16, the achromatic level of Pb/Pr
    /// is 128, the nominal peak of Y is 235, and the nominal extremes of Pb/Pr
    /// are 16 and 240. In the 10-bit case, these values are defined in a similar
    /// manner.
    ///
    /// This is an extension value from ITU-T T.801 | ISO/IEC 15444-2. This value
    /// is not permitted in ITU-T T.800 | ISO/IEC 15444-1 conformant files.
    YPbPr125050,

    /// e-sYCC.
    ///
    /// e-sRGB based YCC colourspace as defined by PIMA 7667:2001, Annex B.
    ///
    /// This is an extension value from ITU-T T.801 | ISO/IEC 15444-2. This value
    /// is not permitted in ITU-T T.800 | ISO/IEC 15444-1 conformant files.
    #[allow(non_camel_case_types)]
    esYCC,

    /// scRGB.
    ///
    /// scRGB as defined by IEC 61966-2-2.
    ///
    /// This is an extension value from ITU-T T.801 | ISO/IEC 15444-2. This value
    /// is not permitted in ITU-T T.800 | ISO/IEC 15444-1 conformant files.
    #[allow(non_camel_case_types)]
    scRGB,

    /// scRGB gray scale.
    ///
    /// scRGB gray scale, using only a luminance channel but the tone reproduction
    /// curves (non-linearities) defined by IEC 61966-2-2.
    ///
    /// This is an extension value from ITU-T T.801 | ISO/IEC 15444-2. This value
    /// is not permitted in ITU-T T.800 | ISO/IEC 15444-1 conformant files.
    #[allow(non_camel_case_types)]
    scRGBGrayScale,

    /// Value reserved for other ITU-T | ISO/IEC uses.
    Reserved,
}

impl EnumeratedColourSpaces {
    fn decode<R: io::Read + io::Seek>(reader: &mut R) -> Result<Self, Box<dyn error::Error>> {
        let mut enumcs: EnumeratedColourSpace = [0u8; 4];
        reader.read_exact(&mut enumcs)?;
        match enumcs {
            ENUMERATED_COLOUR_SPACE_BILEVEL => Ok(EnumeratedColourSpaces::BiLevel),
            ENUMERATED_COLOUR_SPACE_YCBCR1 => Ok(EnumeratedColourSpaces::YCbCr1),
            ENUMERATED_COLOUR_SPACE_YCBCR2 => Ok(EnumeratedColourSpaces::YCbCr2),
            ENUMERATED_COLOUR_SPACE_YCBCR3 => Ok(EnumeratedColourSpaces::YCbCr3),
            ENUMERATED_COLOUR_SPACE_PHOTO_YCC => Ok(EnumeratedColourSpaces::PhotoYCC),
            ENUMERATED_COLOUR_SPACE_CMY => Ok(EnumeratedColourSpaces::CMY),
            ENUMERATED_COLOUR_SPACE_CMYK => Ok(EnumeratedColourSpaces::CMYK),
            ENUMERATED_COLOUR_SPACE_YCCK => Ok(EnumeratedColourSpaces::YCCK),
            ENUMERATED_COLOUR_SPACE_CIELAB => {
                let mut rl_bytes = [0u8; 4];
                let mut ol_bytes = [0u8; 4];
                let mut ra_bytes = [0u8; 4];
                let mut oa_bytes = [0u8; 4];
                let mut rb_bytes = [0u8; 4];
                let mut ob_bytes = [0u8; 4];
                let mut il_bytes = [0u8; 4];
                reader.read_exact(&mut rl_bytes)?;
                reader.read_exact(&mut ol_bytes)?;
                reader.read_exact(&mut ra_bytes)?;
                reader.read_exact(&mut oa_bytes)?;
                reader.read_exact(&mut rb_bytes)?;
                reader.read_exact(&mut ob_bytes)?;
                reader.read_exact(&mut il_bytes)?;
                Ok(EnumeratedColourSpaces::CIELab {
                    rl: u32::from_be_bytes(rl_bytes),
                    ol: u32::from_be_bytes(ol_bytes),
                    ra: u32::from_be_bytes(ra_bytes),
                    oa: u32::from_be_bytes(oa_bytes),
                    rb: u32::from_be_bytes(rb_bytes),
                    ob: u32::from_be_bytes(ob_bytes),
                    il: u32::from_be_bytes(il_bytes),
                })
            }
            ENUMERATED_COLOUR_SPACE_BILEVEL2 => Ok(EnumeratedColourSpaces::BiLevel2),
            ENUMERATED_COLOUR_SPACE_SRGB => Ok(EnumeratedColourSpaces::sRGB),
            ENUMERATED_COLOUR_SPACE_GREYSCALE => Ok(EnumeratedColourSpaces::Greyscale),
            ENUMERATED_COLOUR_SPACE_SYCC => Ok(EnumeratedColourSpaces::sYCC),
            ENUMERATED_COLOUR_SPACE_CIEJAB => {
                let mut rj_bytes = [0u8; 4];
                let mut oj_bytes = [0u8; 4];
                let mut ra_bytes = [0u8; 4];
                let mut oa_bytes = [0u8; 4];
                let mut rb_bytes = [0u8; 4];
                let mut ob_bytes = [0u8; 4];
                reader.read_exact(&mut rj_bytes)?;
                reader.read_exact(&mut oj_bytes)?;
                reader.read_exact(&mut ra_bytes)?;
                reader.read_exact(&mut oa_bytes)?;
                reader.read_exact(&mut rb_bytes)?;
                reader.read_exact(&mut ob_bytes)?;
                Ok(EnumeratedColourSpaces::CIEJab {
                    rj: u32::from_be_bytes(rj_bytes),
                    oj: u32::from_be_bytes(oj_bytes),
                    ra: u32::from_be_bytes(ra_bytes),
                    oa: u32::from_be_bytes(oa_bytes),
                    rb: u32::from_be_bytes(rb_bytes),
                    ob: u32::from_be_bytes(ob_bytes),
                })
            }
            ENUMERATED_COLOUR_SPACE_ESRGB => Ok(EnumeratedColourSpaces::esRGB),
            ENUMERATED_COLOUR_SPACE_ROMM_RGB => Ok(EnumeratedColourSpaces::ROMMRGB),
            ENUMERATED_COLOUR_SPACE_YPBPR_1125_60 => Ok(EnumeratedColourSpaces::YPbPr112560),
            ENUMERATED_COLOUR_SPACE_YPBPR_1250_50 => Ok(EnumeratedColourSpaces::YPbPr125050),
            ENUMERATED_COLOUR_SPACE_ESYCC => Ok(EnumeratedColourSpaces::esYCC),
            ENUMERATED_COLOUR_SPACE_SCRGB => Ok(EnumeratedColourSpaces::scRGB),
            ENUMERATED_COLOUR_SPACE_SCRGB_GRAYSCALE => Ok(EnumeratedColourSpaces::scRGBGrayScale),
            _ => Ok(EnumeratedColourSpaces::Reserved),
        }
    }

    pub fn encoded_methdat(&self) -> Vec<u8> {
        match self {
            EnumeratedColourSpaces::BiLevel => ENUMERATED_COLOUR_SPACE_BILEVEL.to_vec(),
            EnumeratedColourSpaces::YCbCr1 => ENUMERATED_COLOUR_SPACE_YCBCR1.to_vec(),
            EnumeratedColourSpaces::YCbCr2 => ENUMERATED_COLOUR_SPACE_YCBCR2.to_vec(),
            EnumeratedColourSpaces::YCbCr3 => ENUMERATED_COLOUR_SPACE_YCBCR3.to_vec(),
            EnumeratedColourSpaces::PhotoYCC => ENUMERATED_COLOUR_SPACE_PHOTO_YCC.to_vec(),
            EnumeratedColourSpaces::CMY => ENUMERATED_COLOUR_SPACE_CMY.to_vec(),
            EnumeratedColourSpaces::CMYK => ENUMERATED_COLOUR_SPACE_CMYK.to_vec(),
            EnumeratedColourSpaces::YCCK => ENUMERATED_COLOUR_SPACE_YCCK.to_vec(),
            EnumeratedColourSpaces::CIELab {
                rl,
                ol,
                ra,
                oa,
                rb,
                ob,
                il,
            } => {
                let mut methdat = Vec::<u8>::with_capacity(32); // enum value + 7 x u32
                methdat.extend_from_slice(&ENUMERATED_COLOUR_SPACE_CIELAB);
                methdat.extend_from_slice(&rl.to_be_bytes());
                methdat.extend_from_slice(&ol.to_be_bytes());
                methdat.extend_from_slice(&ra.to_be_bytes());
                methdat.extend_from_slice(&oa.to_be_bytes());
                methdat.extend_from_slice(&rb.to_be_bytes());
                methdat.extend_from_slice(&ob.to_be_bytes());
                methdat.extend_from_slice(&il.to_be_bytes());
                methdat
            }
            EnumeratedColourSpaces::BiLevel2 => ENUMERATED_COLOUR_SPACE_BILEVEL2.to_vec(),
            EnumeratedColourSpaces::sRGB => ENUMERATED_COLOUR_SPACE_SRGB.to_vec(),
            EnumeratedColourSpaces::Greyscale => ENUMERATED_COLOUR_SPACE_GREYSCALE.to_vec(),
            EnumeratedColourSpaces::sYCC => ENUMERATED_COLOUR_SPACE_SYCC.to_vec(),
            EnumeratedColourSpaces::CIEJab {
                rj,
                oj,
                ra,
                oa,
                rb,
                ob,
            } => {
                let mut methdat = Vec::<u8>::with_capacity(32); // enum value + 7 x u32
                methdat.extend_from_slice(&ENUMERATED_COLOUR_SPACE_CIEJAB);
                methdat.extend_from_slice(&rj.to_be_bytes());
                methdat.extend_from_slice(&oj.to_be_bytes());
                methdat.extend_from_slice(&ra.to_be_bytes());
                methdat.extend_from_slice(&oa.to_be_bytes());
                methdat.extend_from_slice(&rb.to_be_bytes());
                methdat.extend_from_slice(&ob.to_be_bytes());
                methdat
            }
            EnumeratedColourSpaces::esRGB => ENUMERATED_COLOUR_SPACE_ESRGB.to_vec(),
            EnumeratedColourSpaces::ROMMRGB => ENUMERATED_COLOUR_SPACE_ROMM_RGB.to_vec(),
            EnumeratedColourSpaces::YPbPr112560 => ENUMERATED_COLOUR_SPACE_YPBPR_1125_60.to_vec(),
            EnumeratedColourSpaces::YPbPr125050 => ENUMERATED_COLOUR_SPACE_YPBPR_1250_50.to_vec(),
            EnumeratedColourSpaces::esYCC => ENUMERATED_COLOUR_SPACE_ESYCC.to_vec(),
            EnumeratedColourSpaces::scRGB => ENUMERATED_COLOUR_SPACE_SCRGB.to_vec(),
            EnumeratedColourSpaces::scRGBGrayScale => {
                ENUMERATED_COLOUR_SPACE_SCRGB_GRAYSCALE.to_vec()
            }
            EnumeratedColourSpaces::Reserved => vec![0xff, 0xff, 0xff, 0xff],
        }
    }
}

impl fmt::Display for EnumeratedColourSpaces {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                EnumeratedColourSpaces::BiLevel => "Bi-level",
                EnumeratedColourSpaces::YCbCr1 => "YCbCr(1)",
                EnumeratedColourSpaces::YCbCr2 => "YCbCr(2)",
                EnumeratedColourSpaces::YCbCr3 => "YCbCr(3)",
                EnumeratedColourSpaces::PhotoYCC => "PhotoYCC",
                EnumeratedColourSpaces::CMY => "CMY",
                EnumeratedColourSpaces::CMYK => "CMYK",
                EnumeratedColourSpaces::YCCK => "YCCK",
                EnumeratedColourSpaces::CIELab { .. } => "CIELab",
                EnumeratedColourSpaces::sRGB => "sRGB",
                EnumeratedColourSpaces::Greyscale => "greyscale",
                EnumeratedColourSpaces::sYCC => "sYCC",
                EnumeratedColourSpaces::BiLevel2 => "Bi-level(2)",
                EnumeratedColourSpaces::CIEJab { .. } => "CIEJab",
                EnumeratedColourSpaces::esRGB => "e-sRGB",
                EnumeratedColourSpaces::ROMMRGB => "ROMM-RGB",
                EnumeratedColourSpaces::YPbPr112560 => "YPbPr(1125/60)",
                EnumeratedColourSpaces::YPbPr125050 => "YPbPr(1250/50)",
                EnumeratedColourSpaces::esYCC => "e-sYCC",
                EnumeratedColourSpaces::scRGB => "scRGB",
                EnumeratedColourSpaces::scRGBGrayScale => "scRGB gray scale",
                EnumeratedColourSpaces::Reserved => "Reserved",
            }
        )
    }
}

pub enum ColourspaceMethod {}

/// Colour Specification box.
///
/// Each Colour Specification box defines one method by which an application can
/// interpret the colourspace of the decompressed image data. This colour
/// specification is to be applied to the image data after it has been
/// decompressed and after any reverse decorrelating component transform has been
/// applied to the image data.
///
/// A JP2 file may contain multiple Colour Specification boxes, but must contain
/// at least one, specifying different methods for achieving “equivalent” results.
/// A conforming JP2 reader shall ignore all Colour Specification boxes after the
/// first. However, readers conforming to other standards may use those boxes as
/// defined in those other standards.
///
/// See ITU-T T.800(V4) | ISO/IEC 15444-1:2024 I.5.3.3 for the core requirements.
/// See ITU-T T.801(V3) | ISO/IEC 15444-2:2023 Section M11.7.2 for the extension requirements.
/// See ITU-T T.814 | ISO/IEC 15444-15:2019 Section D.4 for the High Throughput requirements.
#[derive(Debug, Default)]
pub struct ColourSpecificationBox {
    pub(crate) length: u64,
    pub(crate) offset: u64,
    pub(crate) method: ColourSpecificationMethods,
    pub(crate) precedence: [u8; 1],
    pub(crate) colourspace_approximation: [u8; 1],
}

impl ColourSpecificationBox {
    /// Specification method (METH).
    ///
    /// This field specifies the method used by this Colour Specification box to
    /// define the colourspace of the decompressed image.
    ///
    /// This field is encoded as a 1-byte unsigned integer and represented here
    /// as an enumerated value.
    pub fn method(&self) -> &ColourSpecificationMethods {
        &self.method
    }

    /// Precedence (PREC).
    ///
    /// For ITU-T T.800 | ISO/IEC 15444-1, this field shall be 0; however, conforming
    /// readers shall ignore the value of this field. Only a single
    /// Colour Specification box is supported for this case.
    ///
    /// For ITU-T T.801 | ISO/IEC 15444-2, this field specifies the precedence of
    /// this Colour Specification box, with respect to the other Colour Specification
    /// boxes within the same Colour Group box, or the JP2 Header box if this Colour
    /// Specification box is in the JP2 Header box. It is suggested, but not
    /// required, that conforming readers use the colour specification method that
    /// is supported with the highest precedence.
    ///
    /// This field is specified as a signed 1 byte integer.
    pub fn precedence(&self) -> i8 {
        self.precedence[0] as i8
    }

    /// Colourspace approximation (APPROX).
    ///
    /// This field specifies the extent to which this colour specification method
    /// approximates the “correct” definition of the colourspace.
    ///
    /// For ITU-T T.800 | ISO/IEC 15444-1, the value of this field shall be set to
    /// zero; however, conforming readers shall ignore the value of this field.
    ///
    /// For ITU-T T.801 | ISO/IEC 15444-2, contrary to the APPROX field in a JP2
    /// file (a file with "jp2\040" in the BR field in the File Type box), a value
    /// of 0 in the APPROX field is illegal in a JPX file (a file with "jpx\040"
    /// in the BR field in the File Type box). JPX writers are required to properly
    /// indicate the degree of approximation of the colour specification to the
    /// correct definition of the colourspace. This does not specify if the writer
    /// of the file knew the actual colourspace of the image data. If the actual
    /// colourspace is unknown, then the value of the UnkC field in the Image Header
    /// box shall be set to 1 and the APPROX field shall specify the degree to
    /// which this Colour Specification box matches the correct definition of the
    /// assumed or target colourspace. In addition, high values of the APPROX field
    /// (indicating poor approximation) shall not be used to hide that the multiple
    /// Colour Specification boxes in either a Colour Group box or the JP2 Header
    /// box actually represent different colourspaces; the specification of multiple
    /// different colourspaces within a single Colour Group box is illegal. The
    /// legal values are:
    /// - 1: This colour specification method accurately represents the correct
    ///   definition of the colourspace.
    /// - 2: This colour specification method approximates the correct definition
    ///   of the colourspace with exceptional quality.
    /// - 3: This colour specification method approximates the correct definition
    ///   of the colourspace with reasonable quality.
    /// - 4: This colour specification method approximates the correct definition
    ///   of the colourspace with poor quality.
    ///
    /// Other values are reserved.
    ///
    /// This field is specified as 1 byte unsigned integer.
    pub fn colourspace_approximation(&self) -> u8 {
        self.colourspace_approximation[0]
    }
}

impl JBox for ColourSpecificationBox {
    // The type of a Colour Specification box shall be ‘colr’ (0x636F 6C72).
    fn identifier(&self) -> crate::BoxType {
        crate::BOX_TYPE_COLOUR_SPECIFICATION
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn decode<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<(), Box<dyn error::Error>> {
        let mut method = [0u8; 1];
        reader.read_exact(&mut method)?;
        reader.read_exact(&mut self.precedence)?;
        reader.read_exact(&mut self.colourspace_approximation)?;

        if self.precedence() != 0 {
            warn!("Precedence {:?} Unexpected", self.precedence());
        }
        if self.colourspace_approximation() != 0 {
            warn!(
                "Colourspace Approximation {:?} unexpected",
                self.colourspace_approximation()
            );
        }

        self.method = match method[0] {
            // 1 - Enumerated Colourspace.
            //
            // This colourspace specification box contains the enumerated value
            // of the colourspace of this image.
            //
            // The enumerated value is found in the EnumCS field in this box.
            // If the value of the METH field is 1, then the EnumCS shall exist
            // in this box immediately following the APPROX field, and the
            // EnumCS field shall be the last field in this box
            METHOD_ENUMERATED_COLOUR_SPACE => ColourSpecificationMethods::EnumeratedColourSpace {
                code: EnumeratedColourSpaces::decode(reader)?,
            },

            // 2 - Restricted ICC profile.
            // This Colour Specification box contains an ICC profile in the PROFILE field.
            //
            // This profile shall specify the transformation needed to convert the decompressed image data into the PCS_XYZ, and shall conform to either the Monochrome Input or Three-Component Matrix-Based Input profile class, and contain all the required tags specified therein, as defined in ICC.1:1998-09.
            //
            // As such, the value of the Profile Connection Space field in the profile header in the embedded profile shall be ‘XYZ\040’ (0x5859 5A20) indicating that the
            // output colourspace of the profile is in the XYZ colourspace.
            //
            // Any private tags in the ICC profile shall not change the visual appearance of an image processed using this ICC profile.
            //
            // The components from the codestream may have a range greater than the input range of the tone reproduction curve (TRC) of the ICC profile.
            //
            // Any decoded values should be clipped to the limits of the TRC before processing the image through the ICC profile.
            //
            // For example,
            // negative sample values of signed components may be clipped to zero before processing the image data through the profile.
            //
            // If the value of METH is 2, then the PROFILE field shall immediately follow the APPROX field and the PROFILE field shall be the last field in the box.
            METHOD_ENUMERATED_RESTRICTED_ICC_PROFILE => {
                let mut restricted_icc_profile = vec![0; self.length as usize - 3];

                reader.read_exact(&mut restricted_icc_profile)?;
                ColourSpecificationMethods::RestrictedICCProfile {
                    profile_data: restricted_icc_profile,
                }
            }
            METHOD_ENUMERATED_ANY_ICC_PROFILE => {
                let mut any_icc_profile = vec![0; self.length as usize - 3];

                reader.read_exact(&mut any_icc_profile)?;
                ColourSpecificationMethods::AnyICCProfile {
                    profile_data: any_icc_profile,
                }
            }
            METHOD_ENUMERATED_VENDOR_METHOD => {
                let mut vendor_defined_code = [0u8; 16];
                let mut vendor_parameters = vec![0; self.length as usize - (3 + 16)];
                reader.read_exact(&mut vendor_defined_code)?;
                reader.read_exact(&mut vendor_parameters)?;
                ColourSpecificationMethods::VendorColourMethod {
                    vendor_defined_code,
                    vendor_parameters,
                }
            }
            METHOD_ENUMERATED_PARAMETERIZED_COLOUR_SPACE => {
                let mut colprims = [0u8; 2];
                let mut transfc = [0u8; 2];
                let mut matcoeffs = [0u8; 2];
                let mut flags = [0u8; 1];
                reader.read_exact(&mut colprims)?;
                reader.read_exact(&mut transfc)?;
                reader.read_exact(&mut matcoeffs)?;
                reader.read_exact(&mut flags)?;
                ColourSpecificationMethods::ParameterizedColourspace {
                    colour_primaries: u16::from_be_bytes(colprims),
                    transfer_characteristics: u16::from_be_bytes(transfc),
                    matrix_coefficients: u16::from_be_bytes(matcoeffs),
                    video_full_range: flags[0] & 0x80 == 0x80,
                }
            }
            _ => {
                // skip over the METHDAT that we don't understand
                reader.seek_relative((self.length - 3) as i64)?;
                ColourSpecificationMethods::Reserved { value: method[0] }
            }
        };

        Ok(())
    }
}
