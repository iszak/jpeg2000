use std::convert::TryInto;
use std::error;
use std::io;

pub const BOX_TYPE_READER_REQUIREMENTS: crate::BoxType = [0x72, 0x72, 0x65, 0x71];

/// Reader Requirements box.
///
/// This box is not permitted in a Part 1 (T.800 or ISO/IEC 15444-1) file.
///
/// This box is required in a Part 2 (T.801 or ISO/IEC 15444-2) file (i.e. "JPX").
///
/// The Reader Requirements box specifies what features or feature groups have been used in this JPX file, as well as what
/// combination of features shall be supported by a reader in order to fully use the file. The Reader Requirements box shall
/// immediately follow the File Type box, and there shall be one and only one Reader Requirements box in the file.
///
/// All features specified are in addition to the features defined by the JP2 file format and JPEG 2000 codestream profile 0;
/// it is assumed that any reader capable of reading a JPX file is also capable of understanding every feature defined in the
/// JP2 file format and decoding a JPEG 2000 profile 0 codestream.
///
/// This box shall contain an accurate specification, to the extent as known by the writer, of all features in the file and an
/// accurate specification of the set or sets of features required to display the image as intended by the writer.
///
/// NOTE: If a JPX file contains no features other than those defined by the JP2 file format and JPEG 2000 codestream profile 0, or
/// if the write does not know of any features contained in the file beyond those base features, the Reader Requirements box will list
/// zero standard features and zero vendor features.
///
/// Many features from previous revisions of ITU.801 | ISO/IEC 15444-2 have been deprecated. Writers shall not include these features
/// when creating or updating files. Readers shall ignore the contribution of those features when determining whether they can or
/// cannot read the file.
///
/// See ITU.801 | ISO/IEC 15444-2 Section M.6 and M.11.1 for further details.
#[derive(Debug, Default)]
pub struct ReaderRequirementsBox {
    length: u64,
    offset: u64,
    fuam: u128,
    dcm: u128,
    standard_flags: Vec<ExtensionFeatureRequirement>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(u16)]
/// Standard flags for reader requirements.
///
/// These flags are given in T.801(V3) (08/2023) | ISO/IEC 15444-2:2023 Table M.14.
pub enum FeatureFlag {
    /// File not completely understood.
    FileNotCompletelyUnderstood = 0,

    /// Codestream contains no extensions.
    CodestreamContainsNoExtensions = 1,

    /// Contains multiple composition layers.
    ContainsMultipleCompositionLayers = 2,

    /// Deprecated.
    ///  
    /// This feature is deprecated. Writers shall not include deprecated features when
    /// creating or updating files. Readers shall ignore the contribution of those
    /// features when determining whether they can or cannot read the file.
    Deprecated3 = 3,

    /// JPEG 2000 Core coding system Profile 1 codestream as defined in Rec. ITU-T T.800 | ISO/IEC 15444-1, Table A.45.
    CoreCodingSystemProfile1 = 4,

    /// Unrestricted JPEG 2000 Core coding system codestream as defined in Rec. ITU-T T.800 | ISO/IEC 15444-1.
    UnrestrictedJPEG2000CoreCodingSystemCodestream = 5,

    /// Unrestricted JPEG 2000 Extensions coding system codestream as defined in Rec. ITU-T T.801 | ISO/IEC 15444-2.
    UnrestrictedJPEG2000ExtensionsCodestream = 6,

    /// JPEG codestream as defined in ISO/IEC 10918-1.
    ///
    /// Note this is JPEG, not JPEG 2000.
    JpegCodestream = 7,

    /// Deprecated.
    ///  
    /// This feature is deprecated. Writers shall not include deprecated features when
    /// creating or updating files. Readers shall ignore the contribution of those
    /// features when determining whether they can or cannot read the file.
    ///
    /// Note: ITU T.801(08/2002) | ISO/IEC 15444-2:2003 used this value to mean "Does not contain opacity".
    Deprecated8 = 8,

    /// Non-premultiplied opacity channel
    NonPremultipliedOpacityChannel = 9,

    /// Premultiplied opacity channel
    PremultipliedOpacityChannel = 10,

    /// Chroma-key based opacity
    ChromaKeyOpacity = 11,

    /// Deprecated.
    ///  
    /// This feature is deprecated. Writers shall not include deprecated features when
    /// creating or updating files. Readers shall ignore the contribution of those
    /// features when determining whether they can or cannot read the file.
    ///
    /// Note: ITU T.801(08/2002) | ISO/IEC 15444-2:2003 used this value to mean "Codestream is contiguous".
    Deprecated12 = 12,

    /// Fragmented codestream where all fragments are in the file and in order.
    FragmentedCodestreamInOrderInFile = 13,

    /// Fragmented codestream where all fragments are in the file but are out of order.
    FragmentedCodestreamInFile = 14,

    /// Fragmented codestream where not all fragments are within the file but all are in locally accessible files.
    FragmentedCodestreamLocalFiles = 15,

    /// Fragmented codestream where some fragments may be accessible only through a URL specified network connection.
    FragmentedCodestreamRemoteFragments = 16,

    /// Compositing required to produce rendered result from multiple compositing layers.
    CompositingRequired = 17,

    /// Deprecated.
    ///   
    /// This feature is deprecated. Writers shall not include deprecated features when
    /// creating or updating files. Readers shall ignore the contribution of those
    /// features when determining whether they can or cannot read the file.
    ///
    /// Note: ITU T.801(08/2002) | ISO/IEC 15444-2:2003 used this value to mean "Support for compositing layers is not
    /// required (reader can load a single, discrete compositing layer)".
    Deprecated18 = 18,

    /// Deprecated.
    ///   
    /// This feature is deprecated. Writers shall not include deprecated features when
    /// creating or updating files. Readers shall ignore the contribution of those
    /// features when determining whether they can or cannot read the file.
    ///
    /// Note: ITU T.801(08/2002) | ISO/IEC 15444-2:2003 used this value to mean "Contains multiple discrete layers
    /// that should not be combined through either animation or compositing".
    Deprecated19 = 19,

    /// Deprecated.
    ///   
    /// This feature is deprecated. Writers shall not include deprecated features when
    /// creating or updating files. Readers shall ignore the contribution of those
    /// features when determining whether they can or cannot read the file.
    ///
    /// Note: ITU T.801(08/2002) | ISO/IEC 15444-2:2003 used this value to mean "Compositing layers each contain
    /// only a single codestream".
    Deprecated20 = 20,

    /// At least one compositing layer consists of multiple codestreams.
    CompostingLayerWithMultipleCodestreams = 21,

    /// Deprecated.
    ///   
    /// This feature is deprecated. Writers shall not include deprecated features when
    /// creating or updating files. Readers shall ignore the contribution of those
    /// features when determining whether they can or cannot read the file.
    ///
    /// Note: ITU T.801(08/2002) | ISO/IEC 15444-2:2003 used this value to mean "All compositing layers are in the same colourspace".
    Deprecated22 = 22,

    /// Colourspace transformations are required to combine compositing layers.
    ///
    /// Not all compositing layers are in the same colourspace.
    MultipleColourspaceCompositing = 23,

    /// Deprecated.
    ///   
    /// This feature is deprecated. Writers shall not include deprecated features when
    /// creating or updating files. Readers shall ignore the contribution of those
    /// features when determining whether they can or cannot read the file.
    ///
    /// Note: ITU T.801(08/2002) | ISO/IEC 15444-2:2003 used this value to mean "Rendered result created without using animation".
    Deprecated24 = 24,

    /// Animation
    Animation = 25,

    /// First animation layer does not cover entire rendered result area.
    FirstAnimationLayerNotEntireResult = 26,

    /// Deprecated.
    ///   
    /// This feature is deprecated. Writers shall not include deprecated features when
    /// creating or updating files. Readers shall ignore the contribution of those
    /// features when determining whether they can or cannot read the file.
    ///
    /// Note: ITU T.801(08/2002) | ISO/IEC 15444-2:2003 used this value to mean "Animated, and no layer is reused".
    Deprecated27 = 27,

    /// Re-use of animation layers
    AnimationLayerReuse = 28,

    /// Deprecated.
    ///
    /// This feature is deprecated. Writers shall not include deprecated features when
    /// creating or updating files. Readers shall ignore the contribution of those
    /// features when determining whether they can or cannot read the file.
    ///
    /// Note: ITU T.801(08/2002) | ISO/IEC 15444-2:2003 used this value to mean "Animated with persistent frames only".
    Deprecated29 = 29,

    /// Some animation frames are non-persistent.
    AnimationFramesNonPersistent = 30,

    /// Deprecated.
    ///
    /// This feature is deprecated. Writers shall not include deprecated features when
    /// creating or updating files. Readers shall ignore the contribution of those
    /// features when determining whether they can or cannot read the file.
    ///
    /// Note: ITU T.801(08/2002) | ISO/IEC 15444-2:2003 used this value to mean "Rendered result created without using scaling".
    Deprecated31 = 31,

    /// Rendered result involves scaling within a layer.
    RenderedResultsIntraLayerScaling = 32,

    /// Rendered result involves scaling between layers.
    RenderedResultsInterLayerScaling = 33,

    /// ROI metadata
    RegionOfInterestMetadata = 34,

    /// IPR metadata
    IntellectualPropertyMetadata = 35,

    /// Content metadata
    ContentMetadata = 36,

    /// History metadata
    HistoryMetadata = 37,

    /// Creation metadata
    CreationMetadata = 38,

    /// JPX digital signatures
    JpxDigitalSignatures = 39,

    /// JPX checksums
    JpxChecksums = 40,

    /// Desired Graphic Arts reproduction specified.
    DesiredGraphicArtsReproduction = 41,

    /// Deprecated.
    ///  
    /// This feature is deprecated. Writers shall not include deprecated features when
    /// creating or updating files. Readers shall ignore the contribution of those
    /// features when determining whether they can or cannot read the file.
    ///
    /// Note: ITU T.801(08/2002) | ISO/IEC 15444-2:2003 used this value to mean "Compositing layer uses palettized colour".
    Deprecated42 = 42,

    /// Deprecated.
    ///  
    /// This feature is deprecated. Writers shall not include deprecated features when
    /// creating or updating files. Readers shall ignore the contribution of those
    /// features when determining whether they can or cannot read the file.
    ///
    /// Note: ITU T.801(08/2002) | ISO/IEC 15444-2:2003 used this value to mean "Compositing layer uses Restricted ICC profile".
    Deprecated43 = 43,

    /// Compositing layer uses Any ICC profile.
    CompositingLayerAnyIccProfile = 44,

    /// Deprecated.
    ///  
    /// This feature is deprecated. Writers shall not include deprecated features when
    /// creating or updating files. Readers shall ignore the contribution of those
    /// features when determining whether they can or cannot read the file.
    ///
    /// Note: ITU T.801(08/2002) | ISO/IEC 15444-2:2003 used this value to mean "Compositing layer uses sRGB enumerated colourspace".
    Deprecated45 = 45,

    /// Deprecated.
    ///  
    /// This feature is deprecated. Writers shall not include deprecated features when
    /// creating or updating files. Readers shall ignore the contribution of those
    /// features when determining whether they can or cannot read the file.
    ///
    /// Note: ITU T.801(08/2002) | ISO/IEC 15444-2:2003 used this value to mean "Compositing layer uses sRGB-grey enumerated colourspace".
    Deprecated46 = 46,

    /// BiLevel 1 enumerated colourspace.
    BiLevel1EnumeratedColourspace = 47,

    /// BiLevel 2 enumerated colourspace.
    BiLevel2EnumeratedColourspace = 48,

    /// YCbCr 1 enumerated colourspace.
    YcbCr1EnumeratedColourspace = 49,

    /// YCbCr 2 enumerated colourspace.
    YcbCr2EnumeratedColourspace = 50,

    /// YCbCr 3 enumerated colourspace.
    YcbCr3EnumeratedColourspace = 51,

    /// PhotoYCC enumerated colourspace.
    PhotoYCCEnumeratedColourspace = 52,

    /// YCCK enumerated colourspace.
    YcckEnumeratedColourspace = 53,

    /// CMY enumerated colourspace.
    CmyEnumeratedColourspace = 54,

    /// CMYK enumerated colourspace.
    CmykEnumeratedColourspace = 55,

    /// CIELab enumerated colourspace with default parameters.
    CieLabEnumeratedColourspace = 56,

    /// CIELab enumerated colourspace with non-default parameters.
    CieLabEnumeratedColourspaceNonDefault = 57,

    /// CIEJab enumerated colourspace with default parameters.
    CieJabEnumeratedColourspace = 58,

    /// CIEJab enumerated colourspace with non-default parameters.
    CieJabEnumeratedColourspaceNonDefault = 59,

    /// e-sRGB enumerated colourspace.
    EsrgbEnumeratedColourspace = 60,

    /// ROMM-RGB enumerated colourspace.
    RommRgbEnumeratedColourspace = 61,

    /// Non-square samples.
    NonSquareSamples = 62,

    /// Deprecated.
    ///  
    /// This feature is deprecated. Writers shall not include deprecated features when
    /// creating or updating files. Readers shall ignore the contribution of those
    /// features when determining whether they can or cannot read the file.
    ///
    /// Note: ITU T.801(08/2002) | ISO/IEC 15444-2:2003 used this value to mean "Compositing layers have labels".
    Deprecated63 = 63,

    /// Deprecated.
    ///  
    /// This feature is deprecated. Writers shall not include deprecated features when
    /// creating or updating files. Readers shall ignore the contribution of those
    /// features when determining whether they can or cannot read the file.
    ///
    /// Note: ITU T.801(08/2002) | ISO/IEC 15444-2:2003 used this value to mean "Codestreams have labels".
    Deprecated64 = 64,

    /// Deprecated.
    ///  
    /// This feature is deprecated. Writers shall not include deprecated features when
    /// creating or updating files. Readers shall ignore the contribution of those
    /// features when determining whether they can or cannot read the file.
    ///
    /// Note: ITU T.801(08/2002) | ISO/IEC 15444-2:2003 used this value to mean "Compositing layers have different colour spaces".
    Deprecated65 = 65,

    /// Deprecated.
    ///  
    /// This feature is deprecated. Writers shall not include deprecated features when
    /// creating or updating files. Readers shall ignore the contribution of those
    /// features when determining whether they can or cannot read the file.
    ///
    /// Note: ITU T.801(08/2002) | ISO/IEC 15444-2:2003 used this value to mean "Compositing layers have different metadata".
    Deprecated66 = 66,

    /// GIS metadata XML box.
    GisMetadataXmlBox = 67,

    /// JPSEC extensions in codestream as specified in ISO/IEC 15444-8.
    Jpsec = 68,

    /// JP3D extensions in codestream as specified in ISO/IEC 15444-10.
    Jp3d = 69,

    /// Deprecated.
    ///  
    /// This feature is deprecated. Writers shall not include deprecated features when
    /// creating or updating files. Readers shall ignore the contribution of those
    /// features when determining whether they can or cannot read the file.
    ///
    /// Note: This was added in ITU-T Rec. T.801 (2002)/Cor.3 (01/2005) | ISO/IEC 15444-2:2005/Cor.3:2005 (E)
    /// as "Compositing layer uses sYCC enumerated colour space."
    Deprecated70 = 70,

    /// e-sYCC enumerated colourspace.
    EsyccEnumeratedColourspace = 71,

    /// JPEG 2000 Extensions codestream as restricted by baseline conformance requirements in clause M.9.2.3.
    ExtensionRestrictedBaseline = 72,

    /// YPbPr(1125/60) enumerated colourspace.
    Ypbpr1125EnumeratedColourspace = 73,

    /// YPbPr(1250/50) enumerated colourspace.
    Ypbpr1250EnumeratedColourspace = 74,

    /// Codestream contains a JPEG XR (Rec. ITU-T T.832 | ISO/IEC 29199-2) compliant bitstream.
    ///
    /// Note: This was introduced in Amendment 3 (03/2013) to ITU T.801(08/2002) | ISO/IEC 15444-2:2004/Amd.3:2015 (E).
    JpegXrBitstream = 75,

    /// Codestream contains a Sub-baseline profile JPEG XR (Rec. ITU-T T.832 | ISO/IEC 29199-2) compliant bitstream.
    ///
    /// Note: This was introduced in Amendment 3 (03/2013) to ITU T.801(08/2002) | ISO/IEC 15444-2:2004/Amd.3:2015 (E).
    JpegXrSubBaselineBitstream = 76,

    /// Codestream contains a Baseline profile JPEG XR (Rec. ITU-T T.832 | ISO/IEC 29199-2) compliant bitstream.
    ///
    /// Note: This was introduced in Amendment 3 (03/2013) to ITU T.801(08/2002) | ISO/IEC 15444-2:2004/Amd.3:2015 (E).
    JpegXrBaselineBitstream = 77,

    /// Codestream contains a Main profile JPEG XR (Rec. ITU-T T.832 | ISO/IEC 29199-2) compliant bitstream.
    ///
    /// Note: This was introduced in Amendment 3 (03/2013) to ITU T.801(08/2002) | ISO/IEC 15444-2:2004/Amd.3:2015 (E).
    JpegXrMainBitstream = 78,

    /// Codestream contains an Advanced profile JPEG XR (Rec. ITU-T T.832 | ISO/IEC 29199-2) compliant bitstream.
    ///
    /// Note: This was introduced in Amendment 3 (03/2013) to ITU T.801(08/2002) | ISO/IEC 15444-2:2004/Amd.3:2015 (E).
    JpegXrAdvancedBitstream = 79,

    /// Pixel format "Fixed Point" is used.
    ///
    /// Note: This was introduced in Amendment 3 (03/2013) to ITU T.801(08/2002) | ISO/IEC 15444-2:2004/Amd.3:2015 (E).
    PixelFormatFixedPoint = 80,

    /// Pixel format "Floating Point" is used.
    ///
    /// Note: This was introduced in Amendment 3 (03/2013) to ITU T.801(08/2002) | ISO/IEC 15444-2:2004/Amd.3:2015 (E).
    PixelFormatFloatingPoint = 81,

    /// Pixel format "Mantissa" or "Exponent" is used.
    ///
    /// Note: This was introduced in Amendment 3 (03/2013) to ITU T.801(08/2002) | ISO/IEC 15444-2:2004/Amd.3:2015 (E).
    PixelFormatMantissaOrExponent = 82,

    /// Compositing layer uses IEC 61966-2-2 (scRGB) enumerated colourspace.
    ///
    /// Note: This was introduced in Amendment 3 (03/2013) to ITU T.801(08/2002) | ISO/IEC 15444-2:2004/Amd.3:2015 (E).
    CompositingLayerScrgbColourspace = 83,

    /// Block Coder Extensions (Annex P)
    ///
    /// Note: This was introduced in Amendment 3 (03/2013) to ITU T.801(08/2002) | ISO/IEC 15444-2:2004/Amd.3:2015 (E)
    /// and Rec. ITU-T T.801 (2002)/Amd.4 (06/2012) | ISO/IEC 15444-2:2004/Amd.4:2015 (E). The description of the
    /// Block Coder Extension is in Rec. ITU-T T.801 (2002)/Amd.4 (06/2012) | ISO/IEC 15444-2:2004/Amd.4:2015 (E).
    BlockCoderAnnexP = 84,

    /// Compositing layer uses scRGB gray scale (IEC 61966-2-2 based) enumerated colourspace.
    ///
    /// Note: This was introduced in Amendment 3 (03/2013) to ITU T.801(08/2002) | ISO/IEC 15444-2:2004/Amd.3:2015 (E).
    CompositingLayerScrgbGrayScaleColourspace = 85,

    /// JPEG 2000 codestream capabilities specified in Rec. ITU-T T.814 | ISO/IEC 15444-15.
    HighThroughputCodestream = 86,

    /// Flag was not a recognised value.
    ///
    /// This means it is not listed in T.801(V3) | ISO/IEC 15444-2:2023 Table M.14. Other values
    /// can be listed above and not supported at parse or decode time.
    UnsupportedFlag = u16::MAX,
}

impl From<u16> for FeatureFlag {
    fn from(value: u16) -> Self {
        match value {
            0 => Self::FileNotCompletelyUnderstood,
            1 => Self::CodestreamContainsNoExtensions,
            2 => Self::ContainsMultipleCompositionLayers,
            3 => Self::Deprecated3,
            4 => Self::CoreCodingSystemProfile1,
            5 => Self::UnrestrictedJPEG2000CoreCodingSystemCodestream,
            6 => Self::UnrestrictedJPEG2000ExtensionsCodestream,
            7 => Self::JpegCodestream,
            8 => Self::Deprecated8,
            9 => Self::NonPremultipliedOpacityChannel,
            10 => Self::PremultipliedOpacityChannel,
            11 => Self::ChromaKeyOpacity,
            12 => Self::Deprecated12,
            13 => Self::FragmentedCodestreamInOrderInFile,
            14 => Self::FragmentedCodestreamInFile,
            15 => Self::FragmentedCodestreamLocalFiles,
            16 => Self::FragmentedCodestreamRemoteFragments,
            17 => Self::CompositingRequired,
            18 => Self::Deprecated18,
            19 => Self::Deprecated19,
            20 => Self::Deprecated20,
            21 => Self::CompostingLayerWithMultipleCodestreams,
            22 => Self::Deprecated22,
            23 => Self::MultipleColourspaceCompositing,
            24 => Self::Deprecated24,
            25 => Self::Animation,
            26 => Self::FirstAnimationLayerNotEntireResult,
            27 => Self::Deprecated27,
            28 => Self::AnimationLayerReuse,
            29 => Self::Deprecated29,
            30 => Self::AnimationFramesNonPersistent,
            31 => Self::Deprecated31,
            32 => Self::RenderedResultsIntraLayerScaling,
            33 => Self::RenderedResultsInterLayerScaling,
            34 => Self::RegionOfInterestMetadata,
            35 => Self::IntellectualPropertyMetadata,
            36 => Self::ContentMetadata,
            37 => Self::HistoryMetadata,
            38 => Self::CreationMetadata,
            39 => Self::JpxDigitalSignatures,
            40 => Self::JpxChecksums,
            41 => Self::DesiredGraphicArtsReproduction,
            42 => Self::Deprecated42,
            43 => Self::Deprecated43,
            44 => Self::CompositingLayerAnyIccProfile,
            45 => Self::Deprecated45,
            46 => Self::Deprecated46,
            47 => Self::BiLevel1EnumeratedColourspace,
            48 => Self::BiLevel2EnumeratedColourspace,
            49 => Self::YcbCr1EnumeratedColourspace,
            50 => Self::YcbCr2EnumeratedColourspace,
            51 => Self::YcbCr3EnumeratedColourspace,
            52 => Self::PhotoYCCEnumeratedColourspace,
            53 => Self::YcckEnumeratedColourspace,
            54 => Self::CmyEnumeratedColourspace,
            55 => Self::CmykEnumeratedColourspace,
            56 => Self::CieLabEnumeratedColourspace,
            57 => Self::CieLabEnumeratedColourspaceNonDefault,
            58 => Self::CieJabEnumeratedColourspace,
            59 => Self::CieJabEnumeratedColourspaceNonDefault,
            60 => Self::EsrgbEnumeratedColourspace,
            61 => Self::RommRgbEnumeratedColourspace,
            62 => Self::NonSquareSamples,
            63 => Self::Deprecated63,
            64 => Self::Deprecated64,
            65 => Self::Deprecated65,
            66 => Self::Deprecated66,
            67 => Self::GisMetadataXmlBox,
            68 => Self::Jpsec,
            69 => Self::Jp3d,
            70 => Self::Deprecated70,
            71 => Self::EsyccEnumeratedColourspace,
            72 => Self::ExtensionRestrictedBaseline,
            73 => Self::Ypbpr1125EnumeratedColourspace,
            74 => Self::Ypbpr1250EnumeratedColourspace,
            75 => Self::JpegXrBitstream,
            76 => Self::JpegXrSubBaselineBitstream,
            77 => Self::JpegXrBaselineBitstream,
            78 => Self::JpegXrMainBitstream,
            79 => Self::JpegXrAdvancedBitstream,
            80 => Self::PixelFormatFixedPoint,
            81 => Self::PixelFormatFloatingPoint,
            82 => Self::PixelFormatMantissaOrExponent,
            83 => Self::CompositingLayerScrgbColourspace,
            84 => Self::BlockCoderAnnexP,
            85 => Self::CompositingLayerScrgbGrayScaleColourspace,
            86 => Self::HighThroughputCodestream,
            _ => Self::UnsupportedFlag,
        }
    }
}

impl FeatureFlag {
    fn is_deprecated(&self) -> bool {
        matches!(
            self,
            Self::Deprecated3
                | Self::Deprecated8
                | Self::Deprecated12
                | Self::Deprecated18
                | Self::Deprecated19
                | Self::Deprecated20
                | Self::Deprecated22
                | Self::Deprecated24
                | Self::Deprecated27
                | Self::Deprecated29
                | Self::Deprecated31
                | Self::Deprecated42
                | Self::Deprecated43
                | Self::Deprecated45
                | Self::Deprecated46
                | Self::Deprecated63
                | Self::Deprecated64
                | Self::Deprecated65
                | Self::Deprecated66
                | Self::Deprecated70
        )
    }
}

#[derive(Debug, PartialEq)]
pub struct ExtensionFeatureRequirement {
    pub flag: FeatureFlag,
    pub mask: u128,
}

impl ExtensionFeatureRequirement {
    fn decode<R: io::Read + io::Seek>(
        reader: &mut R,
        ml: usize,
    ) -> Result<Self, Box<dyn error::Error>> {
        let mut sf_bytes = [0u8; 2];
        reader.read_exact(&mut sf_bytes)?;
        let sf = u16::from_be_bytes(sf_bytes);
        let flag = sf.into();
        assert_ne!(flag, FeatureFlag::UnsupportedFlag, "flag value: {sf:?}");

        let mut mask_bytes = vec![0u8; ml];
        reader.read_exact(&mut mask_bytes)?;
        let mask = match ml {
            1 => mask_bytes[0] as u128,
            2 => u16::from_be_bytes(mask_bytes[0..2].try_into().unwrap()) as u128,
            4 => u32::from_be_bytes(mask_bytes[0..4].try_into().unwrap()) as u128,
            8 => u64::from_be_bytes(mask_bytes[0..8].try_into().unwrap()) as u128,
            16 => u128::from_be_bytes(mask_bytes[0..16].try_into().unwrap()) as u128,
            _ => unreachable!(),
        };

        Ok(ExtensionFeatureRequirement { flag, mask })
    }
}

impl ReaderRequirementsBox {
    pub fn new(length: u64, offset: u64) -> ReaderRequirementsBox {
        ReaderRequirementsBox {
            length,
            offset,
            ..Default::default()
        }
    }
}

impl crate::JBox for ReaderRequirementsBox {
    // The type of a Reader Requirements box shall be 'rreq' (0x7272 6571').
    fn identifier(&self) -> crate::BoxType {
        BOX_TYPE_READER_REQUIREMENTS
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
        let mut mask_length = [0u8; 1];
        reader.read_exact(&mut mask_length)?;
        let ml = mask_length[0] as usize;
        if !matches!(ml, 1 | 2 | 4 | 8 | 16) {
            return Err(crate::JP2Error::ExcessiveSize {
                box_type: BOX_TYPE_READER_REQUIREMENTS,
                offset: self.offset,
            }
            .into());
        }
        let mut fuam_bytes = vec![0u8; ml];
        let mut dcm_bytes = vec![0u8; ml];
        reader.read_exact(&mut fuam_bytes)?;
        reader.read_exact(&mut dcm_bytes)?;
        let (fuam, dcm) = match ml {
            1 => (fuam_bytes[0] as u128, dcm_bytes[0] as u128),
            2 => (
                u16::from_be_bytes(fuam_bytes[0..2].try_into().unwrap()) as u128,
                u16::from_be_bytes(dcm_bytes[0..2].try_into().unwrap()) as u128,
            ),
            4 => (
                u32::from_be_bytes(fuam_bytes[0..4].try_into().unwrap()) as u128,
                u32::from_be_bytes(dcm_bytes[0..4].try_into().unwrap()) as u128,
            ),
            8 => (
                u64::from_be_bytes(fuam_bytes[0..8].try_into().unwrap()) as u128,
                u64::from_be_bytes(dcm_bytes[0..8].try_into().unwrap()) as u128,
            ),
            16 => (
                u128::from_be_bytes(fuam_bytes[0..16].try_into().unwrap()) as u128,
                u128::from_be_bytes(dcm_bytes[0..16].try_into().unwrap()) as u128,
            ),
            _ => unreachable!(),
        };
        self.fuam = fuam;
        self.dcm = dcm;
        let mut nsf_bytes = [0u8; 2];
        reader.read_exact(&mut nsf_bytes)?;
        let nsf = u16::from_be_bytes(nsf_bytes);
        if nsf > 100 {
            // There are only 86 flags defined, and not all of them are still valid
            return Err(crate::JP2Error::ExcessiveSize {
                box_type: BOX_TYPE_READER_REQUIREMENTS,
                offset: self.offset,
            }
            .into());
        }
        self.standard_flags = Vec::with_capacity(nsf.into());
        for _ in 0..nsf {
            let feature_flag = ExtensionFeatureRequirement::decode(reader, ml)?;
            self.standard_flags.push(feature_flag);
        }
        let mut nvf_bytes = [0u8; 2];
        reader.read_exact(&mut nvf_bytes)?;
        let nvf = u16::from_be_bytes(nvf_bytes);
        if nvf > 10 {
            // vendor flags should be rare
            return Err(crate::JP2Error::ExcessiveSize {
                box_type: BOX_TYPE_READER_REQUIREMENTS,
                offset: self.offset,
            }
            .into());
        }
        if nvf != 0 {
            // TODO: vendor feature flags
            return Err(crate::JP2Error::Unsupported.into());
        }
        Ok(())
    }
}

impl ReaderRequirementsBox {
    pub fn standard_flags(&self) -> &Vec<ExtensionFeatureRequirement> {
        &self.standard_flags
    }
}

mod test {
    #[test]
    fn test_feature_enum_round_trip() {
        for i in 0..87 {
            let flag = crate::extension::FeatureFlag::from(i);
            let value = flag as u16;
            assert_eq!(value as u16, i, "mismatch for {flag:?}");
        }
    }

    #[test]
    fn deprecated_feature_3() {
        let flag = crate::extension::FeatureFlag::from(3);
        assert!(flag.is_deprecated());
    }

    #[test]
    fn deprecated_feature_8() {
        let flag = crate::extension::FeatureFlag::from(8);
        assert!(flag.is_deprecated());
    }

    #[test]
    fn deprecated_feature_12() {
        let flag = crate::extension::FeatureFlag::from(12);
        assert!(flag.is_deprecated());
    }

    #[test]
    fn deprecated_feature_18() {
        let flag = crate::extension::FeatureFlag::from(18);
        assert!(flag.is_deprecated());
    }

    #[test]
    fn deprecated_feature_19() {
        let flag = crate::extension::FeatureFlag::from(19);
        assert!(flag.is_deprecated());
    }

    #[test]
    fn deprecated_feature_20() {
        let flag = crate::extension::FeatureFlag::from(20);
        assert!(flag.is_deprecated());
    }

    #[test]
    fn deprecated_feature_22() {
        let flag = crate::extension::FeatureFlag::from(22);
        assert!(flag.is_deprecated());
    }

    #[test]
    fn deprecated_feature_24() {
        let flag = crate::extension::FeatureFlag::from(24);
        assert!(flag.is_deprecated());
    }

    #[test]
    fn deprecated_feature_27() {
        let flag = crate::extension::FeatureFlag::from(27);
        assert!(flag.is_deprecated());
    }

    #[test]
    fn deprecated_feature_29() {
        let flag = crate::extension::FeatureFlag::from(29);
        assert!(flag.is_deprecated());
    }

    #[test]
    fn deprecated_feature_31() {
        let flag = crate::extension::FeatureFlag::from(31);
        assert!(flag.is_deprecated());
    }

    #[test]
    fn deprecated_feature_42() {
        let flag = crate::extension::FeatureFlag::from(42);
        assert!(flag.is_deprecated());
    }

    #[test]
    fn deprecated_feature_43() {
        let flag = crate::extension::FeatureFlag::from(43);
        assert!(flag.is_deprecated());
    }

    #[test]
    fn deprecated_feature_45() {
        let flag = crate::extension::FeatureFlag::from(45);
        assert!(flag.is_deprecated());
    }

    #[test]
    fn deprecated_feature_46() {
        let flag = crate::extension::FeatureFlag::from(46);
        assert!(flag.is_deprecated());
    }

    #[test]
    fn deprecated_feature_63() {
        let flag = crate::extension::FeatureFlag::from(63);
        assert!(flag.is_deprecated());
    }

    #[test]
    fn deprecated_feature_64() {
        let flag = crate::extension::FeatureFlag::from(64);
        assert!(flag.is_deprecated());
    }

    #[test]
    fn deprecated_feature_65() {
        let flag = crate::extension::FeatureFlag::from(65);
        assert!(flag.is_deprecated());
    }

    #[test]
    fn deprecated_feature_66() {
        let flag = crate::extension::FeatureFlag::from(66);
        assert!(flag.is_deprecated());
    }

    #[test]
    fn deprecated_feature_70() {
        let flag = crate::extension::FeatureFlag::from(70);
        assert!(flag.is_deprecated());
    }

    #[test]
    fn not_deprecated_feature_1() {
        let flag = crate::extension::FeatureFlag::from(1);
        assert!(!flag.is_deprecated());
    }

    #[test]
    fn not_deprecated_feature_86() {
        let flag = crate::extension::FeatureFlag::from(86);
        assert!(!flag.is_deprecated());
    }
}
