use std::collections::HashMap;

use derive_more::Constructor;

use crate::{BufferPosition, BufferRef, Orientation, Rect, Size};

/// How window is subdivided into splits.
///
/// Split tree is n-ary (three side-by-side columns are one level in the tree).
///
/// Direction is implicit: `vec![Leaf, Leaf]` is vertical split, `vec![vec![Leaf, Leaf]]` is
/// horizontal
///
/// Splits are ephemeral --- there are no SplitRefs, you can get-set the whole tree at once.
#[derive(Constructor)]
pub struct SplitTree {
    root: Split,
    top_orientation: Orientation,
}

pub struct SplitMap {
    rects: HashMap<Rect, BufferRef>,
    border_map: BorderMap,
    size: Size,
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

pub struct BorderMap(Vec<Vec<BorderInfo>>);
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
    in_vertical_border: bool,
    in_horizontal_border: bool,
}

#[derive(Constructor)]
pub struct Split {
    proportions: Vec<u16>,
    content: Vec<SplitContent>,
}

impl Split {
    pub fn compute_rects(
        &self,
        rect: Rect,
        min_split_size: Size,
        orientation: Orientation,
    ) -> Option<SplitMap> {
        let sum = self.proportions.iter().sum::<u16>() as f32;
        let fractions = self.proportions.iter().map(|x| *x as f32 / sum);

        let size_by_frac = |frac| match orientation {
            Orientation::Horizontal => rect.size.update_w(|w| (w as f32 * frac) as u16),
            Orientation::Vertical => rect.size.update_h(|w| (w as f32 * frac) as u16),
        };

        let position_by_offset = |offset| match orientation {
            Orientation::Horizontal => rect.pos.update_col(|c| c + offset),
            Orientation::Vertical => rect.pos.update_row(|r| r + offset),
        };

        // iter over content to compute the split rects
        let mut rects = HashMap::new();
        let mut border_map = BorderMap::new(rect.size);
        let mut current_offset = 0u16;
        for (i, (content, fraction)) in self.content.iter().zip(fractions).enumerate() {
            let mut elem_size = size_by_frac(fraction);
            let mut elem_pos = position_by_offset(current_offset);

            // because of how float to unsigned conversions work, the actual space used will be less or equal to
            // the available space, so if we're at the last element, we add the remaining space
            if i == self.content.len() - 1 {
                match orientation {
                    Orientation::Horizontal => {
                        let dead_space = rect.size.w - (current_offset + elem_size.w);
                        elem_size.w += dead_space;
                    }
                    Orientation::Vertical => {
                        let dead_space = rect.size.h - (current_offset + elem_size.h);
                        elem_size.h += dead_space;
                    }
                }
            }

            // update offset depending on orientation
            current_offset += match orientation {
                Orientation::Horizontal => elem_size.w,
                Orientation::Vertical => elem_size.h,
            };

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
                        size: inner_size,
                    } = next_split.compute_rects(rect, min_split_size, orientation.flip())?;
                    border_map.update(inner_border_map, rect.pos);
                    rects.extend(inner_rects.into_iter())
                }
            }
        }

        Some(SplitMap {
            rects,
            border_map,
            size: rect.size,
        })
    }
}

pub enum SplitContent {
    Leaf(BufferRef),
    Branch(Split),
}

/// Define a split tree
///
/// ```no_run
/// use ablet::{split_tree, Ablet, BufferType};
///
/// let ablet = Ablet::new(BufferType::Raw);
/// let def_buffer = ablet.default_buffer_get();
/// let tree = split_tree! {
///     Vertical: {
///         2: {
///             1: def_buffer,
///             1: def_buffer,
///         },
///         1: def_buffer
///     }
/// };
/// ```
#[macro_export]
macro_rules! split_tree {
    ($orientation:ident: { $($proportion:literal : $content:tt),+ $(,)?}) => {{
        use $crate::*;
        SplitTree::new(
            Split::new(
                vec![$($proportion),+],
                vec![$(
                    split_tree!(@resolve_content, $content)
                ),+]
            ),
            Orientation::$orientation
        )
    }};

    (@resolve_content, $buf_ref:ident) => {
        SplitContent::Leaf($buf_ref.clone())
    };

    (@resolve_content, { $($proportion:literal: $content:tt),+ $(,)?}) => {
        SplitContent::Branch(Split::new(
                vec![$($proportion),+],
                vec![$(
                    split_tree!(@resolve_content, $content)
                ),+]
        ))
    };
}

#[cfg(test)]
mod tests {

    use crate::{split_tree, Ablet};

    #[test]
    pub fn test_splits_valid() {
        let ablet = Ablet::new(crate::BufferType::Raw);
        let def_buffer = ablet.default_buffer_get();

        let tree = split_tree! {
            Vertical: {
                2: {
                    1: def_buffer,
                    1: def_buffer,
                },
                1: def_buffer
            }
        };

        let Some(split_map) = tree.compute_rects((40, 40)) else {
            assert!(false, "unexpected None");
            return;
        };

        let mut rects = split_map.rects.keys().collect::<Vec<_>>();
        rects.sort_unstable();

        insta::assert_debug_snapshot!(rects);
    }
}
