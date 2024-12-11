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

pub type SplitMap = HashMap<Rect, BufferRef>;

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
        let mut res = HashMap::new();
        let mut current_offset = 0u16;
        for (content, fraction) in self.content.iter().zip(fractions) {
            let size = size_by_frac(fraction);
            let pos = position_by_offset(current_offset);

            if size.w < min_split_size.w || size.h < min_split_size.h {
                return None;
            }

            // update offset depending on orientation
            current_offset += match orientation {
                Orientation::Horizontal => size.w,
                Orientation::Vertical => size.h,
            };

            let rect = Rect { pos, size };
            match content {
                SplitContent::Leaf(buffer) => {
                    res.insert(rect, buffer.clone());
                }
                SplitContent::Branch(next_split) => res.extend(
                    next_split
                        .compute_rects(rect, min_split_size, orientation.flip())?
                        .into_iter(),
                ),
            }
        }
        Some(res)
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

        let Some(rects) = tree.compute_rects((40, 40)) else {
            assert!(false, "unexpected None");
            return;
        };

        let mut rects = rects.keys().collect::<Vec<_>>();
        rects.sort_unstable();

        insta::assert_debug_snapshot!(rects);
    }
}
