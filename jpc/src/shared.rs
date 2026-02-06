use std::ops::{Index, IndexMut};

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

/// Convenience struct for hold image/tile/precinct/code-block bounds information
#[derive(Debug, Clone, Copy)]
pub struct Bounds {
    pub x0: u32,
    pub x1: u32,
    pub y0: u32,
    pub y1: u32,
}

/// A 2D array
#[derive(Debug, Clone)]
pub struct Array2D<T> {
    data: Vec<T>,
    pub width: usize,
    pub height: usize,
}

impl<T> Index<(usize, usize)> for Array2D<T> {
    type Output = T;

    fn index(&self, (col, row): (usize, usize)) -> &Self::Output {
        &self.data[row * self.width + col]
    }
}

impl<T> IndexMut<(usize, usize)> for Array2D<T> {
    fn index_mut(&mut self, (col, row): (usize, usize)) -> &mut Self::Output {
        &mut self.data[row * self.width + col]
    }
}

impl<T: Clone + Default> Array2D<T> {
    /// Create a new 2D array with given dimensions
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            data: vec![T::default(); width * height],
            width,
            height,
        }
    }
    /// Create from existing data
    pub fn from_data(data: Vec<T>, width: usize, height: usize) -> Self {
        assert_eq!(data.len(), width * height);
        Self {
            data,
            width,
            height,
        }
    }
}

impl<T: PartialEq> PartialEq for Array2D<T> {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data && self.width == other.width && self.height == other.height
    }
}

impl<T> Array2D<T> {
    pub fn map_elements<O, F>(&self, f: F) -> Array2D<O>
    where
        O: Default + Clone,
        F: Fn(&T) -> O,
    {
        let out_data: Vec<O> = self.data.iter().map(f).collect();
        Array2D::from_data(out_data, self.width, self.height)
    }
}

/// todo are these needed ?
impl<T: Clone + Default> Array2D<T> {
    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    /// Get a column as a vector
    pub fn get_column(&self, u: i32) -> Vec<T> {
        let col = u as usize;
        (0..self.height)
            .map(|row| self.data[row * self.width + col].clone())
            .collect()
    }

    /// Set a column from a vector
    pub fn set_column(&mut self, u: i32, values: &[T]) {
        let col = u as usize;
        for (row, value) in values.iter().enumerate() {
            self.data[row * self.width + col] = value.clone();
        }
    }

    /// Get a row as a vector
    pub fn get_row(&self, v: i32) -> Vec<T> {
        let row = v as usize;
        self.data[row * self.width..(row + 1) * self.width].to_vec()
    }

    /// Set a row from a vector
    pub fn set_row(&mut self, v: i32, values: &[T]) {
        let row = v as usize;
        self.data[row * self.width..(row + 1) * self.width].clone_from_slice(values);
    }

    pub fn elements(&self) -> &Vec<T> {
        &self.data
    }
}

#[derive(Debug)]
pub enum SubBandGroup<T> {
    Full { ll: T, hl: T, lh: T, hh: T },
    LL(T),
    Partial { hl: T, lh: T, hh: T },
}
