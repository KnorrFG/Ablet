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
use itertools::Itertools;
use persistent_structs::PersistentStruct;

use crate::{shared, AText, Document, DocumentRef, Range, Rect, Shared, Size, StyledRange};

const CURSOR_STYLE: LazyLock<ContentStyle> = LazyLock::new(|| ContentStyle::new().reverse());

#[derive(Clone)]
pub struct BufferRef(pub(crate) Shared<Buffer>);

impl BufferRef {
    pub fn render_at(&self, rect: Rect) -> io::Result<()> {
        let buffer = self.0.lock().unwrap();
        buffer.render_at(rect)
    }

    pub fn insert_char_at_cursor(&self, c: char) {
        self.0.lock().unwrap().insert_char_at_cursor(c)
    }

    pub fn delete_char_before_cursor(&self) {
        self.0.lock().unwrap().delete_char_before_cursor()
    }

    pub fn insert_text_at_cursor(&self, text: impl Into<AText>) {
        self.0.lock().unwrap().insert_text_at_cursor(text)
    }

    pub fn get_doc(&self) -> DocumentRef {
        self.0.lock().unwrap().document.clone()
    }

    pub fn set_cursor_visible(&self, v: bool) {
        self.0.lock().unwrap().view.cursor_visible = v;
    }

    pub fn add_line(&self, t: impl Into<AText>) {
        self.0.lock().unwrap().add_line(t)
    }

    pub fn move_cursor_by(&self, offset: isize) {
        self.0.lock().unwrap().move_cursor_by(offset)
    }
}

pub struct Buffer {
    pub(crate) document: DocumentRef,
    pub(crate) view: View,
}

impl Buffer {
    pub fn move_cursor_by(&mut self, offset: isize) {
        let pos = self.view.cursor.0 as isize;
        self.view.cursor.0 = (pos + offset)
            .max(0)
            .min(self.document.0.lock().unwrap().content.len() as isize)
            as usize;
    }

    pub fn from_text(text: impl Into<AText>) -> Buffer {
        Self {
            document: Document::from_text(text).into_ref(),
            view: View::default(),
        }
    }

    pub fn from_doc(doc: DocumentRef) -> Buffer {
        Self {
            document: doc,
            view: View::default(),
        }
    }

    pub fn new() -> Buffer {
        Self {
            document: Document::new().into_ref(),
            view: View::default(),
        }
    }

    pub fn into_ref(self) -> BufferRef {
        BufferRef(shared(self))
    }

    pub fn render_at(&self, rect: Rect) -> io::Result<()> {
        self.view.render_doc(&self.document, rect)?;
        Ok(())
    }

    pub fn insert_char_at_cursor(&mut self, c: char) {
        self.view
            .insert_char_at_cursor(c, &mut self.document.0.lock().unwrap());
    }

    pub fn delete_char_before_cursor(&mut self) {
        self.view
            .delete_char_before_cursor(&mut self.document.0.lock().unwrap());
    }

    pub fn insert_text_at_cursor(&mut self, text: impl Into<AText>) {
        self.view
            .insert_text_at_cursor(text, &mut self.document.0.lock().unwrap())
    }

    pub fn scroll_down(&mut self) {
        if let Some(size) = self.view.last_rendered_size {
            let doc = self.document.0.lock().unwrap();
            let n_lines = doc.content.text.lines().count();
            self.view.offset = 0.max(n_lines as isize - size.h as isize) as usize;
        }
    }

    pub fn add_line(&mut self, t: impl Into<AText>) {
        self.document.add_line(t);
        self.scroll_down();
    }
}

impl View {
    fn render_doc(&self, document: &DocumentRef, rect: Rect) -> io::Result<()> {
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
            // throw away the lines that are before the viewable part
            .dropping(self.offset)
            // throw away the lines that are behind the viewable part
            .take(rect.size.h as usize)
            .map(|r| r.shortened_to(rect.size.w as usize))
            // after the next call we have lines on level 1 and segments with different styles
            // within one line.
            .map(|r| atext.get_range_style_pairs(r))
            // split the selections further if they overlap with a selection
            .enumerate()
            .map(|(i, line)| {
                // for each selection, get a simple range, which is the part of the selection
                // that is in the current line
                let line_selections: Vec<Range<usize>> = self
                    .selections
                    .iter()
                    .filter_map(|selection| to_line_range(selection, i, rect.size.w as usize))
                    .collect();
                line.into_iter()
                    .flat_map(|segment| adjust_for_seletions(segment, &line_selections))
                    .collect::<Vec<StyledRange<usize>>>()
            });

        let mut stdout = io::stdout();
        for (i_line, line) in ranges.enumerate() {
            queue!(
                stdout,
                cursor::MoveTo(rect.pos.col, rect.pos.row + i_line as u16)
            )?;
            for styled_range in line {
                // if we are at the cursor, print one char in cursor style, and the rest normally,
                // otherwise print everything normally
                if self.cursor_visible && styled_range.range.into_native().contains(&self.cursor.0)
                {
                    // render part before the cursor
                    let (pre_cursor_opt, Some(at_cursor)) =
                        styled_range.range.split_at_index(self.cursor.0)
                    else {
                        panic!("This should be impossible (because the cursor is in the range)");
                    };
                    if let Some(pre_cursor) = pre_cursor_opt {
                        queue!(
                            stdout,
                            PrintStyledContent(
                                styled_range
                                    .style
                                    .apply(&atext.text[pre_cursor.into_native()])
                            )
                        )?;
                    }

                    // make a cursor visible at line end, if it is on a new_line
                    // this might cause a rendering over a border if a line is max length
                    // and the cursor is at its end
                    let mut text_under_cursor =
                        &atext.text[at_cursor.shortened_to(1).into_native()];
                    if text_under_cursor == "\n" {
                        text_under_cursor = " \n";
                    }

                    queue!(
                        stdout,
                        PrintStyledContent(CURSOR_STYLE.apply(text_under_cursor)),
                        PrintStyledContent(
                            styled_range.style.apply(
                                &atext.text[at_cursor.update_start(|s| s + 1).into_native()]
                            )
                        )
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

        // if the cursor is at the end of the document, append a space to visualize it
        if self.cursor.0 >= atext.len() && self.cursor_visible {
            queue!(stdout, PrintStyledContent(CURSOR_STYLE.apply(" ")),)?;
        }
        Ok(())
    }

    fn insert_char_at_cursor(&mut self, c: char, doc: &mut Document) {
        let pos = self.cursor.0;
        doc.content.replace_range(pos..pos, c.to_string());
        self.cursor.0 += 1;
    }

    fn delete_char_before_cursor(&mut self, doc: &mut Document) {
        let pos = self.cursor.0;
        doc.content.replace_range((pos - 1)..pos, "");
        if pos > 0 {
            self.cursor.0 -= 1;
        }
    }

    pub fn insert_text_at_cursor(&mut self, text: impl Into<AText>, doc: &mut Document) {
        let pos = self.cursor.0;
        let atext = text.into();
        self.cursor.0 += atext.len();
        doc.content.replace_range(pos..pos, atext);
    }
}

/// convert selection to simple range, which is the part of the selection
/// that is in the current line
fn to_line_range(selection: &Selection<TextPosition>, i: usize, w: usize) -> Option<Range<usize>> {
    todo!()
}

fn adjust_for_seletions<'a>(
    mut segment: StyledRange<'a, usize>,
    selections: &[Range<usize>],
) -> Vec<StyledRange<'a, usize>> {
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
pub struct View {
    selections: Vec<Selection<TextPosition>>,
    // NOT supported yet
    // linewrap: bool,
    /// The offset is a character position in a documents text.
    /// It MUST point to the beginning of a line
    offset: usize,
    cursor: TextPosition,
    cursor_visible: bool,
    last_rendered_size: Option<Size>,
}

#[derive(Default)]
pub struct TextPosition(usize);

#[derive(Default, Hash, Clone, Copy, PersistentStruct, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub struct BufferPosition {
    pub row: u16,
    pub col: u16,
}

impl BufferPosition {
    pub fn to_text_pos(&self, doc: &Document) -> usize {
        let mut counter = 0usize;

        // if the cursor is not in the first lines, count the chars in the lines
        // before the cursor line
        if self.row > 0 {
            let mut n_lines_seen = 0;
            for c in doc.content.text.chars() {
                counter += 1;
                if c == '\n' {
                    n_lines_seen += 1;

                    if n_lines_seen == self.row {
                        break;
                    }
                }
            }
        }

        // now the count is at the beginning of the right line, and we only need to add the
        // col offset
        counter + self.col as usize
    }
}

impl BufferPosition {
    pub fn new(row: u16, col: u16) -> Self {
        Self { row, col }
    }
}

pub struct Selection<T> {
    range: Range<T>,
}
