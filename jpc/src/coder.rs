//! MQ-Coder: Arithmetic Entropy Coding for JPEG2000
//! Implementation based on ISO/IEC 15444-1:2019 Annex C

/// Probability estimation state table entry
#[derive(Debug, Clone, Copy)]
struct QeEntry {
    qe: u16,      // Probability estimate (15-bit value)
    nmps: u8,     // Next state if MPS (more probable symbol) is coded
    nlps: u8,     // Next state if LPS (less probable symbol) is coded
    switch: bool, // Whether to switch MPS sense
}

/// Table C.2 - Qe values and probability estimation
const QE_TABLE: [QeEntry; 47] = [
    entry(0x5601, 1, 1, true),
    entry(0x3401, 2, 6, false),
    entry(0x1801, 3, 9, false),
    entry(0x0AC1, 4, 12, false),
    entry(0x0521, 5, 29, false),
    entry(0x0221, 38, 33, false),
    entry(0x5601, 7, 6, true),
    entry(0x5401, 8, 14, false),
    entry(0x4801, 9, 14, false),
    entry(0x3801, 10, 14, false),
    entry(0x3001, 11, 17, false),
    entry(0x2401, 12, 18, false),
    entry(0x1C01, 13, 20, false),
    entry(0x1601, 29, 21, false),
    entry(0x5601, 15, 14, true),
    entry(0x5401, 16, 14, false),
    entry(0x5101, 17, 15, false),
    entry(0x4801, 18, 16, false),
    entry(0x3801, 19, 17, false),
    entry(0x3401, 20, 18, false),
    entry(0x3001, 21, 19, false),
    entry(0x2801, 22, 19, false),
    entry(0x2401, 23, 20, false),
    entry(0x2201, 24, 21, false),
    entry(0x1C01, 25, 22, false),
    entry(0x1801, 26, 23, false),
    entry(0x1601, 27, 24, false),
    entry(0x1401, 28, 25, false),
    entry(0x1201, 29, 26, false),
    entry(0x1101, 30, 27, false),
    entry(0x0AC1, 31, 28, false),
    entry(0x09C1, 32, 29, false),
    entry(0x08A1, 33, 30, false),
    entry(0x0521, 34, 31, false),
    entry(0x0441, 35, 32, false),
    entry(0x02A1, 36, 33, false),
    entry(0x0221, 37, 34, false),
    entry(0x0141, 38, 35, false),
    entry(0x0111, 39, 36, false),
    entry(0x0085, 40, 37, false),
    entry(0x0049, 41, 38, false),
    entry(0x0025, 42, 39, false),
    entry(0x0015, 43, 40, false),
    entry(0x0009, 44, 41, false),
    entry(0x0005, 45, 42, false),
    entry(0x0001, 45, 43, false),
    entry(0x5601, 46, 46, false),
];
const fn entry(qe: u16, nmps: u8, nlps: u8, switch: bool) -> QeEntry {
    QeEntry {
        qe,
        nmps,
        nlps,
        switch,
    }
}

/// Context state for probability estimation
#[derive(Default, Debug, Clone, Copy)]
struct ContextState {
    index: u8, // Index into QE_TABLE
    mps: u8,   // More probable symbol (0 or 1)
}

/// Special contexts
pub const UNIFORM: usize = 18;
pub const RUN_LEN: usize = 17;
pub const ZERO_CTX: usize = 0;

/// MQ Encoder
pub struct MqEncoder {
    a: u16,                      // Interval register (16-bit)
    c: u32,                      // Code register (32-bit)
    ct: i32,                     // Bit counter
    buffer: Vec<u8>,             // Output buffer
    bp: usize,                   // Buffer pointer (points to last byte written)
    contexts: Vec<ContextState>, // Context states
}

impl MqEncoder {
    /// Create a new MQ encoder with specified number of contexts
    pub fn new(num_contexts: usize) -> Self {
        MqEncoder {
            a: 0,
            c: 0,
            ct: 0,
            buffer: Vec::new(),
            bp: 0,
            contexts: vec![ContextState::default(); num_contexts],
        }
    }

    /// Initialize the encoder (INITENC procedure, Figure C.10)
    pub fn init(&mut self) {
        self.a = 0x8000; // Set A to 0.75 in fixed-point
        self.c = 0;
        self.buffer.clear();
        self.buffer.push(0); // Initial byte (BP points before first byte)
        self.bp = 0;
        self.ct = 12; // Account for spacer bits

        // Check if preceding byte is 0xFF
        if self.bp > 0 && self.buffer[self.bp] == 0xFF {
            self.ct = 13;
        }
    }

    /// Reset contexts to initial values.
    ///
    /// Also used for re-initialization when required.
    ///
    /// See ITU T.800 (V4) | ISO/IEC 15444-1:2024 Table D.7
    pub fn reset_contexts(&mut self) {
        assert!(self.contexts.len() == 19);
        for i in 0..19 {
            self.contexts[i].index = 0;
            self.contexts[i].mps = 0;
        }
        self.contexts[UNIFORM] = ContextState { index: 46, mps: 0 };
        self.contexts[RUN_LEN] = ContextState { index: 3, mps: 0 };
        self.contexts[ZERO_CTX] = ContextState { index: 4, mps: 0 };
    }

    /// Encode a decision (ENCODE procedure).
    ///
    /// From ITU-T T.800 (V4) | ISO/IEC 15444-1:2024 Section C.2.2:
    /// > The ENCODE procedure determines whether the decision D is a 0 or not. Then a CODE0 or a
    /// > CODE1 procedure is called appropriately. Often embodiments will not have an ENCODE
    /// > procedure, but will call the CODE0 or CODE1 procedures directly to code a 0-decision or a
    /// > 1-decision. Figure C.3 shows this procedure.
    pub fn encode(&mut self, cx: usize, d: u8) {
        if d == 0 {
            self.code0(cx);
        } else {
            self.code1(cx);
        }
    }

    /// CODE0 procedure.
    ///
    /// See ITU-T T.800 (V4) | ISO/IEC 15444-1:2024 Section C.2.3.
    fn code0(&mut self, cx: usize) {
        let mps = self.contexts[cx].mps;
        if mps == 0 {
            self.code_mps(cx);
        } else {
            self.code_lps(cx);
        }
    }

    /// CODE1 procedure.
    ///
    /// See ITU-T T.800 (V4) | ISO/IEC 15444-1:2024 Section C.2.3.
    fn code1(&mut self, cx: usize) {
        let mps = self.contexts[cx].mps;
        if mps == 1 {
            self.code_mps(cx);
        } else {
            self.code_lps(cx);
        }
    }

    /// CODEMPS procedure with conditional MPS/LPS exchange.
    ///
    /// See See ITU-T T.800 (V4) | ISO/IEC 15444-1:2024 Figure C.7.
    fn code_mps(&mut self, cx: usize) {
        let index = self.contexts[cx].index as usize;
        let qe = QE_TABLE[index].qe;

        self.a -= qe;

        if (self.a & 0x8000) == 0 {
            // Conditional exchange needed
            if self.a < qe {
                self.a = qe;
            } else {
                self.c += qe as u32;
            }
            self.contexts[cx].index = QE_TABLE[index].nmps;
            self.renorm_e();
        } else {
            self.c += qe as u32;
        }
    }

    /// CODELPS procedure with conditional MPS/LPS exchange.
    ///
    /// See See ITU-T T.800 (V4) | ISO/IEC 15444-1:2024 Figure C.6.
    fn code_lps(&mut self, cx: usize) {
        let index = self.contexts[cx].index as usize;
        let qe = QE_TABLE[index].qe;

        self.a -= qe;

        if self.a < qe {
            self.c += qe as u32;
        } else {
            self.a = qe;
        }

        // Update probability estimate
        if QE_TABLE[index].switch {
            self.contexts[cx].mps = 1 - self.contexts[cx].mps;
        }
        self.contexts[cx].index = QE_TABLE[index].nlps;

        self.renorm_e();
    }

    /// RENORME - Encoder renormalization.
    ///
    /// See See ITU-T T.800 (V4) | ISO/IEC 15444-1:2024 Figure C.8.
    fn renorm_e(&mut self) {
        loop {
            self.a <<= 1;
            self.c <<= 1;
            self.ct -= 1;

            if self.ct == 0 {
                self.byte_out();
            }

            if (self.a & 0x8000) == 0 {
                continue;
            } else {
                break;
            }
        }
    }

    /// BYTEOUT - Output a byte of compressed data.
    ///
    /// See See ITU-T T.800 (V4) | ISO/IEC 15444-1:2024 Figure C.9.
    fn byte_out(&mut self) {
        if self.bp >= self.buffer.len() {
            self.buffer.push(0); // TODO clean up use of self.buffer
        }

        let mut b = self.buffer[self.bp];

        if b == 0xFF {
            // Bit stuffing after 0xFF
            self.bp += 1;
            let c_high = ((self.c >> 20) & 0xFF) as u8;
            if self.bp >= self.buffer.len() {
                self.buffer.push(0);
            }
            self.buffer[self.bp] = c_high;
            self.c &= 0xF_FFFF; // Keep lower 20 bits
            self.ct = 7;
        } else {
            // Check for carry
            if (self.c & 0x8000000) != 0 {
                b += 1;
                self.buffer[self.bp] = b;

                if b == 0xFF {
                    self.c &= 0x7FF_FFFF;
                    self.bp += 1;
                    if self.bp >= self.buffer.len() {
                        self.buffer.push(0);
                    }
                    let c_high = ((self.c >> 20) & 0xFF) as u8;
                    self.buffer[self.bp] = c_high;
                    self.c &= 0xF_FFFF;
                    self.ct = 7;
                    return;
                }
            }

            self.bp += 1;
            if self.bp >= self.buffer.len() {
                self.buffer.push(0);
            }
            let c_high = ((self.c >> 19) & 0xFF) as u8;
            self.buffer[self.bp] = c_high;
            self.c &= 0x7_FFFF; // Keep lower 19 bits
            self.ct = 8;
        }
    }

    /// FLUSH - Terminate encoding.
    ///
    /// See See ITU-T T.800 (V4) | ISO/IEC 15444-1:2024 Figure C.11.
    pub fn flush(&mut self) -> Vec<u8> {
        self.set_bits();
        self.c <<= self.ct as u32;
        self.byte_out();
        self.c <<= self.ct as u32;
        self.byte_out();

        // Remove trailing 0xFF if present
        if self.bp < self.buffer.len() && self.buffer[self.bp] == 0xFF {
            self.buffer.truncate(self.bp);
        } else {
            self.buffer.truncate(self.bp + 1);
        }

        self.buffer.clone().drain(1..).collect() // TODO clean up
    }

    /// SETBITS - Set final bits in C register.
    ///
    /// See See ITU-T T.800 (V4) | ISO/IEC 15444-1:2024 Figure C.12.
    fn set_bits(&mut self) {
        let temp = self.c + self.a as u32;
        self.c |= 0xFFFF;
        if self.c >= temp {
            self.c -= 0x8000;
        }
    }
}

pub trait Decoder {
    fn decode_bit(&mut self, cx: usize) -> u8;
}

pub fn standard_decoder(bytes: &[u8]) -> MqDecoder {
    let mut decoder = MqDecoder::new(19);
    decoder.reset_contexts();
    decoder.init(bytes);
    decoder
}

/// MQ Decoder
#[derive(Debug)]
pub struct MqDecoder {
    a: u16, // Interval register (16-bit)
    c: u32,
    ct: i32,                     // Bit counter
    buffer: Vec<u8>,             // Input buffer
    bp: usize,                   // Buffer pointer
    contexts: Vec<ContextState>, // Context states
}

impl MqDecoder {
    /// Create a new MQ decoder with specified number of contexts
    pub fn new(num_contexts: usize) -> Self {
        MqDecoder {
            a: 0,
            c: 0,
            ct: 0,
            buffer: Vec::new(),
            bp: 0,
            contexts: vec![ContextState::default(); num_contexts],
        }
    }

    /// Initialize the decoder with compressed data (INITDEC procedure).
    ///
    /// See See ITU-T T.800 (V4) | ISO/IEC 15444-1:2024 Figure C.20.
    pub fn init(&mut self, data: &[u8]) {
        self.buffer = data.to_vec();
        self.bp = 0;
        self.ct = 0;
        self.c = 0;

        // Read first byte into C (Figure C.20: C = B << 16)
        // This puts byte in bits 23-16 of the combined 32-bit C register
        if self.bp < self.buffer.len() {
            let b = self.buffer[self.bp];
            self.bp += 1;
            self.c = (b as u32) << 16; // Byte goes to bits 23-16 of combined C
        }

        // Read second byte (BYTEIN)
        self.byte_in();

        // Shift C by 7 bits (Figure C.20: C = C << 7)
        self.c <<= 7;
        self.ct -= 7;
        self.a = 0x8000;
    }

    /// Reset contexts to initial values.
    ///
    /// Also used for re-initialization when required.
    ///
    /// See ITU T.800 (V4) | ISO/IEC 15444-1:2024 Section C.3.6 and Table D.7
    pub fn reset_contexts(&mut self) {
        assert!(self.contexts.len() == 19);
        for i in 0..19 {
            self.contexts[i].index = 0;
            self.contexts[i].mps = 0;
        }
        self.contexts[UNIFORM] = ContextState { index: 46, mps: 0 };
        self.contexts[RUN_LEN] = ContextState { index: 3, mps: 0 };
        self.contexts[ZERO_CTX] = ContextState { index: 4, mps: 0 };
    }

    /// Decode a decision (DECODE procedure).
    ///
    /// See ITU T.800 (V4) | ISO/IEC 15444-1:2024 Figure C.15
    pub fn decode(&mut self, cx: usize) -> u8 {
        let index = self.contexts[cx].index as usize;
        let qe = QE_TABLE[index].qe;

        self.a -= qe;
        let c_high = self.c >> 16;

        if c_high < qe as u32 {
            // LPS path
            let d = self.lps_exchange(cx);
            self.renorm_d();
            d
        } else {
            self.c -= (qe as u32) << 16;
            if (self.a & 0x8000) == 0 {
                // MPS path with renormalization
                let d = self.mps_exchange(cx);
                self.renorm_d();
                d
            } else {
                // MPS without renormalization
                self.contexts[cx].mps
            }
        }
    }

    /// MPS_EXCHANGE - Handle MPS conditional exchange
    fn mps_exchange(&mut self, cx: usize) -> u8 {
        let index = self.contexts[cx].index as usize;
        let qe = QE_TABLE[index].qe;

        if self.a < qe {
            // Conditional exchange occurred - LPS decoded
            let d = 1 - self.contexts[cx].mps;
            if QE_TABLE[index].switch {
                self.contexts[cx].mps = 1 - self.contexts[cx].mps;
            }
            self.contexts[cx].index = QE_TABLE[index].nlps;
            d
        } else {
            // MPS decoded
            let d = self.contexts[cx].mps;
            self.contexts[cx].index = QE_TABLE[index].nmps;
            d
        }
    }

    /// LPS_EXCHANGE - Handle LPS conditional exchange.
    ///
    /// See ITU T.800 (V4) | ISO/IEC 15444-1:2024 Figure C.17.
    fn lps_exchange(&mut self, cx: usize) -> u8 {
        let index = self.contexts[cx].index as usize;
        let qe = QE_TABLE[index].qe;

        if self.a < qe {
            // Conditional exchange - MPS decoded
            self.a = qe;
            let d = self.contexts[cx].mps;
            self.contexts[cx].index = QE_TABLE[index].nmps;
            d
        } else {
            // No exchange - LPS decoded
            self.a = qe;
            let d = 1 - self.contexts[cx].mps;
            if QE_TABLE[index].switch {
                self.contexts[cx].mps = 1 - self.contexts[cx].mps;
            }
            self.contexts[cx].index = QE_TABLE[index].nlps;
            d
        }
    }

    /// RENORMD - Decoder renormalization.
    ///
    /// See ITU T.800 (V4) | ISO/IEC 15444-1:2024 Figure C.18
    fn renorm_d(&mut self) {
        loop {
            if self.ct == 0 {
                self.byte_in();
            }

            self.a <<= 1;

            // Shift combined C register
            self.c <<= 1; //(self.c << 1) & 0xFFFF_FFFF;
            self.ct -= 1;

            if (self.a & 0x8000) == 0 {
                continue;
            } else {
                break;
            }
        }
    }

    /// BYTEIN - Read a byte of compressed data
    fn byte_in(&mut self) {
        if self.bp >= self.buffer.len() {
            // End of data - feed 1s (0xFF in bits 15-8)
            self.c += 0xFF00;
            self.ct = 8;
            return;
        }

        let b: u32 = self.buffer[self.bp].into();

        if b == 0xFF {
            if self.bp + 1 < self.buffer.len() {
                let b1 = self.buffer[self.bp + 1];
                if b1 > 0x8F {
                    // Marker code detected - feed 1s
                    self.c += 0xFF00;
                    self.ct = 8;
                } else {
                    // Stuffed bit after 0xFF - increment BP and read 0xFF
                    self.bp += 1;
                    self.c += b << 9; //0xFF00;
                    self.ct = 7;
                }
            } else {
                self.c += 0xFF00;
                self.ct = 8;
            }
        } else {
            // Normal byte - insert into bits 15-8 of C_low
            self.bp += 1;
            self.c += b << 8;
            self.ct = 8;
        }
    }
}

impl Decoder for MqDecoder {
    fn decode_bit(&mut self, cx: usize) -> u8 {
        self.decode(cx)
    }
}

#[cfg(test)]
mod tests {
    use std::iter::zip;

    use super::*;

    #[test]
    fn test_qe_table_integrity() {
        // Verify table has correct number of entries
        assert_eq!(QE_TABLE.len(), 47);

        // Check some known values from Table C.2
        assert_eq!(QE_TABLE[0].qe, 0x5601);
        assert_eq!(QE_TABLE[0].nmps, 1);
        assert_eq!(QE_TABLE[0].nlps, 1);
        assert_eq!(QE_TABLE[0].switch, true);

        assert_eq!(QE_TABLE[22].qe, 0x2401);
        assert_eq!(QE_TABLE[22].nmps, 23);
        assert_eq!(QE_TABLE[22].nlps, 20);
        assert_eq!(QE_TABLE[22].switch, false);

        assert_eq!(QE_TABLE[46].qe, 0x5601);
        assert_eq!(QE_TABLE[46].nmps, 46);
        assert_eq!(QE_TABLE[46].nlps, 46);
        assert_eq!(QE_TABLE[46].switch, false);
    }

    #[test]
    fn test_encode_decode_simple() {
        // Test encoding and decoding simple bit sequences
        let num_contexts = 1;
        let mut encoder = MqEncoder::new(num_contexts);
        encoder.init();

        // Encode a simple sequence
        let bits = vec![0, 1, 0, 1, 1, 0, 0, 1];
        for &bit in &bits {
            encoder.encode(0, bit);
        }

        let compressed = encoder.flush();
        println!(
            "Compressed {} bits to {} bytes",
            bits.len(),
            compressed.len()
        );

        // Decode
        let mut decoder = MqDecoder::new(num_contexts);
        decoder.init(&compressed);

        let mut decoded = Vec::new();
        for _ in 0..bits.len() {
            decoded.push(decoder.decode(0));
        }

        assert_eq!(bits, decoded, "Decoded bits should match original");
    }

    #[test]
    fn test_encode_decode_all_zeros() {
        let num_contexts = 1;
        let mut encoder = MqEncoder::new(num_contexts);
        encoder.init();

        // Encode all zeros
        let bits = vec![0; 100];
        for &bit in &bits {
            encoder.encode(0, bit);
        }

        let compressed = encoder.flush();
        println!("Compressed 100 zeros to {} bytes", compressed.len());

        // Decode
        let mut decoder = MqDecoder::new(num_contexts);
        decoder.init(&compressed);

        let mut decoded = Vec::new();
        for _ in 0..bits.len() {
            decoded.push(decoder.decode(0));
        }

        assert_eq!(bits, decoded);
    }

    #[test]
    fn test_encode_decode_all_ones() {
        let num_contexts = 1;
        let mut encoder = MqEncoder::new(num_contexts);
        encoder.init();

        // Encode all ones
        let bits = vec![1; 100];
        for &bit in &bits {
            encoder.encode(0, bit);
        }

        let compressed = encoder.flush();
        println!("Compressed 100 ones to {} bytes", compressed.len());

        // Decode
        let mut decoder = MqDecoder::new(num_contexts);
        decoder.init(&compressed);

        let mut decoded = Vec::new();
        for _ in 0..bits.len() {
            decoded.push(decoder.decode(0));
        }

        assert_eq!(bits, decoded);
    }

    #[test]
    fn test_encode_decode_alternating() {
        let num_contexts = 1;
        let mut encoder = MqEncoder::new(num_contexts);
        encoder.init();

        // Encode alternating pattern
        let mut bits = Vec::new();
        for i in 0..50 {
            bits.push(i % 2);
        }

        for &bit in &bits {
            encoder.encode(0, bit);
        }

        let compressed = encoder.flush();
        println!(
            "Compressed 50 alternating bits to {} bytes",
            compressed.len()
        );

        // Decode
        let mut decoder = MqDecoder::new(num_contexts);
        decoder.init(&compressed);

        let mut decoded = Vec::new();
        for _ in 0..bits.len() {
            decoded.push(decoder.decode(0));
        }

        assert_eq!(bits, decoded);
    }

    #[test]
    fn test_multiple_contexts() {
        // Test with multiple independent contexts
        let num_contexts = 4;
        let mut encoder = MqEncoder::new(num_contexts);
        encoder.init();

        // Encode different patterns in different contexts
        let sequences = [
            vec![0, 0, 0, 1, 0, 0, 0, 1], // Context 0: mostly zeros
            vec![1, 1, 1, 0, 1, 1, 1, 0], // Context 1: mostly ones
            vec![0, 1, 0, 1, 0, 1, 0, 1], // Context 2: alternating
            vec![0, 0, 1, 1, 0, 0, 1, 1], // Context 3: pairs
        ];

        // Interleave encoding from different contexts
        for i in 0..8 {
            for (cx, seq) in sequences.iter().enumerate() {
                encoder.encode(cx, seq[i]);
            }
        }

        let compressed = encoder.flush();
        println!(
            "Compressed {} bits with 4 contexts to {} bytes",
            sequences.len() * 8,
            compressed.len()
        );

        // Decode
        let mut decoder = MqDecoder::new(num_contexts);
        decoder.init(&compressed);

        let mut decoded_sequences = vec![Vec::new(); 4];
        for _ in 0..8 {
            for cx in 0..4 {
                decoded_sequences[cx].push(decoder.decode(cx));
            }
        }

        for (cx, (original, decoded)) in sequences.iter().zip(decoded_sequences.iter()).enumerate()
        {
            assert_eq!(original, decoded, "Context {} mismatch", cx);
        }
    }

    #[test]
    fn test_random_data() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        // Generate pseudo-random data for testing
        let num_contexts = 2;
        let mut encoder = MqEncoder::new(num_contexts);
        encoder.init();

        let mut bits = Vec::new();
        let mut hasher = DefaultHasher::new();

        for i in 0..200 {
            i.hash(&mut hasher);
            let hash = hasher.finish();
            bits.push((hash & 1) as u8);

            // Use hash to select context too
            let cx = ((hash >> 1) & 1) as usize;
            encoder.encode(cx, bits[i]);
        }

        let compressed = encoder.flush();
        println!(
            "Compressed 200 pseudo-random bits to {} bytes",
            compressed.len()
        );

        // Decode
        let mut decoder = MqDecoder::new(num_contexts);
        decoder.init(&compressed);

        let mut decoded = Vec::new();
        let mut hasher2 = DefaultHasher::new();
        for i in 0..200 {
            // usize is important because previous loop used usize.
            // different types => different hash
            (i as usize).hash(&mut hasher2);
            let hash = hasher2.finish();
            let cx = ((hash >> 1) & 1) as usize;
            decoded.push(decoder.decode(cx));
        }

        assert_eq!(bits, decoded);
    }

    #[test]
    fn test_compression_efficiency() {
        // Test that highly biased data compresses well
        let num_contexts = 1;
        let mut encoder = MqEncoder::new(num_contexts);
        encoder.init();

        // 95% zeros, 5% ones
        let mut bits = Vec::new();
        for i in 0..200 {
            bits.push(if i % 20 == 0 { 1 } else { 0 });
        }

        for &bit in &bits {
            encoder.encode(0, bit);
        }

        let compressed = encoder.flush();

        // Should achieve good compression (well below 200 bits = 25 bytes)
        println!(
            "Compressed 200 biased bits to {} bytes ({}% of original)",
            compressed.len(),
            (compressed.len() * 100) / 25
        );
        assert!(
            compressed.len() < 15,
            "Highly biased data should compress well"
        );
    }

    #[test]
    fn test_empty_sequence() {
        let num_contexts = 1;
        let mut encoder = MqEncoder::new(num_contexts);
        encoder.init();

        let compressed = encoder.flush();
        println!("Empty sequence compressed to {} bytes", compressed.len());

        // Should still be able to decode (returns whatever the MPS is)
        let mut decoder = MqDecoder::new(num_contexts);
        decoder.init(&compressed);

        // Decoding from empty data should not crash
        let _ = decoder.decode(0);
    }

    #[test]
    fn test_encode_j10() {
        // See ITU T.800 (V4) | ISO/IEC 15444-1:2024 Section J.10.4, Table J.22
        let j10_4 = b"\x01\x8F\x0D\xC8\x75\x5D";
        let mut encoder = MqEncoder::new(19);
        encoder.reset_contexts();
        encoder.init();

        let context_indexes = vec![
            17, 18, 18, 9, 3, 3, 10, 3, 10, 15, 0, 9, 4, 10, 15, 15, 15, 16, 15, 16, 16, 16, 16,
            16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16,
        ];

        let given_bits = [
            1, 1, 1, 1, 0, 1, 0, 1, 0, 0, 1, 1, 1, 0, 1, 0, 1, 0, 0, 0, 1, 1, 0, 0, 1, 1, 1, 0, 1,
            0, 0, 0, 0, 1,
        ];
        println!(
            "Length of ctx {} and exp {}",
            context_indexes.len(),
            given_bits.len()
        );
        for (idx, bit) in zip(context_indexes.iter(), given_bits.iter()) {
            let ctx = *idx;
            let bit = *bit;
            encoder.encode(ctx, bit);
        }
        // These encodes make the output match what was given. The output without the extra
        // encodes, still decodes properly. There are multiple valid outputs for a given input.
        encoder.encode(0, 1);
        encoder.encode(0, 1);
        let encoded = encoder.flush();
        println!("encoded : {:02X?}", encoded);
        println!("expected: {:02X?}", j10_4);
        assert_eq!(encoded, j10_4);
    }

    #[test]
    fn test_decode_j10() {
        let j10_4 = b"\x01\x8F\x0D\xC8\x75\x5D";
        let mut decoder = MqDecoder::new(19);
        decoder.reset_contexts();
        decoder.init(j10_4);

        let context_indexes = vec![
            17, 18, 18, 9, 3, 3, 10, 3, 10, 15, 0, 9, 4, 10, 15, 15, 15, 16, 15, 16, 16, 16, 16,
            16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16,
        ];
        let exp_bits = vec![
            1, 1, 1, 1, 0, 1, 0, 1, 0, 0, 1, 1, 1, 0, 1, 0, 1, 0, 0, 0, 1, 1, 0, 0, 1, 1, 1, 0, 1,
            0, 0, 0, 0, 1,
        ];

        let mut decoded = Vec::new();
        println!(
            "Length of ctx {} and exp {}",
            context_indexes.len(),
            exp_bits.len()
        );
        for idx in context_indexes.iter() {
            let ctx = *idx;
            let dc = decoder.decode(ctx);
            println!("working from ctx {} -> {}", ctx, dc);
            decoded.push(dc);
        }
        assert_eq!(exp_bits, decoded);
    }

    #[test]
    fn test_encode_j10_2() {
        // See ITU T.800 (V4) | ISO/IEC 15444-1:2024 Section J.10.4, Table J.23
        let j10_4 = b"\x0F\xB1\x76";
        let mut encoder = MqEncoder::new(19);
        encoder.reset_contexts();
        encoder.init();

        let context_indexes = [17, 18, 18, 9, 3, 0, 3, 3, 14, 0, 3, 10, 3, 10, 3, 16];
        let given_bits = [1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 0, 0, 1];
        println!(
            "Length of ctx {} and exp {}",
            context_indexes.len(),
            given_bits.len()
        );
        for (idx, bit) in zip(context_indexes.iter(), given_bits.iter()) {
            let ctx = *idx;
            let bit = *bit;
            encoder.encode(ctx, bit);
        }
        // These encodes make the output match what was given. The output without the extra
        // encodes, still decodes properly. There are multiple valid outputs for a given input.
        encoder.encode(0, 1);
        encoder.encode(0, 0);
        encoder.encode(0, 1);
        let encoded = encoder.flush();
        println!("encoded : {:02X?}", encoded);
        println!("expected: {:02X?}", j10_4);
        assert_eq!(encoded, j10_4);
    }

    #[test]
    fn test_decode_j10_2() {
        let exp_bits = vec![1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 0, 0, 1];
        let j10_4 = b"\x0F\xB1\x76";
        let mut decoder = MqDecoder::new(19);
        decoder.reset_contexts();
        decoder.init(j10_4);
        let context_indexes = [17, 18, 18, 9, 3, 0, 3, 3, 14, 0, 3, 10, 3, 10, 3, 16];
        let mut decoded = Vec::new();
        for ctx in context_indexes.iter() {
            decoded.push(decoder.decode(*ctx));
        }
        assert_eq!(exp_bits, decoded);
    }
}
