use std::{
    borrow::Cow,
    ops::Sub,
    sync::{Arc, Mutex},
};

use crossterm::style::ContentStyle;
use derive_more::derive::Constructor;
use itertools::Itertools;
use persistent_structs::PersistentStruct;

type Shared<T> = Arc<Mutex<T>>;

fn shared<T>(t: T) -> Arc<Mutex<T>> {
    Arc::new(Mutex::new(t))
}

/// Prompt is a special singleton split used for the primary interraction with the user.
/// Think command palette, `M-x`, or, indeed, shell's prompt. Maybe we want to display it at the
/// bottom, like in Emacs, or maybe we want to popup it front and center.
pub struct Prompt {
    buffer: BufferRef,
}

#[derive(Hash, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Rect {
    pub pos: BufferPosition,
    pub size: Size,
}

#[derive(Hash, Clone, Copy, PersistentStruct, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub struct Size {
    pub w: u16,
    pub h: u16,
}

impl From<(u16, u16)> for Size {
    fn from((w, h): (u16, u16)) -> Self {
        Self { w, h }
    }
}

#[derive(Default)]
pub struct AText {
    pub(crate) text: String,
    pub(crate) style_map: Vec<Option<usize>>,
    pub(crate) styles: Vec<crossterm::style::ContentStyle>,
}

impl AText {
    /// returns a list of pairs (range, style) that fall within the given
    /// range
    fn get_range_style_pairs(&self, r: Range<u16>) -> Vec<StyledRange<u16>> {
        let mut res = vec![];
        let mut start = r.start;
        let styles_in_range = self.style_map[r.into_native()].chunk_by(|a, b| a == b);
        for chunk in styles_in_range {
            let end = start + chunk.len() as u16;
            assert!(
                chunk.len() > 0,
                "unexpected zero-len chunk in get_range_style_pairs"
            );
            let style = chunk[0];
            res.push(StyledRange {
                style: if let Some(style) = style {
                    Cow::Borrowed(&self.styles[style])
                } else {
                    Cow::Owned(ContentStyle::default())
                },
                range: Range { start, end },
            });
            start = end;
        }
        res
    }
}

impl<T: AsRef<str>> From<T> for AText {
    fn from(value: T) -> Self {
        let v = value.as_ref();
        AText {
            text: v.into(),
            style_map: vec![None; v.len()],
            styles: vec![],
        }
    }
}

pub enum BufferType {
    Raw,
    Fancy,
}

#[derive(Debug, Clone, Copy)]
pub enum Orientation {
    Horizontal,
    Vertical,
}

impl Orientation {
    pub fn flip(&self) -> Self {
        match self {
            Orientation::Horizontal => Self::Vertical,
            Orientation::Vertical => Self::Horizontal,
        }
    }
}

trait RangeCompatibleNumber<T>: Copy + Sub<T, Output = T> + PartialOrd + Into<usize> {}

impl<T: Copy + Sub<T, Output = T> + PartialOrd + Into<usize>> RangeCompatibleNumber<T> for T {}

#[derive(Debug, Clone, Copy, PartialEq, PersistentStruct, Constructor)]
pub struct Range<T> {
    start: T,
    end: T,
}

impl<T: RangeCompatibleNumber<T>> Range<T> {
    pub fn shortened_to(&self, w: T) -> Self {
        if self.len() > w {
            self.update_end(|e| e - (self.len() - w))
        } else {
            *self
        }
    }

    pub fn len(&self) -> T {
        self.end - self.start
    }

    pub fn into_native(self) -> std::ops::Range<usize> {
        self.start.into()..self.end.into()
    }

    pub fn get_overlap_with(&self, foreign: &Range<T>) -> OverlapDescription<T> {
        if foreign.start > self.end || self.start > foreign.end {
            return OverlapDescription::None;
        }

        if self.start < foreign.start {
            if foreign.end >= self.end {
                OverlapDescription::Right {
                    old: range(self.start, foreign.start),
                    foreign: range(foreign.start, self.end),
                }
            } else {
                OverlapDescription::Inner {
                    old_l: range(self.start, foreign.start),
                    foreign: range(foreign.start, foreign.end),
                    old_r: range(foreign.end, self.end),
                }
            }
        } else {
            // foreign.start <= self.start
            if foreign.end < self.end {
                OverlapDescription::Left {
                    foreign: range(self.start, foreign.end),
                    old: range(foreign.end, self.end),
                }
            } else {
                // foreign.end >= self.end
                OverlapDescription::Complete
            }
        }
    }

    pub fn overlaps(&self, foreign: &Range<T>) -> bool {
        self.get_overlap_with(foreign) != OverlapDescription::None
    }
}

pub fn range<T: RangeCompatibleNumber<T>>(start: T, end: T) -> Range<T> {
    Range { start, end }
}

/// Describes how to ranges overlay
#[derive(Debug, PartialEq)]
pub enum OverlapDescription<T> {
    None,
    Complete,

    /// they overlay so that the foreighn range is overlapping
    /// the left most part
    Left {
        foreign: Range<T>,
        old: Range<T>,
    },
    /// they overlay so that the foreighn range is overlapping
    /// the right most part
    Right {
        old: Range<T>,
        foreign: Range<T>,
    },
    /// they are overlapping so that the foreign range is in
    /// middle without touching borders
    Inner {
        old_l: Range<T>,
        foreign: Range<T>,
        old_r: Range<T>,
    },
}

#[derive(PersistentStruct, Clone)]
pub struct StyledRange<'a, T> {
    pub(crate) style: Cow<'a, crossterm::style::ContentStyle>,
    pub(crate) range: Range<T>,
}

#[derive(Default)]
pub struct TextPosition(usize);

mod termutils;
pub use termutils::{with_setup_terminal, SetupError};

mod splittree;
pub use splittree::{Split, SplitContent, SplitMap, SplitTree};

mod ablet_type;
pub use ablet_type::Ablet;

mod document;
pub use document::{Document, DocumentRef};

mod buffer;
pub use buffer::{Buffer, BufferPosition, BufferRef, View};
