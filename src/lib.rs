use std::{
    borrow::Cow,
    io::{self},
    ops::Sub,
    sync::{Arc, Mutex},
};

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use derive_more::derive::Constructor;
use persistent_structs::PersistentStruct;

type Shared<T> = Arc<Mutex<T>>;

fn shared<T>(t: T) -> Arc<Mutex<T>> {
    Arc::new(Mutex::new(t))
}

#[derive(Hash, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Rect {
    pub pos: BufferPosition,
    pub size: Size,
}

impl Rect {
    pub fn new(row: u16, col: u16, w: u16, h: u16) -> Self {
        Self {
            pos: BufferPosition { row, col },
            size: Size { w, h },
        }
    }
}

pub fn rect(row: u16, col: u16, w: u16, h: u16) -> Rect {
    Rect::new(row, col, w, h)
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

pub trait RangeCompatibleNumber<T>: Copy + Sub<T, Output = T> + PartialOrd + Into<usize> {}

impl<T: Copy + Sub<T, Output = T> + PartialOrd + Into<usize>> RangeCompatibleNumber<T> for T {}

#[derive(Debug, Clone, Copy, PartialEq, PersistentStruct, Constructor)]
pub struct Range<T> {
    start: T,
    end: T,
}

impl<T: RangeCompatibleNumber<T>> Range<T> {
    pub fn split_at_index(self, v: T) -> (Option<Self>, Option<Self>) {
        if v.into() == 0 {
            (None, Some(self))
        } else if v >= self.end {
            (Some(self), None)
        } else {
            (
                Some(Self {
                    start: self.start,
                    end: v,
                }),
                Some(Self {
                    start: v,
                    end: self.end,
                }),
            )
        }
    }

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

impl<T: RangeCompatibleNumber<T>> From<std::ops::Range<T>> for Range<T> {
    fn from(value: std::ops::Range<T>) -> Self {
        Self {
            start: value.start,
            end: value.end,
        }
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

    /// they overlay so that the foreign range is overlapping
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

macro_rules! with_cleanup {
    (cleanup: $cleanup:block, code: $code:block) => {{
        #[allow(unused_mut)] // its a false positive warning
        let mut f = move || $code;
        let res = f();
        $cleanup;
        res
    }};
}

pub trait EventHandler<T> {
    fn handle(&mut self, ev: &Event, buf: &BufferRef) -> Option<T>;
}

pub struct SimpleLineHandler;

pub enum SimpleLineHandlerResult {
    LineDone,
    Abort,
}

impl EventHandler<SimpleLineHandlerResult> for SimpleLineHandler {
    fn handle(&mut self, ev: &Event, buf: &BufferRef) -> Option<SimpleLineHandlerResult> {
        match ev {
            Event::Key(ke) => match ke.code {
                KeyCode::Char('c') if ke.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Some(SimpleLineHandlerResult::Abort);
                }
                KeyCode::Char('a') if ke.modifiers.contains(KeyModifiers::CONTROL) => {
                    buf.move_cursor_to_line_start()
                }
                KeyCode::Char('e') if ke.modifiers.contains(KeyModifiers::CONTROL) => {
                    buf.move_cursor_to_line_end()
                }
                KeyCode::Char(c) => buf.insert_char_at_cursor(c),
                KeyCode::Backspace => buf.delete_char_before_cursor(),
                KeyCode::Left => buf.move_cursor_by(-1),
                KeyCode::Right => buf.move_cursor_by(1),
                KeyCode::Enter => return Some(SimpleLineHandlerResult::LineDone),
                _ => {}
            },
            Event::Paste(text) => buf.insert_text_at_cursor(text.as_str()),
            _ => {}
        }
        None
    }
}

/// runs a loop that renders the split, hands of events to the handler,
/// and returns when the handler returns Some(T)
pub fn edit_buffer<H: EventHandler<T>, T>(
    buf: &BufferRef,
    split_tree: &SplitTree,
    event_handler: &mut H,
) -> io::Result<T> {
    loop {
        split_tree.render()?;
        let ev = event::read()?;
        if let Some(res) = event_handler.handle(&ev, buf) {
            return Ok(res);
        }
    }
}

mod termutils;
pub use termutils::{with_setup_terminal, SetupError};

mod splittree;
pub use splittree::{Split, SplitContent, SplitSize, SplitTree};

mod document;
pub use document::{Document, DocumentRef};

mod buffer;
pub use buffer::{Buffer, BufferPosition, BufferRef};

mod atext;
pub use atext::AText;

/// crossterms event module, use this to get inputs
pub use crossterm::event as ctevent;
