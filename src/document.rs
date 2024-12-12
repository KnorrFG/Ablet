use crate::{AText, Shared};

#[derive(Default)]
pub struct Document {
    pub(crate) content: AText,
}

#[derive(Clone)]
pub struct DocumentRef(pub(crate) Shared<Document>);

impl DocumentRef {
    pub fn add_line<T: Into<AText>>(&self, t: T) {
        let mut this = self.0.lock().unwrap();
        this.content.append_text(t);
        this.content.push_char('\n');
    }
}
