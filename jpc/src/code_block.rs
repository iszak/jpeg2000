use log::{debug, info};

use crate::coder::{Decoder, RUN_LEN, UNIFORM};
use crate::shared::SubBandType;

#[derive(Debug, Clone)]
enum Coeff {
    // TODO i16 is probably wrong, might need generic
    Significant { value: i16, is_negative: bool },
    Insignificant(u8), // Insignificant at what bit-plane shift
}

impl Coeff {
    /// contribution to sign context -> -1, 0, 1
    ///
    /// ITU-T T.800(V4) | ISO/IEC 15444-1:2024 Table D.2
    fn sign_contribution(&self) -> i8 {
        match self {
            Coeff::Insignificant(_) => 0,
            Coeff::Significant { is_negative, .. } => match is_negative {
                true => -1,
                false => 1,
            },
        }
    }
}

struct CodeBlockDecodeError {}

/// decoder for codeblocks
///
/// A CodeBlockDecoder produces coefficients from compressed data.
struct CodeBlockDecoder {
    width: i32,
    height: i32,
    subband: SubBandType,
    no_passes: u8, // Max 164 from table B.4
    bit_plane_shift: u8,
    coefficients: Vec<Coeff>,
}

/// Wrapper around an x, y coord
#[derive(Debug, Clone, Copy)]
struct CoeffIndex {
    y: i32,
    x: i32,
}

impl CodeBlockDecoder {
    fn new(width: i32, height: i32, subband: SubBandType, no_passes: u8, mb: u8) -> Self {
        Self {
            width,
            height,
            subband,
            no_passes,
            bit_plane_shift: mb - 1,
            coefficients: vec![Coeff::Insignificant(u8::MAX); (width * height) as usize],
        }
    }

    /// Decode coefficients from the given compressed data.
    fn decode(&mut self, coder: &mut dyn Decoder) -> Result<(), CodeBlockDecodeError> {
        info!("Decoding code block for subband {:?}", self.subband);

        // Start in CleanUp -> SignificancePropagation -> MagnitudeRefinement -> repeat ...
        self.pass_cleanup(coder);
        for _ in (1..self.no_passes).step_by(3) {
            debug!("Beginning a pass set");
            self.bit_plane_shift -= 1;
            self.pass_significance(coder);
            self.pass_refinement(coder);
            self.pass_cleanup(coder);
            debug!("coefficients: {:?}", self.coefficients);
        }
        Ok(())
    }
    /// Return coefficients
    /// TODO return type is whak
    /// Note, return a copy, maybe need to decode more for this codeblock later and don't want to
    /// lose state
    fn coefficients(&self) -> Vec<i32> {
        self.coefficients
            .iter()
            .map(|c| match c {
                Coeff::Significant { value, is_negative } => {
                    if *is_negative {
                        -1 * value
                    } else {
                        *value
                    }
                }
                Coeff::Insignificant(_) => 0,
            } as i32)
            .collect()
    }

    /// Handle a cleanup pass
    ///
    /// Cleanup does cleanup and sign coding.
    /// See ITU-T T.800(V4) | ISO/IEC 15444-1:2024 Section D.3.4
    fn pass_cleanup(&mut self, coder: &mut dyn Decoder) {
        // Iterate coefficients in strips 4 tall across full width
        for by in (0..self.height).step_by(4) {
            for x in 0..self.width {
                let mut offset_y: i32 = 0;

                // Count insignificants in this column strip
                let mut count_insig = 0;
                for y in by..(by + 4).min(self.height) {
                    count_insig += (!self.is_significant(CoeffIndex { y, x })) as i32;
                }

                // Decision D8: Are four contiguous undecoded coefficients in a column each with a 0 context?
                let d8 = 4 == count_insig;
                if d8 {
                    // All Insignificant, determine first significant
                    let c4 = coder.decode_bit(RUN_LEN);
                    // c4 -> d11
                    if c4 != 1 {
                        // skip all, go to next column of 4
                        debug!("Skipping column of 4");
                        continue;
                    } else {
                        // Decode how many coeffs to skip
                        // two uniform context decodes
                        let a = coder.decode_bit(UNIFORM);
                        let b = coder.decode_bit(UNIFORM);
                        let c5 = 2 * a + b;
                        assert!(c5 < 4, "Improper decode from mq coder");

                        // go forward s
                        offset_y += c5 as i32;
                        debug!("Skip {} coeffs", c5);
                    }
                    let nsi = CoeffIndex {
                        x,
                        y: by + offset_y,
                    };
                    self.make_significant(nsi);

                    // C2 decode sign bit
                    self.decode_sign_bit(nsi, coder);
                    offset_y += 1;
                }

                // remaining coefficients in this column strip
                for y in (by + offset_y)..(by + 4).min(self.height) {
                    let idx = CoeffIndex { x, y };
                    let newly_sig =
                        !self.is_significant(idx) && self.significance_decode(idx, coder);
                    if newly_sig {
                        // C2 decode sign bit
                        self.decode_sign_bit(idx, coder);
                    }
                }
            }
        }
        info!("completed cleanup pass");
    }

    /// Handle a significance propagation pass
    fn pass_significance(&mut self, coder: &mut dyn Decoder) {
        // Iterate coefficients in strips 4 tall across full width
        for by in (0..self.height).step_by(4) {
            for x in 0..self.width {
                for y in by..(by + 4).min(self.height) {
                    let idx = CoeffIndex { y, x };
                    if self.is_significant(idx) {
                        continue; // D1 yes
                    }
                    let sig_ctx = self.significance_context(idx);
                    if 0 == sig_ctx {
                        continue; // D2 yes
                    }
                    let newly_sig = self.significance_decode_ctx(sig_ctx, idx, coder);
                    if newly_sig {
                        // C2
                        self.decode_sign_bit(idx, coder);
                    } else {
                        *self.coeff_at_mut(idx) = Coeff::Insignificant(self.bit_plane_shift);
                    }
                }
            }
        }
        info!("completed significance pass");
    }

    /// Handle a magnitude refinement pass
    fn pass_refinement(&mut self, coder: &mut dyn Decoder) {
        // Iterate coefficients in strips 4 tall across full width
        for by in (0..self.height).step_by(4) {
            for x in 0..self.width {
                for y in by..(by + 4).min(self.height) {
                    let idx = CoeffIndex { y, x };
                    if !self.is_significant(idx) {
                        continue; // D5 yes
                    }
                    // is bit set for this bit-plane
                    let is_bit_set = self.is_bit_plane_set(idx);
                    debug!("Is bit set: {}, for {:?}", is_bit_set, idx);
                    if is_bit_set {
                        continue; // D6 yes
                    }
                    // C3
                    self.magnitude_decode(idx, coder);
                }
            }
        }
        info!("completed refinement pass");
    }

    fn coeff_at(&self, idx: CoeffIndex) -> &Coeff {
        let CoeffIndex { x, y } = idx;
        let out_bounds = x < 0 || x >= self.width || y < 0 || y >= self.height;
        if out_bounds {
            debug!("Out of bounds coeff_at {}, {}", x, y);
            &Coeff::Insignificant(u8::MAX)
        } else {
            &self.coefficients[(self.width * idx.y + idx.x) as usize]
        }
    }

    fn coeff_at_mut(&mut self, idx: CoeffIndex) -> &mut Coeff {
        let CoeffIndex { x, y } = idx;
        let out_bounds = x < 0 || x >= self.width || y < 0 || y >= self.height;
        assert!(!out_bounds, "Should not be trying to mutate out of bounds");
        &mut self.coefficients[(self.width * idx.y + idx.x) as usize]
    }

    fn significance_context(&self, idx: CoeffIndex) -> usize {
        let CoeffIndex { x, y } = idx;
        // mutables
        let mut h = 0; // horizontal contributions
        let mut v = 0; // vertical contributions
        let mut d = 0; // diagonal contributions

        // Count significant neighbors
        h += self.is_significant(CoeffIndex { y, x: x - 1 }) as u8;
        h += self.is_significant(CoeffIndex { y, x: x + 1 }) as u8;
        v += self.is_significant(CoeffIndex { y: y - 1, x }) as u8;
        v += self.is_significant(CoeffIndex { y: y + 1, x }) as u8;

        // Diagonals (only if both adjacent orthogonal are insignificant)
        d += self.is_significant(CoeffIndex { y: y - 1, x: x - 1 }) as u8;
        d += self.is_significant(CoeffIndex { y: y - 1, x: x + 1 }) as u8;
        d += self.is_significant(CoeffIndex { y: y + 1, x: x - 1 }) as u8;
        d += self.is_significant(CoeffIndex { y: y + 1, x: x + 1 }) as u8;

        debug!(
            "For subband {:?}, idx: {:?}, found h={}, v={}, d={}",
            self.subband, idx, h, v, d
        );

        // Compute context based on subband and neighbor counts
        // Different formulas for LL / LH (vertical high pass), HL (horizontal high pass), HH (diagonal high pass) subbands
        // ITU-T T.800 | ISO/IEC 15444-1 Table D.1
        match self.subband {
            SubBandType::LL | SubBandType::LH => match (h, v, d) {
                (0, 0, 0) => 0,
                (0, 0, 1) => 1,
                (0, 0, _) => 2,
                (0, 1, _) => 3,
                (0, 2, _) => 4,
                (1, 0, 0) => 5,
                (1, 0, _) => 6,
                (1, _, _) => 7,
                (2, _, _) => 8,
                (_, _, _) => panic!("Unknown significance context calculation"),
            },
            SubBandType::HL => match (h, v, d) {
                (0, 0, 0) => 0,
                (0, 0, 1) => 1,
                (0, 0, _) => 2,
                (1, 0, _) => 3,
                (2, 0, _) => 4,
                (0, 1, 0) => 5,
                (0, 1, _) => 6,
                (_, 1, _) => 7,
                (_, 2, _) => 8,
                (_, _, _) => panic!("Unknown significance context calculation"),
            },
            SubBandType::HH => match (h + v, d) {
                (0, 0) => 0,
                (1, 0) => 1,
                (a, 0) if a >= 2 => 2,
                (0, 1) => 3,
                (1, 1) => 4,
                (a, 1) if a >= 2 => 5,
                (0, 2) => 6,
                (a, 2) if a >= 1 => 7,
                (_, b) if b >= 3 => 8,
                (_, _) => panic!("Unknown significance context calculation"),
            },
        }
    }

    /// Checks if the bit in this bit-plane was set
    fn is_bit_plane_set(&self, idx: CoeffIndex) -> bool {
        match self.coeff_at(idx) {
            Coeff::Insignificant(_) => {
                panic!("Attemping to check bit-plane of Insignificant coefficient")
            }
            Coeff::Significant { value, .. } => 1 == (0x1 & (value >> self.bit_plane_shift)),
        }
    }

    fn is_significant(&self, idx: CoeffIndex) -> bool {
        let CoeffIndex { x, y } = idx;
        let out_bounds = x < 0 || x >= self.width || y < 0 || y >= self.height;
        if out_bounds {
            return false;
        }
        match self.coeff_at(idx) {
            Coeff::Insignificant(_) => false,
            Coeff::Significant { .. } => true,
        }
    }

    /// Turn a coefficient significant
    fn make_significant(&mut self, idx: CoeffIndex) {
        debug!("Marking significant {:?}", idx);
        match self.coeff_at(idx) {
            Coeff::Insignificant(_) => {
                *self.coeff_at_mut(idx) = Coeff::Significant {
                    value: 1 << self.bit_plane_shift,
                    is_negative: false,
                };
            }
            _ => panic!("tried to make a coefficient doubly significant"),
        }
    }

    /// Decode the significance for a specific CoeffIndex from the decoder
    fn significance_decode(&mut self, idx: CoeffIndex, decoder: &mut dyn Decoder) -> bool {
        if let Coeff::Insignificant(bs) = self.coeff_at(idx) {
            if *bs == self.bit_plane_shift {
                return false;
            }
        } else {
            panic!("Should have checked if sig");
        }
        let cx = self.significance_context(idx);
        self.significance_decode_ctx(cx, idx, decoder)
    }

    /// Decode the significance with a known context
    fn significance_decode_ctx(
        &mut self,
        cx: usize,
        idx: CoeffIndex,
        decoder: &mut dyn Decoder,
    ) -> bool {
        let sig = decoder.decode_bit(cx);
        debug!("significance {sig} for {idx:?}");
        if sig == 1 {
            self.make_significant(idx);
            true
        } else {
            false
        }
    }

    /// Decode the magnitude bit for a specific CoeffIndex from the decoder
    fn magnitude_decode(&mut self, idx: CoeffIndex, decoder: &mut dyn Decoder) {
        let cx = self.magnitude_context(idx);
        let b = decoder.decode_bit(cx);
        *self.coeff_at_mut(idx) = match self.coeff_at(idx) {
            Coeff::Insignificant(_) => {
                panic!("Cannot set magnitude bit for an Insignificant coefficient")
            }
            Coeff::Significant { value, is_negative } => {
                let value = value | (b << self.bit_plane_shift) as i16;
                let is_negative = *is_negative;
                Coeff::Significant { value, is_negative }
            }
        };
        debug!("Set bit {} for {:?}", b, idx);
    }

    /// Decode the sign bit for a specific CoeffIndex from the decoder
    fn decode_sign_bit(&mut self, idx: CoeffIndex, decoder: &mut dyn Decoder) {
        let (cx, xor) = self.sign_context(idx);
        let sign_bit = decoder.decode_bit(cx);
        if let Coeff::Significant { value, .. } = self.coeff_at(idx) {
            *self.coeff_at_mut(idx) = Coeff::Significant {
                value: *value,
                is_negative: (sign_bit ^ xor) != 0,
            };
        } else {
            panic!("Cannot set sign bit on coeff");
        }
    }

    fn num_zero_bit_plane(&mut self, arg: u8) {
        self.bit_plane_shift -= arg;
    }

    /// Determine the context for sign bit decoding
    ///
    /// ITU-T T.800(V4) | ISO/IEC 15444-1:2024 section D.3.2
    fn sign_context(&self, idx: CoeffIndex) -> (usize, u8) {
        let CoeffIndex { x, y } = idx;

        let v0 = self.coeff_at(CoeffIndex { y: y - 1, x });
        let v1 = self.coeff_at(CoeffIndex { y: y + 1, x });
        let h0 = self.coeff_at(CoeffIndex { y, x: x - 1 });
        let h1 = self.coeff_at(CoeffIndex { y, x: x + 1 });

        debug!("v0 {v0:?} v1 {v1:?} h0 {v1:?} h1 {h1:?}");

        /// Add up the contribution to a -1,0,1
        fn contribution(a: &Coeff, b: &Coeff) -> i8 {
            let total = a.sign_contribution() + b.sign_contribution();
            match total {
                1 | 2 => 1,
                0 => 0,
                -1 | -2 => -1,
                _ => panic!("Total should be in range -2..=2"),
            }
        }
        debug!(
            "sign context vert {}, {}",
            v0.sign_contribution(),
            v1.sign_contribution()
        );
        debug!(
            "sign context horz {}, {}",
            h0.sign_contribution(),
            h1.sign_contribution()
        );

        let vc = contribution(v0, v1);
        let hc = contribution(h0, h1);
        // ITU-T T.800(V4) | ISO/IEC 15444-1:2024 Table D.3
        let (ctx, xor) = match (hc, vc) {
            (1, 1) => (13, 0),
            (1, 0) => (12, 0),
            (1, -1) => (11, 0),
            (0, 1) => (10, 0),
            (0, 0) => (9, 0),
            (0, -1) => (10, 1),
            (-1, 1) => (11, 1),
            (-1, 0) => (12, 1),
            (-1, -1) => (13, 1),
            (_, _) => panic!("Invalid context values for sign_context"),
        };
        (ctx, xor)
    }

    fn magnitude_context(&self, idx: CoeffIndex) -> usize {
        if let Coeff::Significant { value, .. } = self.coeff_at(idx) {
            let c = value.count_ones();
            let sv = value >> (1 + self.bit_plane_shift);
            if sv != 1 {
                debug!("First refinement for idx {:?} w/ {}, c {}", idx, value, c);
                return 16;
            }
        }
        let CoeffIndex { x, y } = idx;
        let h0 = self.is_significant(CoeffIndex { y, x: x - 1 }) as u8;
        let h1 = self.is_significant(CoeffIndex { y, x: x + 1 }) as u8;
        let v0 = self.is_significant(CoeffIndex { y: y - 1, x }) as u8;
        let v1 = self.is_significant(CoeffIndex { y: y + 1, x }) as u8;

        let c = v0 + v1 + h0 + h1;
        if c > 0 {
            // early return if we know w/o diagonals
            return 15;
        }

        let mut dc = 0u8;
        // Diagonals (only if both adjacent orthogonal are insignificant)
        dc += self.is_significant(CoeffIndex { y: y - 1, x: x - 1 }) as u8;
        dc += self.is_significant(CoeffIndex { y: y - 1, x: x + 1 }) as u8;
        dc += self.is_significant(CoeffIndex { y: y + 1, x: x - 1 }) as u8;
        dc += self.is_significant(CoeffIndex { y: y + 1, x: x + 1 }) as u8;
        if dc + c > 0 {
            15
        } else {
            14
        }
    }
}

/// ColumnIndex type to help avoid indexing mistakes
#[derive(Debug)]
struct ColumnIndex {
    pub base_y: i32,
    pub x: i32,
}

// Decoder State
#[derive(Debug, Default)]
enum State {
    SignificancePropagation,
    #[default]
    CleanUp,
    MagnitudeRefinement,
}

#[cfg(test)]
mod tests {
    use crate::coder::{standard_decoder, Decoder};

    use super::*;

    pub fn init_logger() {
        let _ = env_logger::builder()
            .is_test(true)
            .filter_level(log::LevelFilter::Debug)
            .try_init();
    }

    struct MockCoder {
        exp: Vec<(usize, u8)>,
        index: usize,
    }

    impl Decoder for MockCoder {
        fn decode_bit(&mut self, cx: usize) -> u8 {
            let (exp_cx, out) = self.exp[self.index];
            self.index += 1;
            assert_eq!(exp_cx, cx, "incorrect cx during decode");
            out
        }
    }

    /// Test decoding the codeblock from J.10 for LL using a mock mqcoder
    #[test]
    fn test_cb_decode_j10a_mocked() {
        init_logger();

        // Mock decoder that checks input contexts
        let mut coder = MockCoder {
            exp: vec![
                (17, 1),
                (18, 1),
                (18, 1),
                (9, 1),
                (3, 0),
                (3, 1),
                (10, 0),
                (3, 1),
                (10, 0),
                (15, 0),
                (0, 1),
                (9, 1),
                (4, 1),
                (10, 0),
                // Refinement phase
                (15, 1),
                (15, 0),
                (15, 1),
                (16, 0),
                (15, 0),
                // next bit-plane
                (16, 0),
                (16, 1),
                (16, 1),
                (16, 0),
                (16, 0),
                // next bit-plane
                (16, 1),
                (16, 1),
                (16, 1),
                (16, 0),
                (16, 1),
                // last bit-plane
                (16, 0),
                (16, 0),
                (16, 0),
                (16, 0),
                (16, 1),
            ],
            index: 0,
        };
        // There are 16 coding passes in this example
        let mut codeblock = CodeBlockDecoder::new(1, 5, SubBandType::LL, 16, 9);
        // codeblock.mb(9);
        codeblock.num_zero_bit_plane(3);
        // 9 - 3 = 6 bits to set
        // 6-1 = 5 => 1+5*3 = 16 coding passes

        assert!(
            codeblock.decode(&mut coder).is_ok(),
            "Expected decode to work"
        );
        assert_eq!(
            coder.exp.len(),
            coder.index,
            "Expected all mock data to be used"
        );

        let coeffs = codeblock.coefficients();
        let exp_coeffs = vec![-26, -22, -30, -32, -19];
        assert_eq!(coeffs, exp_coeffs, "Coefficients didn't match");
    }

    /// Test decoding the codeblock from J.10 for LL
    #[test]
    fn test_cb_decode_j10a() {
        init_logger();
        let bd = b"\x01\x8F\x0D\xC8\x75\x5D";
        let mut coder = standard_decoder(bd);

        // There are 16 coding passes in this example
        let mut codeblock = CodeBlockDecoder::new(1, 5, SubBandType::LL, 16, 9);
        codeblock.num_zero_bit_plane(3);
        // 9 - 3 = 6 bits to set
        // 6-1 = 5 => 1+5*3 = 16 coding passes

        assert!(
            codeblock.decode(&mut coder).is_ok(),
            "Expected decode to work"
        );

        let coeffs = codeblock.coefficients();
        let exp_coeffs = vec![-26, -22, -30, -32, -19];
        assert_eq!(coeffs, exp_coeffs, "Coefficients didn't match");
    }

    /// Test decoding the codeblock from J.10 for LH using a mock mqcoder
    #[test]
    fn test_cb_decode_j10b_mocked() {
        init_logger();

        // Mock decoder that checks input contexts
        let mut coder = MockCoder {
            exp: vec![
                (17, 1),
                (18, 0),
                (18, 1),
                (9, 0),
                (3, 0),
                (0, 0),
                (3, 0),
                (3, 0),
                (14, 0),
                (0, 0),
                (3, 1),
                (10, 0),
                (3, 1),
                (10, 0),
                (3, 0),
                (16, 1),
            ],
            index: 0,
        };
        // There are 7 coding passes in this example
        let mut codeblock = CodeBlockDecoder::new(1, 4, SubBandType::LH, 7, 10);
        // codeblock.mb(10);
        codeblock.num_zero_bit_plane(7);
        // 10 - 7 = 3 bits to set
        // 3 bits to set => 7 (=1cleanup+2bitplanes*3) coding passes

        assert!(
            codeblock.decode(&mut coder).is_ok(),
            "Expected decode to work"
        );
        assert_eq!(
            coder.exp.len(),
            coder.index,
            "Expected all mock data to be used"
        );

        let coeffs = codeblock.coefficients();
        let exp_coeffs = vec![1, 5, 1, 0];
        assert_eq!(coeffs, exp_coeffs, "Coefficients didn't match");
    }

    #[test]
    fn test_cb_decode_j10b() {
        init_logger();
        // Test decoding the codeblock from J.10 for LH
        let bd = b"\x0F\xB1\x76";
        let mut coder = standard_decoder(bd);

        let mut codeblock = CodeBlockDecoder::new(1, 4, SubBandType::LH, 7, 10);
        codeblock.num_zero_bit_plane(7);

        assert!(
            codeblock.decode(&mut coder).is_ok(),
            "Expected decode to work"
        );

        let coeffs = codeblock.coefficients();
        let exp_coeffs = vec![1, 5, 1, 0];
        assert_eq!(coeffs, exp_coeffs, "Coefficients didn't match");
    }
}
