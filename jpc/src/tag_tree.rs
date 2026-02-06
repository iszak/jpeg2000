//! A tag tree represents a 2d-array of natural numbers.
//!
//! From ITU-T T.800(V4) | ISO/IEC 15444-1:2024
//! B.10.2 A tag tree is a way of representing a two-dimensional array of non-negative integers in
//! a hierarchical way. It successively creates reduced resolution levels of this two-dimensional
//! array, forming a tree. At every node of this tree the minimum integer of the (up to four) nodes
//! below it is recorded. Figure B.12 shows an example of this representation. The notation, qi(m,
//! n), is the value at the node that is mth from the left and nth from the top, at the ith level.
//! Level 0 is the lowest level of the tag tree; it contains the top node.
use std::io::{self, Read};

use log::{debug, info};

use crate::{bit_reader::BitReader, shared::I2};

/// A tag tree node has several states depending on how much has been decoded
#[derive(Debug, Default, Clone, Copy)]
enum TagTreeNode {
    /// The default uninitialized state
    #[default]
    SeeParent,
    /// The node is in the process of being decoded
    AtLeast(u32),
    /// The node value is known
    Value(u32),
}

/// The ZeroPlaneTagTree provides a simple interface for grabbing zero bit plane tag tree information
#[derive(Debug)]
pub struct ZeroPlaneTagTree {
    tag_tree: TagTreeDecoder,
}

impl ZeroPlaneTagTree {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            tag_tree: TagTreeDecoder::new(width, height),
        }
    }

    /// Read enough bits to decide on value
    ///
    /// The maximum number of zero planes is no more than 38, since 38 is the highest bit depth
    pub fn read<R: Read>(
        &mut self,
        dim_idx: I2,
        br: &mut BitReader<'_, R>,
    ) -> Result<u8, io::Error> {
        self.tag_tree.read(dim_idx, br).map(|v| {
            if v <= u8::MAX as u32 {
                v as u8
            } else {
                panic!("Invalid zero plane number")
            }
        })
    }
}

/// The InclusionTagTree provides a simple interface for testing inclusion status of codeblocks
#[derive(Debug)]
pub struct InclusionTagTree {
    tag_tree: TagTreeDecoder,
}

impl InclusionTagTree {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            tag_tree: TagTreeDecoder::new(width, height),
        }
    }

    /// Check if a code block was included before layer index bound
    pub fn query_inclusion(&self, dim_idx: I2, bound: u32) -> bool {
        match self.tag_tree.query_i2(dim_idx) {
            TagTreeNode::SeeParent => panic!("base case fail"),
            TagTreeNode::AtLeast(_) => false,
            TagTreeNode::Value(v) => v < bound,
        }
    }

    /// Reads enough bits to decide if value will be greater than bound.
    pub fn read_for_inclusion<R: Read>(
        &mut self,
        dim_idx: I2,
        bound: u32,
        br: &mut BitReader<'_, R>,
    ) -> Result<bool, io::Error> {
        let node = self
            .tag_tree
            .read_until_bound(dim_idx, bound, self.tag_tree.max_depth, br)?;
        match node {
            TagTreeNode::SeeParent => panic!("unable to handle"),
            TagTreeNode::AtLeast(v) => {
                // partials must be bigger than bound to be returned
                assert!(v > bound, "Expected to know partial is large enough");
                Ok(false)
            }
            TagTreeNode::Value(v) => {
                assert!(v <= bound, "was previously included");
                Ok(true)
            }
        }
    }
}

/// A decoder from tag tree bits to numbers in the 2d-array.
///
/// TagTreeDecoder takes in bits and returns values from the represented 2d-array. Only positive
/// integers will ever be produced.
///
/// See Section B.10.2 for description and Figure B.13 + Table B.5 for example usages
/// From ITU-T T.800(V4) | ISO/IEC 15444-1:2024
#[derive(Debug)]
struct TagTreeDecoder {
    max_depth: usize,
    /// levels is a Vec containing data (width, items) for each level of the tree.
    levels: Vec<(usize, Vec<TagTreeNode>)>,
    item_count: usize,
}

impl TagTreeDecoder {
    pub fn new(width: usize, height: usize) -> Self {
        let mut mw = width;
        let mut mh = height;
        let mut max_depth = 0;
        let mut levels = Vec::new();
        // Determine max depth by dividing out groups of 2x2==4
        while mw > 1 || mh > 1 {
            let w = mw.max(1);
            let size: usize = w * mh.max(1);
            levels.push((w, vec![TagTreeNode::SeeParent; size]));
            debug!("added vec of size {size}");
            max_depth += 1;
            mw = mw.div_ceil(2);
            mh = mh.div_ceil(2);
        }
        levels.push((1, vec![TagTreeNode::AtLeast(0)]));
        levels.reverse(); // reverse in place so level 0 is at index 0
        info!("Need a depth of {max_depth} to represent tag tree");
        assert_eq!(max_depth + 1, levels.len());
        Self {
            max_depth,
            levels,
            item_count: width * height,
        }
    }

    /// query the tag tree to determine what is know about a value for a given raster index
    fn query_raster(&self, raster_index: usize) -> TagTreeNode {
        let (c, r) = (
            raster_index % self.item_count,
            raster_index / self.item_count,
        );
        let idx = I2 {
            x: c as u32,
            y: r as u32,
        };
        self.query_recursive(idx, self.max_depth)
    }

    /// Internal call that needs to know depth
    fn query_exact(&self, depth: usize, column: usize, row: usize) -> TagTreeNode {
        let (width, level) = &self.levels[depth];
        level[row * width + column]
    }

    /// Query node, and parents, to find the correct value for this node
    fn query_recursive(&self, dim_idx: I2, level: usize) -> TagTreeNode {
        let value = *self.node(dim_idx, level);
        if let TagTreeNode::SeeParent = value {
            let parent_dim = I2 {
                x: dim_idx.x / 2,
                y: dim_idx.y / 2,
            };
            let parent = self.query_recursive(parent_dim, level - 1);
            match parent {
                TagTreeNode::SeeParent => panic!("no base case"),
                TagTreeNode::AtLeast(v) => TagTreeNode::AtLeast(v),
                TagTreeNode::Value(v) => TagTreeNode::AtLeast(v),
            }
        } else {
            value
        }
    }

    pub fn query_inclusion(&self, dim_idx: I2, bound: u32) -> bool {
        match self.query_i2(dim_idx) {
            TagTreeNode::SeeParent => panic!("base case fail"),
            TagTreeNode::AtLeast(_) => false,
            TagTreeNode::Value(v) => v < bound,
        }
    }

    fn query_i2(&self, dim_idx: I2) -> TagTreeNode {
        self.query_recursive(dim_idx, self.max_depth)
    }

    /// Reads enough bits to decide if value will be greater than bound
    /// ie return will be either
    /// TagTreeNode::Value(v) where v <= bound
    /// TagTreeNode::AtLeast(v) where v > bound
    fn read_until_bound<R: Read>(
        &mut self,
        dim_idx: I2,
        bound: u32,
        level: usize,
        br: &mut BitReader<'_, R>,
    ) -> Result<TagTreeNode, io::Error> {
        let mut partial = match *self.node(dim_idx, level) {
            value_node @ TagTreeNode::Value(_) => {
                // have a value at this depth, return it to caller
                return Ok(value_node);
            }
            TagTreeNode::SeeParent => {
                let parent_dim = I2 {
                    x: dim_idx.x / 2,
                    y: dim_idx.y / 2,
                };
                let parent = self.read_until_bound(parent_dim, bound, level - 1, br)?;
                match parent {
                    TagTreeNode::SeeParent => panic!("no base case"),
                    TagTreeNode::AtLeast(v) => {
                        // partial at the parent level nothing left to do, return it to caller
                        assert!(v > bound, "Expected invariant to be satisfied");
                        return Ok(parent);
                    }
                    TagTreeNode::Value(v) => v,
                }
            }
            TagTreeNode::AtLeast(v) => v,
        };

        while partial <= bound {
            if br.next_bit()? {
                // Accept the value
                *self.node_mut(dim_idx, level) = TagTreeNode::Value(partial);
                return Ok(*self.node(dim_idx, level));
            }
            partial += 1;
        }
        *self.node_mut(dim_idx, level) = TagTreeNode::AtLeast(partial);
        Ok(*self.node(dim_idx, level))
    }

    fn read<R: Read>(&mut self, dim_idx: I2, br: &mut BitReader<'_, R>) -> Result<u32, io::Error> {
        let TagTreeNode::Value(v) = self.read_until_bound(dim_idx, u32::MAX, self.max_depth, br)?
        else {
            panic!("Unable to read value");
        };
        Ok(v)
    }

    /// way to grab node mutable
    fn node_mut(&mut self, dim_idx: I2, level: usize) -> &mut TagTreeNode {
        let (width, vals) = &mut self.levels[level];
        let idx = (dim_idx.y as usize) * *width + (dim_idx.x as usize);
        &mut vals[idx]
    }

    /// way to grab a node
    fn node(&self, dim_idx: I2, level: usize) -> &TagTreeNode {
        let (width, vals) = &self.levels[level];
        let idx = (dim_idx.y as usize) * *width + (dim_idx.x as usize);
        &vals[idx]
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    fn init_logger() {
        let _ = env_logger::builder()
            .is_test(true)
            .filter_level(log::LevelFilter::Info)
            .try_init();
    }

    #[test]
    fn test_oner() -> Result<(), io::Error> {
        init_logger();
        // Test a one item tree
        let mut tt = TagTreeDecoder::new(1, 1);
        assert_eq!(0, tt.max_depth);
        let mut cursor = Cursor::new([0b0010_0000]);
        let mut bits_read_exp = 0;
        let mut br = BitReader::new(&mut cursor).unwrap();

        assert_eq!(2, tt.read(I2 { x: 0, y: 0 }, &mut br)?);
        bits_read_exp += 3;
        assert_eq!(bits_read_exp, br.bits_read());
        Ok(())
    }

    /// Test a two level tree, max_depth == 1.
    ///
    /// Level 1
    /// ┌────────┬────────┐
    /// │   1    │   1    │
    /// │ q₁(0,0)│ q₁(1,0)│
    /// ├────────┼────────┤
    /// │   2    │   2    │
    /// └────────┴────────┘
    ///
    /// Level 0 (Root - Minimum of entire tree):
    /// ┌─────────────────┐
    /// │        1        │
    /// │    q₀(0,0)      │
    /// └─────────────────┘
    #[test]
    fn test_two_level() -> Result<(), io::Error> {
        init_logger();
        let mut tt = TagTreeDecoder::new(2, 2);
        assert_eq!(1, tt.max_depth);

        let mut cursor = Cursor::new([0b0111_0101]);
        let mut bits_read_exp = 0;
        let mut br = BitReader::new(&mut cursor)?;

        assert_eq!(1, tt.read(I2 { x: 0, y: 0 }, &mut br)?);
        bits_read_exp += 3;
        assert_eq!(bits_read_exp, br.bits_read());
        assert_eq!(1, tt.read(I2 { x: 1, y: 0 }, &mut br)?);
        bits_read_exp += 1;
        assert_eq!(bits_read_exp, br.bits_read());
        assert_eq!(2, tt.read(I2 { x: 0, y: 1 }, &mut br)?);
        bits_read_exp += 2;
        assert_eq!(bits_read_exp, br.bits_read());
        assert_eq!(2, tt.read(I2 { x: 1, y: 1 }, &mut br)?);
        bits_read_exp += 2;
        assert_eq!(bits_read_exp, br.bits_read());
        Ok(())
    }

    /// Test basic tag tree from B.10.2
    /// Level 3 (Original array of numbers):
    /// ┌───┬───┬───┬───┬───┬───┐
    /// │ 1 │ 3 │ 2 │ 3 │ 2 │ 3 │
    /// │q₃ │q₃ │q₃ │   │   │   │
    /// │0,0│1,0│2,0│   │   │   │
    /// ├───┼───┼───┼───┼───┼───┤
    /// │ 2 │ 2 │ 1 │ 4 │ 3 │ 2 │
    /// ├───┼───┼───┼───┼───┼───┤
    /// │ 2 │ 2 │ 2 │ 2 │ 1 │ 2 │
    /// └───┴───┴───┴───┴───┴───┘
    ///
    /// Level 2 (Minimum of 2x2 blocks from Level 3):
    /// ┌────────┬────────┬───────┐
    /// │   1    │   1    │   2   │
    /// │ q₂(0,0)│ q₂(1,0)│       │
    /// ├────────┼────────┼───────┤
    /// │   2    │   2    │   1   │
    /// └────────┴────────┴───────┘
    ///
    /// Level 1 (Minimum of 2x2 blocks from Level 2):
    /// ┌───────────────┬───────────────┐
    /// │       1       │       1       │
    /// │    q₁(0,0)    │               │
    /// └───────────────┴───────────────┘
    ///
    /// Level 0 (Root - Minimum of entire tree):
    /// ┌───────────────────────────────┐
    /// │               1               │
    /// │           q₀(0,0)             │
    /// └───────────────────────────────┘
    ///
    /// Each level represents the minimum value of a 2x2 block
    /// (or smaller at boundaries) from the level below.
    #[test]
    fn test_given_example() -> Result<(), io::Error> {
        init_logger();
        let mut tt = TagTreeDecoder::new(6, 3);

        let mut cursor = Cursor::new([
            0b0111_1001,
            0b1010_0110,
            0b1101_0101,
            0b1000_1011,
            0b0111_0111,
            0b1101_0000,
        ]);
        let mut bits_read_exp = 0;
        let mut br = BitReader::new(&mut cursor)?;

        assert_eq!(3, tt.max_depth);

        assert_eq!(1, tt.read(I2 { x: 0, y: 0 }, &mut br)?);
        bits_read_exp += 5;
        assert_eq!(bits_read_exp, br.bits_read());
        assert_eq!(3, tt.read(I2 { x: 1, y: 0 }, &mut br)?);
        bits_read_exp += 3;
        assert_eq!(bits_read_exp, br.bits_read());
        assert_eq!(2, tt.read(I2 { x: 2, y: 0 }, &mut br)?);
        bits_read_exp += 3;
        assert_eq!(bits_read_exp, br.bits_read());
        assert_eq!(3, tt.read(I2 { x: 3, y: 0 }, &mut br)?);
        bits_read_exp += 3;
        assert_eq!(bits_read_exp, br.bits_read());
        assert_eq!(2, tt.read(I2 { x: 4, y: 0 }, &mut br)?);
        bits_read_exp += 4;
        assert_eq!(bits_read_exp, br.bits_read());
        assert_eq!(3, tt.read(I2 { x: 5, y: 0 }, &mut br)?);
        bits_read_exp += 2;
        assert_eq!(bits_read_exp, br.bits_read());

        // Next row
        assert_eq!(2, tt.read(I2 { x: 0, y: 1 }, &mut br)?);
        bits_read_exp += 2;
        assert_eq!(bits_read_exp, br.bits_read());
        assert_eq!(2, tt.read(I2 { x: 1, y: 1 }, &mut br)?);
        bits_read_exp += 2;
        assert_eq!(bits_read_exp, br.bits_read());
        assert_eq!(1, tt.read(I2 { x: 2, y: 1 }, &mut br)?);
        bits_read_exp += 1;
        assert_eq!(bits_read_exp, br.bits_read());
        assert_eq!(4, tt.read(I2 { x: 3, y: 1 }, &mut br)?);
        bits_read_exp += 4;
        assert_eq!(bits_read_exp, br.bits_read());
        assert_eq!(3, tt.read(I2 { x: 4, y: 1 }, &mut br)?);
        bits_read_exp += 2;
        assert_eq!(bits_read_exp, br.bits_read());
        assert_eq!(2, tt.read(I2 { x: 5, y: 1 }, &mut br)?);
        bits_read_exp += 1;
        assert_eq!(bits_read_exp, br.bits_read());

        // Next row
        assert_eq!(2, tt.read(I2 { x: 0, y: 2 }, &mut br)?);
        bits_read_exp += 3;
        assert_eq!(bits_read_exp, br.bits_read());
        assert_eq!(2, tt.read(I2 { x: 1, y: 2 }, &mut br)?);
        bits_read_exp += 1;
        assert_eq!(bits_read_exp, br.bits_read());
        assert_eq!(2, tt.read(I2 { x: 2, y: 2 }, &mut br)?);
        bits_read_exp += 3;
        assert_eq!(bits_read_exp, br.bits_read());
        assert_eq!(2, tt.read(I2 { x: 3, y: 2 }, &mut br)?);
        bits_read_exp += 1;
        assert_eq!(bits_read_exp, br.bits_read());
        assert_eq!(1, tt.read(I2 { x: 4, y: 2 }, &mut br)?);
        bits_read_exp += 2;
        assert_eq!(bits_read_exp, br.bits_read());
        assert_eq!(2, tt.read(I2 { x: 5, y: 2 }, &mut br)?);
        bits_read_exp += 2;
        assert_eq!(bits_read_exp, br.bits_read());
        Ok(())
    }

    /// Figure B.13 and Table B.5
    ///
    /// Tests parsing inclusion status
    #[test]
    fn test_packet_cb_inclusion() -> Result<(), io::Error> {
        init_logger();
        let mut incl_tree = InclusionTagTree::new(3, 2);

        // Bit reader calls expected
        // First "Packet"
        // for 0,0 read 111
        // for 1,0 read 1
        // for 2,0 read 0
        // for 0,1 read 0
        // for 1,1 read 0
        // for 2,1 no read
        // Second "Packet"
        // for 0,0 no read
        // for 1,0 no read
        // for 2,0 read 10
        // for 0,1 read 0
        // for 1,1 read 1
        // for 2,1 read 1
        let mut cursor = Cursor::new([0b1111_0001, 0b0011_0000]);

        let mut bits_read_exp = 0;
        let mut br = BitReader::new(&mut cursor)?;

        {
            let cb00 = I2 { x: 0, y: 0 };
            let included = incl_tree.query_inclusion(cb00, 0);
            assert!(!included);
            assert_eq!(bits_read_exp, br.bits_read());

            let to_include = incl_tree.read_for_inclusion(cb00, 0, &mut br)?;
            assert!(to_include, "expected to include value");
            bits_read_exp += 3;
            assert_eq!(bits_read_exp, br.bits_read());
        }
        {
            let cb10 = I2 { x: 1, y: 0 };
            let included = incl_tree.query_inclusion(cb10, 0);
            assert!(!included);

            let to_include = incl_tree.read_for_inclusion(cb10, 0, &mut br)?;
            assert!(to_include, "expected to include value");
            bits_read_exp += 1;
            assert_eq!(bits_read_exp, br.bits_read());
        }
        {
            let cb20 = I2 { x: 2, y: 0 };
            let included = incl_tree.query_inclusion(cb20, 0);
            assert!(!included);

            let to_include = incl_tree.read_for_inclusion(cb20, 0, &mut br)?;
            assert!(!to_include, "!expected to include value");
            bits_read_exp += 1;
            assert_eq!(bits_read_exp, br.bits_read());
        }
        {
            let cb01 = I2 { x: 0, y: 1 };
            let included = incl_tree.query_inclusion(cb01, 0);
            assert!(!included);

            let to_include = incl_tree.read_for_inclusion(cb01, 0, &mut br)?;
            assert!(!to_include, "!expected to include value");
            bits_read_exp += 1;
            assert_eq!(bits_read_exp, br.bits_read());
        }
        {
            let cb11 = I2 { x: 1, y: 1 };
            let included = incl_tree.query_inclusion(cb11, 0);
            assert!(!included);

            let to_include = incl_tree.read_for_inclusion(cb11, 0, &mut br)?;
            assert!(!to_include, "!expected to include value");
            bits_read_exp += 1;
            assert_eq!(bits_read_exp, br.bits_read());
        }
        {
            let cb21 = I2 { x: 2, y: 1 };
            let included = incl_tree.query_inclusion(cb21, 0);
            assert!(!included);

            let to_include = incl_tree.read_for_inclusion(cb21, 0, &mut br)?;
            assert!(!to_include, "!expected to include value");
            bits_read_exp += 0;
            assert_eq!(bits_read_exp, br.bits_read());
        }

        // "Packet" for second layer
        {
            let cb00 = I2 { x: 0, y: 0 };
            let included = incl_tree.query_inclusion(cb00, 1);
            assert!(included);
            assert_eq!(bits_read_exp, br.bits_read());
            // already included
        }
        {
            let cb10 = I2 { x: 1, y: 0 };
            let included = incl_tree.query_inclusion(cb10, 1);
            assert!(included);
            assert_eq!(bits_read_exp, br.bits_read());
            // already included
        }
        {
            let cb20 = I2 { x: 2, y: 0 };
            let included = incl_tree.query_inclusion(cb20, 1);
            assert!(!included);

            let to_include = incl_tree.read_for_inclusion(cb20, 1, &mut br)?;
            assert!(!to_include, "!expected to include value");
            bits_read_exp += 2;
            assert_eq!(bits_read_exp, br.bits_read());
        }
        {
            let cb01 = I2 { x: 0, y: 1 };
            let included = incl_tree.query_inclusion(cb01, 1);
            assert!(!included);

            let to_include = incl_tree.read_for_inclusion(cb01, 1, &mut br)?;
            assert!(!to_include, "!expected to include value");
            bits_read_exp += 1;
            assert_eq!(bits_read_exp, br.bits_read());
        }
        {
            let cb11 = I2 { x: 1, y: 1 };
            let included = incl_tree.query_inclusion(cb11, 1);
            assert!(!included);

            let to_include = incl_tree.read_for_inclusion(cb11, 1, &mut br)?;
            assert!(to_include, "expected to include value");
            bits_read_exp += 1;
            assert_eq!(bits_read_exp, br.bits_read());
        }
        {
            let cb21 = I2 { x: 2, y: 1 };
            let included = incl_tree.query_inclusion(cb21, 1);
            assert!(!included);

            let to_include = incl_tree.read_for_inclusion(cb21, 1, &mut br)?;
            assert!(to_include, "expected to include value");
            bits_read_exp += 1;
            assert_eq!(bits_read_exp, br.bits_read());
        }
        Ok(())
    }

    #[test]
    fn test_tag_tree_zero_bit() -> Result<(), io::Error> {
        init_logger();
        let mut zero_tree = ZeroPlaneTagTree::new(3, 2);

        // Bit reader calls expected
        // First "Packet"
        // for 0,0 read 000111 -> 3
        // for 1,0 read 01 -> 4
        //
        // Second "Packet"
        // for 1,1 read 1 -> 3
        // for 2,1 00011 -> 6
        let mut cursor = Cursor::new([0b0001_1101, 0b1000_1100]);

        let mut bits_read_exp = 0;
        let mut br = BitReader::new(&mut cursor)?;

        {
            let cb00 = I2 { x: 0, y: 0 };
            let zbs = zero_tree.read(cb00, &mut br)?;
            assert_eq!(3, zbs);
            bits_read_exp += 6;
            assert_eq!(bits_read_exp, br.bits_read());
        }
        {
            let cb10 = I2 { x: 1, y: 0 };
            let zbs = zero_tree.read(cb10, &mut br)?;
            assert_eq!(4, zbs);
            bits_read_exp += 2;
            assert_eq!(bits_read_exp, br.bits_read());
        }
        {
            let cb11 = I2 { x: 1, y: 1 };
            let zbs = zero_tree.read(cb11, &mut br)?;
            assert_eq!(3, zbs);
            bits_read_exp += 1;
            assert_eq!(bits_read_exp, br.bits_read());
        }
        {
            let cb21 = I2 { x: 2, y: 1 };
            let zbs = zero_tree.read(cb21, &mut br)?;
            assert_eq!(6, zbs);
            bits_read_exp += 5;
            assert_eq!(bits_read_exp, br.bits_read());
        }
        Ok(())
    }
}
