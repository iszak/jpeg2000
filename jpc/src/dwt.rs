//! Discrete Wavelet Transformation for JPEG 2000
//!
//! This module implements the forward and inverse discrete wavelet transformations
//! as specified in Annex F of ITU-T T.800 (ISO/IEC 15444-1) - JPEG 2000 Core Coding System.
//!
//! The implementation supports:
//! - 5-3 Reversible (lossless) wavelet transformation
//! - 9-7 Irreversible (lossy) wavelet transformation
//!
//! Both transformations use lifting-based filtering as specified in the standard.

use std::{cmp::min, ops::Index};

use num_traits::{Euclid, FromPrimitive, Num};

use crate::shared::{Array2D, SubBandGroup};

/// Filter type selection for DWT operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterType {
    /// 5-3 Reversible filter for lossless compression
    Reversible53,
    /// 9-7 Irreversible filter for lossy compression
    Irreversible97,
}

impl FilterType {
    /// Perform an inverse discrete wavelet transform
    pub fn idwt<T: ValidNumeric>(
        &self,
        ll: Array2D<T>,
        groups: Vec<SubBandGroup<Array2D<T>>>,
    ) -> Array2D<T> {
        match self {
            FilterType::Reversible53 => Reversible53::idwt(ll, groups),
            FilterType::Irreversible97 => {
                todo!("Not implemented");
                //Irreversible97::idwt(ll, groups)
            }
        }
    }
}

/// A filter can perform forward and inverse discrete wavelet transfoms
pub trait Filter<T> {
    fn idwt(ll: Array2D<T>, groups: Vec<SubBandGroup<Array2D<T>>>) -> Array2D<T>;
    //fn fdwt(a: Array2D<T>) -> (Array2D<T>, Vec<SubBandGroup<Array2D<T>>);
}

/// The reversible 5-3 wavelet transform
struct Reversible53;

/// The irreversible 9-7 wavelet transform
struct Irreversible97;

/// An view of a one-dimensional signal with a periodic symmetric extension.
///
/// See also F.3.7 1D_EXTR procedure
struct ExtendedSignal<T>(Vec<T>);

impl<T> Index<i64> for ExtendedSignal<T> {
    type Output = T;

    fn index(&self, index: i64) -> &Self::Output {
        let width = self.0.len() as i64;

        if index >= width || index < 0 {
            let pse_len = 2 * (width - 1);
            let pse_index = min(
                index.rem_euclid(pse_len),
                pse_len - (index.rem_euclid(pse_len)),
            );
            assert!(pse_index >= 0 && pse_index < width);
            self.0.index(pse_index as usize)
        } else {
            assert!(index >= 0);
            let index = index as usize;
            assert!(index < self.0.len());
            self.0.index(index)
        }
    }
}

/// These are the default implementations shared between the 53 and 97 filter types.
trait InternalFilter<T: ValidNumeric>: Filter<T> {
    /// 1D_FILTR needs to be implemented for each filter type
    fn m_1d_filt_r(y: ExtendedSignal<T>) -> Vec<T>;

    /// 1D_FILTD needs to be implemented for each filter type
    fn m_1d_filt_d(_: ExtendedSignal<T>) -> Vec<T> {
        todo!("Not impl")
    }

    /// 2D_SR two-dimensional signal reconstruction
    ///
    /// See also F.3.2
    fn m_2d_sr(group: SubBandGroup<&Array2D<T>>) -> Array2D<T> {
        let a = interleave(group);
        let a = Self::horizontal_sr(a);
        Self::vertical_sr(a)
    }

    /// VER_SR vertical signal reconstruction
    ///
    /// Performs a vertical sub-band reconstruction of a two-dimensional array of coefficients.
    ///
    /// See also F.3.5
    fn vertical_sr(mut a: Array2D<T>) -> Array2D<T> {
        for col in 0..a.width as i32 {
            let yv = a.get_column(col);
            let xv = Self::m_1d_filt_r(ExtendedSignal(yv));
            a.set_column(col, &xv);
        }
        a
    }

    /// HOR_SR horizontal signal reconstruction
    ///
    /// Performs a horizontal sub-band reconstruction of a two-dimensional array of coefficients.
    ///
    /// See also F.3.4
    fn horizontal_sr(mut a: Array2D<T>) -> Array2D<T> {
        for row in 0..a.height as i32 {
            // Grab a row
            let yu = a.get_row(row);
            let xu = Self::m_1d_filt_r(ExtendedSignal(yu));
            a.set_row(row, &xu);
        }
        a
    }
}

/// Blanket implementation for filters that use InternalFilter
impl<T: ValidNumeric, U: InternalFilter<T>> Filter<T> for U {
    fn idwt(ll: Array2D<T>, groups: Vec<SubBandGroup<Array2D<T>>>) -> Array2D<T> {
        let out = groups.iter().fold(ll, |ll, group| match group {
            SubBandGroup::Full { .. } => panic!("Not expecting any full groups"),
            SubBandGroup::LL(_) => panic!("Expected only the first group to be a LL"),
            SubBandGroup::Partial { lh, hl, hh } => Self::m_2d_sr(SubBandGroup::Full {
                ll: &ll,
                lh,
                hl,
                hh,
            }),
        });
        out
    }
}

/// Implement 1D_FILTR and 1D_FILTD for 53 filter
impl<T: ValidNumeric> InternalFilter<T> for Reversible53 {
    fn m_1d_filt_r(y: ExtendedSignal<T>) -> Vec<T> {
        let width = y.0.len();
        if width == 0 {
            return vec![];
        } else if width == 1 {
            return vec![y[0]];
        }
        // fought the compiler here
        let two = T::from_i32(2).unwrap();
        let four = T::from_i32(4).unwrap();
        // alloc 1 more than width in case we need it to calculate a final odd-index element
        let mut x = Vec::with_capacity(width + 1);
        x.extend(&y.0);

        // process even elements
        for i in (0..width as i64).step_by(2) {
            x[i as usize] = y[i] - (y[i - 1] + y[i + 1] + two).div_euclid(&four);
        }
        if width.is_multiple_of(2) {
            // need the extra for calculating the last odd index
            x.push(x[width - 2]);
        }
        // process odd elements
        for i in (1..width).step_by(2) {
            x[i] = y[i as i64] + (x[i - 1] + x[i + 1]).div_euclid(&two);
        }
        x.truncate(width);
        x
    }
}

/// Implement 1D_FILTR and 1D_FILTD for 97 filter
impl<T: ValidNumeric> InternalFilter<T> for Irreversible97 {
    fn m_1d_filt_r(_y: ExtendedSignal<T>) -> Vec<T> {
        todo!("need 97");
    }
}

trait ValidNumeric: Num + Copy + Default + Euclid + FromPrimitive + std::fmt::Debug {}

impl ValidNumeric for i32 {}
impl ValidNumeric for i64 {}
impl ValidNumeric for f32 {}
impl ValidNumeric for f64 {}

/// F.3.3 2D_INTERLEAVE procedure
///
/// good candidate for a faster or more memory efficient version
fn interleave<T: Default + Copy>(group: SubBandGroup<&Array2D<T>>) -> Array2D<T> {
    let SubBandGroup::Full { ll, hl, lh, hh } = group else {
        panic!("interleave called without a full group");
    };
    let u0 = 0usize;
    let v0 = 0usize;
    let u1 = ll.width + hl.width;
    let v1 = ll.height + lh.height;
    // outer sub-bands should have matching dimensions, or 0 dimension
    assert!(ll.width == lh.width || lh.width == 0);
    assert!(hl.width == hh.width || hh.height == 0);
    assert!(ll.height == hl.height || hl.height == 0);
    assert!(lh.height == hh.height || hh.height == 0);
    let mut a = Array2D::new(u1, v1);

    // handle LL
    {
        let b = ll;
        let mut vb = 0; // v0.div_ceil(2);
        while !(vb >= v1.div_ceil(2)) {
            let mut ub = 0; // u0.div_ceil(2);
            while !(ub >= u1.div_ceil(2)) {
                a[(2 * ub, 2 * vb)] = b[(ub, vb)];
                ub += 1;
            }
            vb += 1;
        }
    }

    // handle HL
    {
        let b = hl;
        let mut vb = v0.div_ceil(2);

        while !(vb >= v1.div_ceil(2)) {
            let mut ub = u0 / 2;
            while !(ub >= u1 / 2) {
                a[(2 * ub + 1, 2 * vb)] = b[(ub, vb)];
                ub += 1;
            }
            vb += 1;
        }
    }

    // handle LH
    {
        let b = lh;
        let mut vb = v0 / 2;
        while !(vb >= v1 / 2) {
            let mut ub = u0.div_ceil(2);
            while !(ub >= u1.div_ceil(2)) {
                a[(2 * ub, 2 * vb + 1)] = b[(ub, vb)];
                ub += 1;
            }
            vb += 1;
        }
    }
    // handle HH
    {
        let b = hh;
        let mut vb = v0 / 2;
        while !(vb >= v1 / 2) {
            let mut ub = u0 / 2;
            while !(ub >= u1 / 2) {
                a[(2 * ub + 1, 2 * vb + 1)] = b[(ub, vb)];
                ub += 1;
            }
            vb += 1;
        }
    }

    a
}

fn deinterleave<T: Copy + Default>(a: &Array2D<T>) -> SubBandGroup<Array2D<T>> {
    let u0 = 0;
    let v0 = 0;
    let u1 = a.width;
    let v1 = a.height;

    let ll = {
        let mut b = Array2D::new(u1.div_ceil(2), v1.div_ceil(2));
        let mut vb = 0;
        while !(vb >= v1.div_ceil(2)) {
            let mut ub = 0;
            while !(ub >= u1.div_ceil(2)) {
                b[(ub, vb)] = a[(2 * ub, 2 * vb)];
                ub += 1;
            }
            vb += 1;
        }
        b
    };
    let hl = {
        let mut b = Array2D::new(u1 / 2, v1.div_ceil(2));
        let mut vb = 0;
        while !(vb >= v1.div_ceil(2)) {
            let mut ub = 0;
            while !(ub >= u1 / 2) {
                b[(ub, vb)] = a[(2 * ub + 1, 2 * vb)];
                ub += 1;
            }
            vb += 1;
        }
        b
    };
    let lh = {
        let mut b = Array2D::new(u1.div_ceil(2), v1 / 2);
        let mut vb = 0;
        while !(vb >= v1 / 2) {
            let mut ub = 0;
            while !(ub >= u1.div_ceil(2)) {
                b[(ub, vb)] = a[(2 * ub, 2 * vb + 1)];
                ub += 1;
            }
            vb += 1;
        }
        b
    };
    let hh = {
        let mut b = Array2D::new(u1 / 2, v1 / 2);
        let mut vb = 0;
        while !(vb >= v1 / 2) {
            let mut ub = 0;
            while !(ub >= u1 / 2) {
                b[(ub, vb)] = a[(2 * ub + 1, 2 * vb + 1)];
                ub += 1;
            }
            vb += 1;
        }
        b
    };

    SubBandGroup::Full { ll, hl, lh, hh }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::Array2D;

    const EPSILON: f64 = 1e-10;
    const EPSILON_97: f64 = 1e-6;

    fn approx_eq(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() < eps
    }

    fn arrays_approx_eq(a: &Array2D<f64>, b: &Array2D<f64>, eps: f64) -> bool {
        if a.width() != b.width() || a.height() != b.height() {
            return false;
        }
        for row in 0..a.height() {
            for col in 0..a.width() {
                if !approx_eq(a[(col, row)], b[(col, row)], eps) {
                    return false;
                }
            }
        }
        true
    }

    #[test]
    fn test_basic_2d_interleave() {
        //  LL | HL
        //  -------
        //  LH | HH
        //
        //  0  1 |  4  5
        //  2  3 |  6  7
        //---------------
        //  8  9 | 10 11
        //
        //  becomes
        //
        //  LL:
        //  0  4  1  5
        //  8 10  9 11
        //  2  6  3  7
        let exp = Array2D::from_data(vec![0, 4, 1, 5, 8, 10, 9, 11, 2, 6, 3, 7], 4, 3);
        let ll = Array2D::from_data(vec![0, 1, 2, 3], 2, 2);
        let hl = Array2D::from_data(vec![4, 5, 6, 7], 2, 2);
        let lh = Array2D::from_data(vec![8, 9], 2, 1);
        let hh = Array2D::from_data(vec![10, 11], 2, 1);

        let group = SubBandGroup::Full {
            ll: &ll,
            hl: &hl,
            lh: &lh,
            hh: &hh,
        };
        let ll_new = interleave(group);

        assert_eq!(exp, ll_new);
    }

    #[test]
    fn test_basic_2d_deinterleave() {
        let given = Array2D::from_data(vec![0, 4, 1, 5, 8, 10, 9, 11, 2, 6, 3, 7], 4, 3);
        let exp_ll = Array2D::from_data(vec![0, 1, 2, 3], 2, 2);
        let exp_hl = Array2D::from_data(vec![4, 5, 6, 7], 2, 2);
        let exp_lh = Array2D::from_data(vec![8, 9], 2, 1);
        let exp_hh = Array2D::from_data(vec![10, 11], 2, 1);

        let SubBandGroup::Full { ll, hl, lh, hh } = deinterleave(&given) else {
            panic!("Not given a full group");
        };

        assert_eq!(exp_ll, ll);
        assert_eq!(exp_hl, hl);
        assert_eq!(exp_lh, lh);
        assert_eq!(exp_hh, hh);
    }

    #[test]
    fn test_basic_signal_extension() {
        let signal: Vec<i64> = (1..8).collect();

        let extended = ExtendedSignal(signal);

        let result: Vec<i64> = (-8..15).map(|i| extended[i]).collect();
        assert_eq!(
            vec![5, 6, 7, 6, 5, 4, 3, 2, 1, 2, 3, 4, 5, 6, 7, 6, 5, 4, 3, 2, 1, 2, 3],
            result
        );
    }

    #[test]
    fn test_decode_j10_1d() {
        // example given in J.10
        let coeffs = [-26, 1, -22, 5, -30, 1, -32, 0, -19];
        let samples = [101, 103, 104, 105, 96, 97, 96, 102, 109];
        let level_shift = (2i64).pow(7); // Ssiz = 7
        let signal: Vec<i64> = samples.iter().map(|v| (*v) - level_shift).collect();

        let reconstructed = Reversible53::m_1d_filt_r(ExtendedSignal(coeffs.to_vec()));
        for (i, (&orig, &recon)) in signal.iter().zip(reconstructed.iter()).enumerate() {
            assert_eq!(orig, recon);
        }
    }

    #[test]
    fn test_decode_j10_2d() {
        // example given in J.10
        let ll = Array2D::from_data(vec![-26, -22, -30, -32, -19], 1, 5);
        let lh = Array2D::from_data(vec![1, 5, 1, 0], 1, 4);

        let recon = Reversible53::idwt(
            ll,
            vec![SubBandGroup::Partial {
                hl: Array2D::new(0, 5),
                lh,
                hh: Array2D::new(0, 4),
            }],
        );

        let samples = [101, 103, 104, 105, 96, 97, 96, 102, 109];
        let level_shift = (2i32).pow(7); // Ssiz = 7
        let signal: Vec<i32> = samples.iter().map(|v| (*v) - level_shift).collect();
        let orig = Array2D::from_data(signal, 1, 9);

        assert_eq!(orig, recon, "Expected reconstruction to match original");
    }
}
