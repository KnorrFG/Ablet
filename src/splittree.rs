use std::{collections::HashMap, iter};

use derive_more::Constructor;
use itertools::{izip, Itertools};

use crate::{BufferPosition, BufferRef, Orientation, Rect, Size};

/// How window is subdivided into splits.
///
/// Split tree is n-ary (three side-by-side columns are one level in the tree).
///
/// Direction is implicit: `vec![Leaf, Leaf]` is vertical split, `vec![vec![Leaf, Leaf]]` is
/// horizontal
///
/// Splits are ephemeral --- there are no SplitRefs, you can get-set the whole tree at once.
#[derive(Constructor, Clone)]
pub struct SplitTree {
    root: Split,
    top_orientation: Orientation,
}

pub struct SplitMap {
    pub(crate) rects: HashMap<Rect, BufferRef>,
    pub(crate) border_map: BorderMap,
}

impl SplitTree {
    const MIN_SPLIT_SIZE: Size = Size { w: 1, h: 1 };

    /// Returns a map from rects to buffer refs, unless there is less than MIN_SPLIT_SIZE
    /// cells of space for a rect
    pub fn compute_rects(&self, term_size: (u16, u16)) -> Option<SplitMap> {
        self.root.compute_rects(
            Rect {
                pos: BufferPosition::new(0, 0),
                size: term_size.into(),
            },
            Self::MIN_SPLIT_SIZE,
            self.top_orientation,
        )
    }
}

pub struct BorderMap(pub(crate) Vec<Vec<BorderInfo>>);

impl BorderMap {
    pub fn new(size: Size) -> Self {
        Self(vec![vec![BorderInfo::default(); size.w as _]; size.h as _])
    }

    pub fn size(&self) -> Size {
        let h = self.0.len() as u16;
        let w = if h > 0 { self.0[0].len() as u16 } else { 0 };
        Size { w, h }
    }

    pub fn update(&mut self, inner_border_map: BorderMap, pos: BufferPosition) {
        let inner_size = inner_border_map.size();
        for row in 0..inner_size.h {
            for col in 0..inner_size.w {
                self.0[(row + pos.row) as usize][(col + pos.col) as usize] =
                    inner_border_map.0[row as usize][col as usize];
            }
        }
    }

    pub fn add_vertical(&mut self, pos: BufferPosition, len: u16) {
        for i in 0..len {
            self.0[(pos.row + i) as usize][pos.col as usize].in_vertical_border = true;
        }
    }

    pub fn add_horizontal(&mut self, pos: BufferPosition, len: u16) {
        for i in 0..len {
            self.0[(pos.row) as usize][(pos.col + i) as usize].in_horizontal_border = true;
        }
    }
}

#[derive(Debug, Default, Copy, Clone)]
pub struct BorderInfo {
    pub(crate) in_vertical_border: bool,
    pub(crate) in_horizontal_border: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum SplitSize {
    Proportion(u16),
    Fixed(u16),
}

#[derive(Constructor, Clone)]
pub struct Split {
    sizes: Vec<SplitSize>,
    content: Vec<SplitContent>,
}

impl Split {
    pub fn compute_rects(
        &self,
        rect: Rect,
        min_split_size: Size,
        orientation: Orientation,
    ) -> Option<SplitMap> {
        assert!(!self.sizes.is_empty(), "emtpy splits aren't allowed");

        let fixed_sizes = self
            .sizes
            .iter()
            .enumerate()
            .filter_map(|(i, x)| {
                if let SplitSize::Fixed(x) = x {
                    // the first elem in a split will have the specified size
                    // all others will have an extra separator
                    if i == 0 {
                        Some(*x)
                    } else {
                        Some(*x + 1)
                    }
                } else {
                    None
                }
            })
            .sum::<u16>();

        let sum_proportions = self
            .sizes
            .iter()
            .filter_map(|x| {
                if let SplitSize::Proportion(h) = x {
                    Some(h)
                } else {
                    None
                }
            })
            .sum::<u16>() as f32;

        let size_by_frac = |frac| match orientation {
            Orientation::Horizontal => rect
                .size
                .update_w(|w| ((w as f32 - fixed_sizes as f32) * frac) as u16),
            Orientation::Vertical => rect
                .size
                .update_h(|h| ((h as f32 - fixed_sizes as f32) * frac) as u16),
        };

        let position_by_offset = |offset| match orientation {
            Orientation::Horizontal => rect.pos.update_col(|c| c + offset),
            Orientation::Vertical => rect.pos.update_row(|r| r + offset),
        };

        // all but the first split will get an additional border.
        // This will happen later in the loop. The size in the relevant dimension will be reduces
        // by one, and the offset will be increased by one, if a border is required.
        // To make sure the splits have the sizes specified by the user, we need to add one
        // in the relevant dimension to all but the first split for all fixed sizes
        let split_sizes = {
            let head_split_size = match self.sizes[0] {
                SplitSize::Proportion(x) => size_by_frac(x as f32 / sum_proportions),
                SplitSize::Fixed(x) => match orientation {
                    Orientation::Horizontal => rect.size.with_w(x),
                    Orientation::Vertical => rect.size.with_h(x),
                },
            };

            let tail_split_sizes = self.sizes[1..].iter().map(|x| match x {
                SplitSize::Proportion(x) => size_by_frac(*x as f32 / sum_proportions),
                SplitSize::Fixed(x) => match orientation {
                    Orientation::Horizontal => rect.size.with_w(*x + 1),
                    Orientation::Vertical => rect.size.with_h(*x + 1),
                },
            });

            iter::once(head_split_size).chain(tail_split_sizes)
        };
        // Prepare a list of bools that will be zipped with the content in the next loop,
        // that tells us whether we're dealing with the last dynamically sized element in
        // the split.
        let mut is_last_dynamically_sized_elem = vec![false; self.sizes.len()];
        let i_last_dynamically_sized_elem_from_back = self
            .sizes
            .iter()
            .rev()
            .find_position(|x| matches!(**x, SplitSize::Proportion(_)));
        if let Some((i_from_back, _)) = i_last_dynamically_sized_elem_from_back {
            let i = is_last_dynamically_sized_elem.len() - 1 - i_from_back;
            is_last_dynamically_sized_elem[i] = true;
        }

        let is_fixed_size = self.sizes.iter().map(|x| match x {
            SplitSize::Proportion(_) => false,
            SplitSize::Fixed(_) => true,
        });

        // iter over content to compute the split rects
        let mut rects = HashMap::new();
        let mut border_map = BorderMap::new(rect.size);
        let mut current_offset = 0u16;
        let mut used_dynamic_space = 0u16;
        for (i, (content, mut elem_size, elem_is_last_dynamic_elem, elem_is_fixed_size)) in izip!(
            &self.content,
            split_sizes,
            is_last_dynamically_sized_elem,
            is_fixed_size
        )
        .enumerate()
        {
            let mut elem_pos = position_by_offset(current_offset);

            // because of how float to unsigned conversions work, the actual space used will be less or equal to
            // the available space, so if we're at the last element, we add the remaining space
            if elem_is_last_dynamic_elem {
                match orientation {
                    Orientation::Horizontal => {
                        let space_for_dynamic_buffers = rect.size.w - fixed_sizes;
                        let dead_space =
                            space_for_dynamic_buffers - used_dynamic_space - elem_size.w;
                        elem_size.w += dead_space;
                    }
                    Orientation::Vertical => {
                        let space_for_dynamic_buffers = rect.size.h - fixed_sizes;
                        let dead_space =
                            space_for_dynamic_buffers - used_dynamic_space - elem_size.h;
                        elem_size.h += dead_space;
                    }
                }
            }

            // update offset depending on orientation
            let elem_offset = match orientation {
                Orientation::Horizontal => elem_size.w,
                Orientation::Vertical => elem_size.h,
            };
            current_offset += elem_offset;

            if !elem_is_fixed_size {
                used_dynamic_space += elem_offset;
            }

            // for all elems but the first we add a border between the current and the last elem
            // and cut of the first row/col of the current elem for that
            if i > 0 {
                match orientation {
                    Orientation::Horizontal => {
                        border_map.add_vertical(elem_pos, elem_size.h);
                        elem_pos.col += 1;
                        elem_size.w -= 1;
                    }
                    Orientation::Vertical => {
                        border_map.add_horizontal(elem_pos, elem_size.w);
                        elem_pos.row += 1;
                        elem_size.h -= 1;
                    }
                };
            }

            // make sure there is enought space for the elem
            if elem_size.w < min_split_size.w || elem_size.h < min_split_size.h {
                return None;
            }

            let rect = Rect {
                pos: elem_pos,
                size: elem_size,
            };

            // now we know the contents rect, so lets process the content
            match content {
                SplitContent::Leaf(buffer) => {
                    rects.insert(rect, buffer.clone());
                }
                SplitContent::Branch(next_split) => {
                    let SplitMap {
                        rects: inner_rects,
                        border_map: inner_border_map,
                    } = next_split.compute_rects(rect, min_split_size, orientation.flip())?;
                    border_map.update(inner_border_map, rect.pos);
                    rects.extend(inner_rects.into_iter())
                }
            }
        }

        Some(SplitMap { rects, border_map })
    }
}

#[derive(Clone)]
pub enum SplitContent {
    Leaf(BufferRef),
    Branch(Split),
}

/// Define a split tree
///
/// ```no_run
/// use ablet::{split_tree, Buffer};
///
/// let def_buffer = Buffer::new().into_ref();
///
/// let tree = split_tree! (
///     Vertical: {
///         2: {
///             1: def_buffer,
///             1: def_buffer,
///         },
///         1: def_buffer,
///     }
/// );
/// ```
#[macro_export]
macro_rules! split_tree {
    ($orientation:ident: { $($entries:tt)+}) => {{
        use $crate::*;
        use std::iter;
        SplitTree::new(
            split_tree!(@entries_to_split, $($entries)+),
            Orientation::$orientation
        )
    }};

    (@entries_to_split, $($entries:tt)+) => {
        Split::new(
            split_tree!(@entries_to_sizes, $($entries)+).collect(),
            split_tree!(@entries_to_contents, $($entries)+).collect()
        )
    };


    (@entries_to_sizes, ) => { iter::empty() };
    (@entries_to_contents, ) => { iter::empty() };

    (@entries_to_sizes, $fixed:literal ! : $content:tt, $($tail:tt)*) => {
        iter::once(SplitSize::Fixed($fixed)).chain(split_tree!(@entries_to_sizes, $($tail)*))
    };

    (@entries_to_sizes, $fixed:literal ! : $content:tt) => {
        iter::once(SplitSize::Fixed($fixed))
    };

    (@entries_to_sizes, $proportional:literal : $content:tt, $($tail:tt)*) => {
        iter::once(SplitSize::Proportion($proportional)).chain(split_tree!(@entries_to_sizes, $($tail)*))
    };

    (@entries_to_sizes, $proportional:literal : $content:tt) => {
        iter::once(SplitSize::Proportion($proportional))
    };

    (@entries_to_contents, $size:literal $(!)? : $buf_ref:ident, $($tail:tt)*) => {
        iter::once(SplitContent::Leaf($buf_ref.clone())).chain(split_tree!(@entries_to_contents, $($tail)*))
    };

    (@entries_to_contents, $size:literal $(!)? : $buf_ref:ident) => {
        iter::once(SplitContent::Leaf($buf_ref.clone()))
    };

    (@entries_to_contents, $size:literal $(!)? : { $($entries:tt)+ }, $($tail:tt)*) => {
        iter::once(SplitContent::Branch(split_tree!(@entries_to_split, $($entries)+))).chain(split_tree!(@entries_to_contents, $($tail)*))
    };

    (@entries_to_contents, $size:literal $(!)? : { $($entries:tt)+ }) => {
        iter::once(SplitContent::Branch(split_tree!(@entries_to_split, $($entries)+)))
    };
}

#[cfg(test)]
mod tests {

    use crate::{split_tree, Buffer};

    #[test]
    pub fn test_splits_valid() {
        let def_buffer = Buffer::new().into_ref();

        let tree = split_tree! (
            Vertical: {
                2: {
                    1: def_buffer,
                    1: def_buffer,
                },
                1: def_buffer,
            }
        );

        let Some(split_map) = tree.compute_rects((40, 40)) else {
            assert!(false, "unexpected None");
            return;
        };

        let mut rects = split_map.rects.keys().collect::<Vec<_>>();
        rects.sort_unstable();

        insta::assert_debug_snapshot!(rects);
    }
}
