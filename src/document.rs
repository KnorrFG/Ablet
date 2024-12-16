use crate::{shared, AText, Shared};

#[derive(Default)]
pub struct Document {
    pub(crate) content: AText,
}

impl Document {
    pub fn from_text(text: impl Into<AText>) -> Document {
        Self {
            content: text.into(),
        }
    }

    pub fn new() -> Document {
        Self::from_text("")
    }

    pub fn into_ref(self) -> DocumentRef {
        DocumentRef(shared(self))
    }
}

#[derive(Clone)]
pub struct DocumentRef(pub(crate) Shared<Document>);

impl DocumentRef {
    pub fn add_line<T: Into<AText>>(&self, t: T) {
        let mut this = self.0.lock().unwrap();
        this.content.append_text(t);
        this.content.push_char('\n');
    }

    pub fn update_content<T>(&self, f: impl FnOnce(&mut AText) -> T) -> T {
        let mut this = self.0.lock().unwrap();
        f(&mut this.content)
    }

    pub fn take(&self) -> AText {
        self.update_content(|text| {
            let mut res = AText::default();
            std::mem::swap(&mut res, text);
            res
        })
    }
}
