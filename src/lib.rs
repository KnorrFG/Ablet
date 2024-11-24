use std::sync::{Arc, Mutex, RwLock};

use nonempty::{nonempty, NonEmpty};
use persistent_structs::PersistentStruct;
// User Access to the elems only goes through the handles.

type Shared<T> = Arc<Mutex<T>>;

fn shared<T>(t: T) -> Arc<Mutex<T>> {
    Arc::new(Mutex::new(t))
}

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
            split_tree: shared(SplitTree {
                root: Split {
                    proportion: 1.,
                    content: SplitContent::Leaf(default_buffer_ref),
                },
            }),
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
}

#[derive(Clone)]
pub struct BufferRef(Shared<Buffer>);

#[derive(Clone)]
pub struct DocumentRef(Shared<Document>);

impl DocumentRef {
    pub fn add_line<T: Into<AText>>(&self, t: T) {
        let AText {
            text,
            mut style_map,
            styles,
        } = t.into();
        let mut this = self.0.lock().unwrap();
        let my_styles = &mut this.content.styles;

        // check whether any of the styles of the new text are already in
        // this docs styles, if so, store the index
        let style_mapping = styles
            .iter()
            .map(|new_style| {
                my_styles.iter().enumerate().find_map(|(i, my_style)| {
                    if my_style == new_style {
                        Some(i)
                    } else {
                        None
                    }
                })
            })
            .collect::<Vec<_>>();

        // if a style is missing, add it to this docs style table, and insert its index.
        let style_mapping = style_mapping
            .iter()
            .enumerate()
            .map(|(entry_i, entry_mapping)| {
                if let Some(i) = entry_mapping {
                    *i
                } else {
                    let new_mapping = my_styles.len();
                    my_styles.push(styles[entry_i].clone());
                    new_mapping
                }
            })
            .collect::<Vec<_>>();

        // update the new texts style map to point to the styles in this doc
        for si in &mut style_map {
            *si = si.map(|i| style_mapping[i])
        }

        this.content.text.push_str(&text);
        this.content.style_map.append(&mut style_map);
        this.content.text.push('\n');
        this.content.style_map.push(None);
    }
}

/// Prompt is a special singleton split used for the primary interraction with the user.
/// Think command palette, `M-x`, or, indeed, shell's prompt. Maybe we want to display it at the
/// bottom, like in Emacs, or maybe we want to popup it front and center.
struct Prompt {
    buffer: BufferRef,
}

/// How window is subdivided into splits.
///
/// Split tree is n-ary (three side-by-side columns are one level in the tree).
///
/// Direction is implicit: `vec![Leaf, Leaf]` is vertical split, `vec![vec![Leaf, Leaf]]` is
/// horizontal
///
/// Splits are ephemeral --- there are no SplitRefs, you can get-set the whole tree at once.
struct SplitTree {
    root: Split,
}

struct Split {
    proportion: f32,
    content: SplitContent,
}

enum SplitContent {
    Leaf(BufferRef),
    Branch(Vec<Split>), // Even branches are v-splits, odd are h-splits.
}

/// A Buffer is its textual content plus extra state, notably, cursors.
/// Do cursors belong in the core model? I think so, they are the primary means of interaction.
/// Though, it's a bit hard to see how to make Vim vs Emacs bindings customizable without
/// hard-coding?
struct Buffer {
    document: DocumentRef,
    view: View,
}

enum View {
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
}

#[derive(Default)]
struct RawView {
    cursor: BufferPosition,
    selections: Vec<Selection<BufferPosition>>,
}

#[derive(Default)]
struct FancyView {
    selections: Vec<Selection<TextPosition>>,
    linewrap: bool,
    /// The offset is a character position in a documents text.
    /// It MUST point to the beginning of a line
    offset: usize,
    cursor: TextPosition,
}

struct Selection<T> {
    range: Range<T>,
}

struct Range<T> {
    start: T,
    end: T,
}

#[derive(Default)]
struct TextPosition(usize);

#[derive(Default)]
struct BufferPosition {
    row: usize,
    col: usize,
}

#[derive(Default)]
pub struct Document {
    content: AText,
}

#[derive(Default)]
pub struct AText {
    text: String,
    style_map: Vec<Option<usize>>,
    styles: Vec<termcolor::ColorSpec>,
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

mod termutils;
pub use termutils::{with_setup_terminal, SetupError};