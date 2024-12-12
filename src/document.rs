use crate::{AText, Shared};

#[derive(Default)]
pub struct Document {
    pub(crate) content: AText,
}

#[derive(Clone)]
pub struct DocumentRef(pub(crate) Shared<Document>);

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
