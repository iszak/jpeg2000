use log::info;

/// A tag tree represents a 2d-array of natural numbers.
///
/// B.10.2 A tag tree is a way of representing a two-dimensional array of non-negative integers in
/// a hierarchical way. It successively creates reduced resolution levels of this two-dimensional
/// array, forming a tree. At every node of this tree the minimum integer of the (up to four) nodes
/// below it is recorded. Figure B.12 shows an example of this representation. The notation, qi(m,
/// n), is the value at the node that is mth from the left and nth from the top, at the ith level.
/// Level 0 is the lowest level of the tag tree; it contains the top node.
pub struct TagTree {}

/// A decoder from tag tree bits to numbers in the 2d-array.
///
/// TagTreeDecoder takes in bits and returns values from the represented 2d-array. Only positive
/// integers will ever be produced.
#[derive(Debug)]
pub struct TagTreeDecoder {
    max_depth: usize,
    cur_depth: usize,
    cur_value: u8,
    /// levels is a Vec containing data (width, items) for each level of the tree.
    levels: Vec<(usize, Vec<u8>)>, // Todo, what should be here?
}

impl TagTreeDecoder {
    pub fn new(width: usize, height: usize) -> Self {
        let mut mw = width;
        let mut mh = height;
        let mut max_depth = 0;
        let mut levels = Vec::new();
        // Determine max depth by dividing out groups of 4
        while mw > 1 || mh > 1 {
            let w = mw.max(1);
            let size: usize = w * mh.max(1);
            levels.push((w, Vec::with_capacity(size)));
            println!("added vec of size {size}");
            max_depth += 1;
            mw = mw.div_ceil(2);
            mh = mh.div_ceil(2);
        }
        levels.push((1, Vec::with_capacity(1)));
        levels.reverse(); // reverse in place so level 0 is at index 0
        println!("Need a depth of {max_depth} to represent tag tree");

        assert_eq!(max_depth + 1, levels.len());

        Self {
            max_depth,
            cur_depth: 0,
            cur_value: 0,
            levels,
        }
    }

    fn cur_offset(&self) -> usize {
        match self.levels.get(self.cur_depth) {
            None => panic!("Not deep enough to know current location."),
            Some((cw, clvl)) => clvl.len(),
        }
    }

    // TODO fix push_bit type signature for better return type
    pub fn push_bit(&mut self, b: u8) -> Option<u8> {
        if b == 0 {
            self.cur_value += 1;
            return None;
        }
        assert_eq!(1, b);
        // b == 1, record value at current position, prep return value, go to next position
        let (_, lvl) = &mut self.levels[self.cur_depth];
        lvl.push(self.cur_value);
        // maintain cur_value and cur_depth to point at next position to fill
        if self.cur_depth < self.max_depth {
            // deeper!
            self.cur_depth += 1;
            info!(
                "Recorded {} now at depth {}",
                self.cur_value, self.cur_depth
            );
            return None;
        }
        if self.cur_depth == 0 && self.max_depth == 0 {
            // handle single value tag tree... todo might be nicer some where else
            return Some(self.cur_value);
        }

        // record out and walk up tree to maintain invariants
        let out = self.cur_value;

        // Find next place that needs a value and parent that provides cur_value
        loop {
            let (c_width, cur_lvl) = &self.levels[self.cur_depth];
            // now to generate value for this offset
            let offset = cur_lvl.len();
            let parent_column = (offset % c_width) / 2;
            let parent_row = (offset / c_width) / 2;
            let (pw, plvl) = &self.levels[self.cur_depth - 1];
            let parent_offset = (parent_row * pw) + parent_column;
            // if parent exists, use that for cur_value
            if parent_offset < plvl.len() {
                self.cur_value = plvl[parent_offset];
                break;
            }
            // keep walking
            self.cur_depth -= 1;
        }
        info!(
            "Recorded {} now back tracked to depth {} new cur_value {}",
            out, self.cur_depth, self.cur_value
        );
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic() {
        // Test basic tag tree from B.10.2
        let mut tt = TagTreeDecoder::new(6, 3);

        assert_eq!(3, tt.max_depth);

        assert!(tt.push_bit(0).is_none()); // 0.0,0 inc -> 1
        assert!(tt.push_bit(1).is_none()); // set 0. 0,0 = 1
        assert!(tt.push_bit(1).is_none()); // set 1. 0,0 = 1
        assert!(tt.push_bit(1).is_none()); // set 2. 0,0 = 1
        assert_eq!(Some(1), tt.push_bit(1)); // set 3. 0,0 = 1, ret
                                             //
        assert!(tt.push_bit(0).is_none()); // inc -> 2
        assert!(tt.push_bit(0).is_none()); // inc -> 3
        assert_eq!(Some(3), tt.push_bit(1)); // set 3. 1,0 = 3
                                             //
        assert!(tt.push_bit(1).is_none()); // set 2.1,0 = 1
        assert!(tt.push_bit(0).is_none()); // inc -> 2
        assert_eq!(Some(2), tt.push_bit(1)); // set 3.2,0 = 2
                                             //
        assert!(tt.push_bit(0).is_none()); // inc -> 2
        assert!(tt.push_bit(0).is_none()); // inc -> 3
        assert_eq!(Some(3), tt.push_bit(1)); // set 3.3,0 = 3

        assert!(tt.push_bit(1).is_none()); // set 1,1,0 = 1
        assert!(tt.push_bit(0).is_none()); // inc -> 2
        assert!(tt.push_bit(1).is_none()); // set 2,2,0 = 2
        assert_eq!(Some(2), tt.push_bit(1)); // set 3.4,0 = 2

        assert!(tt.push_bit(0).is_none());
        assert_eq!(Some(3), tt.push_bit(1)); // 3,5,0

        // Next row
        assert!(tt.push_bit(0).is_none());
        assert_eq!(Some(2), tt.push_bit(1)); // 3,0,1

        assert!(tt.push_bit(0).is_none());
        assert_eq!(Some(2), tt.push_bit(1)); // 3,1,1

        assert_eq!(Some(1), tt.push_bit(1)); // 3,2,1
        assert!(tt.push_bit(0).is_none());
        assert!(tt.push_bit(0).is_none());
        assert!(tt.push_bit(0).is_none());
        assert_eq!(Some(4), tt.push_bit(1)); // 3,3,1
        assert!(tt.push_bit(0).is_none());
        assert_eq!(Some(3), tt.push_bit(1)); // 3,4,1
        assert_eq!(Some(2), tt.push_bit(1)); // 3,5,1

        // Next row
        assert!(tt.push_bit(0).is_none());
        assert!(tt.push_bit(1).is_none()); // 2,0,1
        assert_eq!(Some(2), tt.push_bit(1)); // 3,0,2
        assert_eq!(Some(2), tt.push_bit(1)); // 3,1,2
        assert!(tt.push_bit(0).is_none());
        assert!(tt.push_bit(1).is_none()); // 2,1,1
        assert_eq!(Some(2), tt.push_bit(1)); // 3,2,2
        assert_eq!(Some(2), tt.push_bit(1)); // 3,3,2
        assert!(tt.push_bit(1).is_none()); // 2,2,1
        assert_eq!(Some(1), tt.push_bit(1)); // 3,4,2
        assert!(tt.push_bit(0).is_none());
        assert_eq!(Some(2), tt.push_bit(1)); // 3,5,2
    }
}
