/// Sub-band types in the wavelet decomposition
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubBandType {
    /// Low-pass horizontal, Low-pass vertical
    LL,
    /// High-pass horizontal, Low-pass vertical
    HL,
    /// Low-pass horizontal, High-pass vertical
    LH,
    /// High-pass horizontal, High-pass vertical
    HH,
}

/// Two dimensional index
#[derive(Debug, Clone, Copy)]
pub struct I2 {
    pub x: u32,
    pub y: u32,
}
