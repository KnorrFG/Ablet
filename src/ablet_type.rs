use std::io::{self, Write};

use crossterm::{
    cursor, execute, queue,
    style::Print,
    terminal::{Clear, ClearType},
};
use itertools::enumerate;

use crate::{
    shared, Buffer, BufferRef, BufferType, Document, DocumentRef, Orientation, Prompt, Shared,
    Split, SplitContent, SplitMap, SplitTree, View,
};

#[derive(Clone)]
pub struct Ablet {
    prompt: Shared<Prompt>,
    split_tree: Shared<SplitTree>,
    buffers: Vec<Shared<Buffer>>,
    documents: Vec<Shared<Document>>,
}

impl Ablet {
    pub fn new(buf_type: BufferType) -> Self {
        let prompt_doc = shared(Document::default());
        let default_buffer_doc = shared(Document::default());
        let default_buffer_view = match buf_type {
            BufferType::Raw => View::raw(),
            BufferType::Fancy => View::fancy(),
        };
        let prompt_buffer = shared(Buffer {
            document: DocumentRef(prompt_doc.clone()),
            view: View::fancy(),
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
        let term_size = crossterm::terminal::size()?;
        queue!(io::stdout(), Clear(ClearType::All))?;
        let Some(SplitMap {
            rects,
            border_map,
            size,
        }) = self.split_tree.lock().unwrap().compute_rects(term_size)
        else {
            return render_screen_too_small_info();
        };

        for (rect, buffer) in rects {
            buffer.render_at(rect, false)?;
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
        stdout.flush()
    }

    pub fn split_tree_set(&mut self, tree: SplitTree) {
        self.split_tree = shared(tree);
    }
}

fn render_screen_too_small_info() -> Result<(), io::Error> {
    execute!(
        io::stdout(),
        cursor::MoveTo(0, 0),
        Print("The terminal window is too small to render the ui, please enlarge")
    )
}
