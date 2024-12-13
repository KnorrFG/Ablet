use std::io::{self, Write};

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyModifiers},
    execute, queue,
    style::Print,
    terminal::{Clear, ClearType},
};
use itertools::enumerate;

use crate::{
    rect, shared, Buffer, BufferRef, BufferType, Document, DocumentRef, Orientation, Prompt, Rect,
    Shared, Split, SplitContent, SplitMap, SplitTree, View,
};

#[derive(Clone)]
pub struct Ablet {
    prompt: Shared<Prompt>,
    split_tree: Shared<SplitTree>,
    buffers: Vec<Shared<Buffer>>,
    documents: Vec<Shared<Document>>,
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
                KeyCode::Char(c) => buf.insert_char_at_cursor(c),
                KeyCode::Backspace => buf.delete_char_before_cursor(),
                KeyCode::Enter => return Some(SimpleLineHandlerResult::LineDone),
                _ => {}
            },
            Event::Paste(text) => buf.insert_text_at_cursor(text.as_str()),
            _ => {}
        }
        None
    }
}

impl Ablet {
    pub fn new() -> Self {
        let prompt_doc = shared(Document::default());
        let default_buffer_doc = shared(Document::default());
        let default_buffer_view = View::default();
        let prompt_buffer = shared(Buffer {
            document: DocumentRef(prompt_doc.clone()),
            view: View::default(),
        });
        let default_buffer = shared(Buffer {
            document: DocumentRef(default_buffer_doc.clone()),
            view: default_buffer_view,
        });
        let prompt_buffer_ref = BufferRef(prompt_buffer.clone());
        let default_buffer_ref = BufferRef(default_buffer.clone());

        Self {
            prompt: shared(Prompt {
                buffer: prompt_buffer_ref,
            }),
            split_tree: shared(SplitTree::new(
                Split::new(vec![1], vec![SplitContent::Leaf(default_buffer_ref)]),
                Orientation::Vertical,
            )),
            buffers: vec![prompt_buffer, default_buffer],
            documents: vec![prompt_doc, default_buffer_doc],
        }
    }

    pub fn default_buffer_get(&self) -> BufferRef {
        BufferRef(self.buffers[1].clone())
    }

    pub fn default_document_get(&self) -> DocumentRef {
        DocumentRef(self.documents[1].clone())
    }

    pub fn render(&self) -> io::Result<()> {
        let (term_w, term_h) = crossterm::terminal::size()?;

        queue!(io::stdout(), Clear(ClearType::All))?;
        let Some(SplitMap {
            rects,
            border_map,
            size,
        }) = self
            .split_tree
            .lock()
            .unwrap()
            .compute_rects((term_w, term_h - 2))
        else {
            return render_screen_too_small_info();
        };

        for (rect, buffer) in rects {
            buffer.render_at(rect)?;
        }

        let mut stdout = io::stdout();
        for (row_i, row) in enumerate(border_map.0) {
            for (col_i, field) in enumerate(row) {
                if field.in_vertical_border {
                    queue!(
                        stdout,
                        cursor::MoveTo(col_i as u16, row_i as u16),
                        Print("\u{2502}")
                    )?;
                } else if field.in_horizontal_border {
                    queue!(
                        stdout,
                        cursor::MoveTo(col_i as u16, row_i as u16),
                        Print("\u{2500}")
                    )?;
                }
            }
        }

        let prompt_serparator_line = format!("{:\u{2500}<1$}", "", term_w as usize);
        queue!(
            stdout,
            cursor::MoveTo(0, term_h - 2),
            Print(prompt_serparator_line),
        )?;

        self.prompt
            .lock()
            .unwrap()
            .buffer
            .render_at(rect(term_h - 1, 0, term_w, 1))?;
        stdout.flush()
    }

    pub fn split_tree_set(&mut self, tree: SplitTree) {
        self.split_tree = shared(tree);
    }

    pub fn prompt_buffer_get(&self) -> BufferRef {
        self.prompt.lock().unwrap().buffer.clone()
    }

    pub fn edit_prompt<H: EventHandler<T>, T>(&self, event_handler: &mut H) -> io::Result<T> {
        let buf = self.prompt_buffer_get();
        buf.set_cursor_visible(true);
        with_cleanup!(
            cleanup: {self.prompt_buffer_get().set_cursor_visible(false)},
            code: {
                loop {
                    self.render()?;
                    let ev = event::read()?;
                    if let Some(res) = event_handler.handle(&ev, &buf) {
                        return Ok(res);
                    }
                }
            }
        )
    }
}

fn render_screen_too_small_info() -> Result<(), io::Error> {
    execute!(
        io::stdout(),
        cursor::MoveTo(0, 0),
        Print("The terminal window is too small to render the ui, please enlarge")
    )
}
