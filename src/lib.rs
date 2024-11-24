use std::sync::{Arc, Mutex, RwLock};

use nonempty::{nonempty, NonEmpty};
use persistent_structs::PersistentStruct;
// User Access to the elems only goes through the handles.

// let ablet = Ablet::new(BufferType::Doc); // here, ablet is the module. This returns a buffer handle;
// let def_buffer = ablet.default_buffer();
// let input = def_buffer.parent().split_below(BufferType::QueryLine); // or with separator

#[derive(Clone)]
pub struct Ablet {
    state: Arc<RwLock<AbletState>>,
}

fn shared<T>(t: T) -> Arc<RwLock<T>> {
    Arc::new(RwLock::new(t))
}

impl Ablet {
    pub fn doc() -> Self {
        Self::new(AbletConfig {
            layout_direction: Direction::Vertical,
            default_buffer_type: BufferType::Doc,
        })
    }

    pub fn new(
        AbletConfig {
            layout_direction,
            default_buffer_type,
        }: AbletConfig,
    ) -> Self {
        Self {
            state: shared(AbletState {
                layout: shared(Layout {
                    elems: nonempty![LItem {
                        proportion: 1.,
                        elem: Elem::Buffer(shared(default_buffer_type.make()))
                    }],
                    direction: layout_direction,
                }),
            }),
        }
    }
}

#[derive(Default, PersistentStruct)]
pub struct AbletConfig {
    layout_direction: Direction,
    default_buffer_type: BufferType,
}

struct AbletState {
    layout: Arc<RwLock<Layout>>,
}

/// Represents a Row or a Col
struct Layout {
    elems: NonEmpty<LItem>,
    direction: Direction,
}

/// Layout Item, has it's data, and some meta info
/// that is relevant to the layout but not to the sub elems
struct LItem {
    proportion: f32,
    elem: Elem,
}

/// an elem is another layout, or a Buffer that displays
/// something, or a visual separator between two elems
#[derive(Clone)]
enum Elem {
    Layout(Arc<RwLock<Layout>>),
    Buffer(Arc<RwLock<Buffer>>),
    Separator(Arc<RwLock<Separator>>),
}

struct ElemHandleT<T> {
    parent: Option<Box<ElemHandleT<Layout>>>,
    elem: Arc<RwLock<T>>,
    instance: Ablet,
}

impl<T> Clone for ElemHandleT<T> {
    fn clone(&self) -> Self {
        Self {
            parent: self.parent.clone(),
            elem: self.elem.clone(),
            instance: self.instance.clone(),
        }
    }
}

pub trait ElemHandle {
    fn parent(&self) -> Option<ElemHandleT<Layout>>;
    fn parent_u(&self) -> ElemHandleT<Layout> {
        self.parent()
            .expect("called parent() on a top level layout")
    }

    fn as_layout(&self) -> Option<LayoutHandle>;
    fn as_layout_u(&self) -> LayoutHandle {
        self.as_layout()
            .expect("as_layout() called on non layout handle")
    }

    fn as_doc_buffer(&self) -> Option<DocBufferHandle>;
    fn as_doc_buffer_u(&self) -> DocBufferHandle {
        self.as_doc_buffer()
            .expect("as_doc_buffer() called on non doc-buffer handle")
    }

    fn as_raw_buffer(&self) -> Option<RawBufferHandle>;
    fn as_raw_buffer_u(&self) -> RawBufferHandle {
        self.as_raw_buffer()
            .expect("as_raw_buffer() called on non raw-buffer handle")
    }

    fn as_input_line_buffer(&self) -> Option<InputLineBufferHandle>;
    fn as_input_line_buffer_u(&self) -> InputLineBufferHandle {
        self.as_input_line_buffer()
            .expect("as_input_line_buffer() called on non non-input-line-buffer handle")
    }
}

pub type LayoutHandle = ElemHandleT<Layout>;
pub type DocBufferHandle = ElemHandleT<DocBuffer>;
pub type RawBufferHandle = ElemHandleT<RawBuffer>;
pub type InputLineBufferHandle = ElemHandleT<InputLineBuffer>;

/// The different type of Buffers
enum Buffer {
    Doc(DocBuffer),
    Raw(RawBuffer),
    ILine(InputLineBuffer),
}

/// Renders a document, supports append line, may later become an
/// editor, supports scrolling. Does not support view coordinates,
/// that is what a RawBuffer is for.
#[derive(Clone)]
struct DocBuffer {
    contents: Arc<RwLock<Doc>>,
    pre_rendered: termcolor::Buffer,
    width: usize,
    height: usize,
    line_wrap: bool,
    view_offset: usize,
}

impl Default for DocBuffer {
    fn default() -> Self {
        todo!()
    }
}
/// A doc to be rendered by a Doc buffer
///
/// The outer vec contains lines, the inner is there to allow for multiple
/// styles within a single line
#[derive(Clone, Default)]
pub struct Doc(Arc<RwLock<DocData>>);

/// Data of a Document
///
/// Represents text as Vec of lines. For each hyphene in the text, there
/// is an entry in the style map that says which style should be used.
#[derive(Default)]
pub struct DocData {
    lines: Vec<SingleLineString>,
    style_map: Vec<Vec<usize>>,
    styles: Vec<termcolor::ColorSpec>,
}

struct SingleLineString(String);

#[derive(Default)]
struct RawBuffer {
    contents: DocData,
}

#[derive(Default, Clone, Copy)]
enum Direction {
    Horizontal,
    #[default]
    Vertical,
}

#[derive(Default)]
struct InputLineBuffer(String);

struct Separator {
    sep_char: char,
}

#[derive(Default, Clone, Copy)]
pub enum BufferType {
    #[default]
    Doc,
    Raw,
    InputLine,
}

impl BufferType {
    fn make(&self) -> Buffer {
        match self {
            BufferType::Doc => Buffer::Doc(DocBuffer::default()),
            BufferType::Raw => Buffer::Raw(RawBuffer::default()),
            BufferType::InputLine => Buffer::ILine(InputLineBuffer::default()),
        }
    }
}
