use std::{
    borrow::Cow,
    collections::BTreeSet,
    io::{self, Write},
    sync::LazyLock,
};

use crossterm::{
    cursor, queue,
    style::{ContentStyle, PrintStyledContent, Stylize},
};
use persistent_structs::PersistentStruct;

use crate::{DocumentRef, Range, Rect, Shared, StyledRange, TextPosition};

const CURSOR_STYLE: LazyLock<ContentStyle> = LazyLock::new(|| ContentStyle::new().reverse());

#[derive(Clone)]
pub struct BufferRef(pub(crate) Shared<Buffer>);

impl BufferRef {
    pub fn render_at(&self, rect: Rect, cursor_visible: bool) -> io::Result<()> {
        let buffer = self.0.lock().unwrap();
        buffer.render_at(rect, cursor_visible)
    }
}
/// A Buffer is its textual content plus extra state, notably, cursors.
/// Do cursors belong in the core model? I think so, they are the primary means of interaction.
/// Though, it's a bit hard to see how to make Vim vs Emacs bindings customizable without
/// hard-coding?
pub struct Buffer {
    pub(crate) document: DocumentRef,
    pub(crate) view: View,
}

impl Buffer {
    fn render_at(&self, rect: Rect, cursor_visible: bool) -> io::Result<()> {
        self.view.render_doc(&self.document, rect, cursor_visible);
        Ok(())
    }
}

pub enum View {
    Raw(RawView),
    Fancy(FancyView),
}

impl View {
    pub fn fancy() -> Self {
        Self::Fancy(FancyView::default())
    }

    pub fn raw() -> Self {
        Self::Raw(RawView::default())
    }

    fn render_doc(
        &self,
        document: &DocumentRef,
        rect: Rect,
        cursor_visible: bool,
    ) -> io::Result<()> {
        match self {
            View::Raw(v) => v.render_doc(document, rect, cursor_visible),
            View::Fancy(v) => todo!("unimplemented"),
        }
    }
}

#[derive(Default)]
struct RawView {
    cursor: BufferPosition,
    selections: Vec<Selection<BufferPosition>>,
}

impl RawView {
    fn render_doc(
        &self,
        document: &DocumentRef,
        rect: Rect,
        cursor_visible: bool,
    ) -> io::Result<()> {
        // * slice into lines, because they are relevant for visibility
        //   and for render slices
        // * check what is visible (because if its outside the buffers size,
        //    it's not)
        // * get render slices. A render slice is a consecutive block of text
        //   that can be rendered without changing the style. It's influenced
        //   by the style map, the selections and the cursor
        //
        // with slice, I don't mean the &[T]. I guess a range is good to represent it
        let doc_lock = document.0.lock().unwrap();
        let atext = &doc_lock.content;

        let ranges = get_line_ranges(&atext.text)
            .into_iter()
            // throw away the lines that are not in the view
            .take(rect.size.h as usize)
            .map(|r| r.shortened_to(rect.size.w as usize))
            // convert from Range<usize> to Range<u16>
            .map(|Range { start, end }| Range {
                start: start as u16,
                end: end as u16,
            })
            // after the next call we have lines on level 1 and segments with different styles
            // within one line.
            .map(|r| atext.get_range_style_pairs(r))
            // split the selections further if they overlap with a selection
            .enumerate()
            .map(|(i, line)| {
                // for each selection, get a simple range, which is the part of the selection
                // that is in the current line
                let line_selections: Vec<Range<u16>> = self
                    .selections
                    .iter()
                    .filter_map(|selection| to_line_range(selection, i, rect.size.w))
                    .collect();
                line.into_iter()
                    .flat_map(|segment| adjust_for_seletions(segment, &line_selections))
                    .collect::<Vec<StyledRange<u16>>>()
            });

        let mut stdout = io::stdout();
        for (i_line, line) in ranges.enumerate() {
            queue!(
                stdout,
                cursor::MoveTo(rect.pos.col, rect.pos.row + i_line as u16)
            )?;
            for (i_col, styled_range) in line.iter().enumerate() {
                // if we are at the cursor, print one char in cursor style, and the rest normally,
                // otherwise print everything normally
                // TODO this is wrong. This would work, if we went through the line by chars
                // but we go through the line by render segment. So we need to split the
                // segment that contains the cursor. This works in the special case, that the
                // cursor is at the beginning of a segment
                if cursor_visible
                    && self.cursor
                        == (BufferPosition {
                            col: i_col as u16,
                            row: i_line as u16,
                        })
                {
                    queue!(
                        stdout,
                        PrintStyledContent(
                            CURSOR_STYLE.apply(
                                &atext.text[styled_range.range.shortened_to(1).into_native()]
                            )
                        ),
                        PrintStyledContent(styled_range.style.apply(
                            &atext.text[styled_range.range.update_start(|s| s + 1).into_native()]
                        ))
                    )?;
                } else {
                    queue!(
                        stdout,
                        PrintStyledContent(
                            styled_range
                                .style
                                .apply(&atext.text[styled_range.range.into_native()])
                        )
                    )?;
                }
            }
        }
        Ok(())
    }
}

/// convert selection to simple range, which is the part of the selection
/// that is in the current line
fn to_line_range(selection: &Selection<BufferPosition>, i: usize, w: u16) -> Option<Range<u16>> {
    todo!()
}

fn adjust_for_seletions<'a>(
    mut segment: StyledRange<'a, u16>,
    selections: &[Range<u16>],
) -> Vec<StyledRange<'a, u16>> {
    // when there are multiple selections that might overlap with a range,
    // we must check for each selection, whether it overlaps, and if some
    // none overlapping part remains, that must be checked against all remaining
    // selections.
    // An implicit assumption here is that selections don't overlap

    if let [current_selection, selections @ ..] = selections {
        use crate::OverlapDescription::*;
        match segment.range.get_overlap_with(&current_selection) {
            // no overlap with the current selection, check the rest
            None => adjust_for_seletions(segment, selections),
            // complete overlap with a selection, no need to check remaining selections
            Complete => {
                *segment.style.to_mut() = segment.style.on_grey();
                vec![segment]
            }
            // remember overlap, and check the remaining unoverlapped space against
            // the remaining selections, sort in the end. Since we know the resulting
            // ranges won't overlap, it suffices to sort by range start
            Right { old, foreign } | Left { foreign, old } => {
                let mut found_selection = vec![StyledRange {
                    style: Cow::Owned(segment.style.on_grey()),
                    range: foreign,
                }];
                found_selection.extend(adjust_for_seletions(segment.with_range(old), selections));
                found_selection.sort_unstable_by(|a, b| a.range.start.cmp(&b.range.start));
                found_selection
            }
            Inner {
                old_l,
                foreign,
                old_r,
            } => {
                // same as above, but we need to check both free areas now
                let mut found_selection = vec![StyledRange {
                    style: Cow::Owned(segment.style.on_grey()),
                    range: foreign,
                }];
                found_selection.extend(adjust_for_seletions(
                    segment.clone().with_range(old_l),
                    selections.clone(),
                ));
                found_selection.extend(adjust_for_seletions(segment.with_range(old_r), selections));
                found_selection.sort_unstable_by(|a, b| a.range.start.cmp(&b.range.start));
                found_selection
            }
        }
    } else {
        vec![segment]
    }
}

fn get_line_ranges(text: &str) -> Vec<Range<usize>> {
    let lines = text.chars().filter(|c| *c == '\n').count() + 1;
    let mut res = Vec::with_capacity(lines);
    let mut current_line_start = 0;
    for (i, char) in text.chars().enumerate() {
        if char == '\n' {
            res.push(Range::new(current_line_start, i));
            current_line_start = i + 1;
        }
    }
    res.push(Range::new(current_line_start, text.len()));
    res
}

#[derive(Default)]
pub struct FancyView {
    selections: Vec<Selection<TextPosition>>,
    linewrap: bool,
    /// The offset is a character position in a documents text.
    /// It MUST point to the beginning of a line
    offset: usize,
    cursor: TextPosition,
}

#[derive(Default, Hash, Clone, Copy, PersistentStruct, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub struct BufferPosition {
    pub row: u16,
    pub col: u16,
}

impl BufferPosition {
    pub fn new(row: u16, col: u16) -> Self {
        Self { row, col }
    }
}

pub struct Selection<T> {
    range: Range<T>,
}
