use std::{borrow::Cow, collections::HashMap, fmt::Display, ops::Index};

use crossterm::style::{ContentStyle, StyledContent};
use itertools::{enumerate, Itertools};

use crate::{Range, StyledRange};

#[derive(Default, Clone, Debug)]
pub struct AText {
    pub(crate) text: String,
    pub(crate) style_map: Vec<Option<usize>>,
    pub(crate) styles: Vec<crossterm::style::ContentStyle>,
}

impl AText {
    /// returns a list of pairs (range, style) that fall within the given
    /// range. Assumes self is a single line
    pub(crate) fn get_range_style_pairs(&self, r: Range<u16>) -> Vec<StyledRange<u16>> {
        let mut res = vec![];
        let mut start = r.start;
        let styles_in_range = self.style_map[r.into_native()].chunk_by(|a, b| a == b);
        for chunk in styles_in_range {
            let end = start + chunk.len() as u16;
            assert!(
                chunk.len() > 0,
                "unexpected zero-len chunk in get_range_style_pairs"
            );
            let style = chunk[0];
            res.push(StyledRange {
                style: if let Some(style) = style {
                    Cow::Borrowed(&self.styles[style])
                } else {
                    Cow::Owned(ContentStyle::default())
                },
                range: Range { start, end },
            });
            start = end;
        }
        res
    }

    /// replaces a part of the string with a new given string. Returns true if
    /// everything worked. If the range is not contained in the string, the new text
    /// will be appended
    pub fn replace_range<T: Into<AText>>(&mut self, r: std::ops::Range<usize>, new_text: T) {
        // * split into 3 parts: pre-range, range, post-range. The middle one might be
        //   empty, if the range is len 0. The left one might be empty, if the range
        //   is 0..0, and the right one may be empty if the range is len..len.
        // * concat pre-range with new text, and the result of that with post-range

        let mut new_text = new_text.into();
        if r.len() == 0 {
            if r.start == 0 {
                new_text += self.clone();
                *self = new_text;
            } else if r.start >= self.text.len() {
                self.append_text(new_text);
            } else {
                let (Some(l), Some(r)) = self.clone().split_at_index(r.start) else {
                    panic!("this should be impossible");
                };

                let mut res = l;
                res += new_text;
                res += r;
                *self = res;
            }
        } else {
            if r.start == 0 {
                new_text += self.clone();
                *self = new_text;
            } else if r.start >= self.text.len() {
                self.append_text(new_text);
            } else {
                let (Some(l), Some(_)) = self.clone().split_at_index(r.start) else {
                    panic!("this should be impossible");
                };

                let (_, mb_r) = self.clone().split_at_index(r.end);

                let mut res = l;
                res += new_text;
                if let Some(r) = mb_r {
                    res += r;
                }
                *self = res;
            }
        }
    }

    /// if index is 0, the result will be (None, Some(self)), if the index is
    /// greater or equal to len, it will be (Some(self), None), otherwise
    /// it will be (Some(left), Some(right))
    pub fn split_at_index(self, index: usize) -> (Option<AText>, Option<AText>) {
        if index == 0 {
            (None, Some(self))
        } else if index >= self.text.len() {
            (Some(self), None)
        } else {
            let AText {
                text,
                style_map,
                styles,
            } = self;
            let ltext = text[..index].to_string();
            let rtext = text[index..].to_string();
            let lstyle_map = style_map[..index].to_vec();
            let rstyle_map = style_map[index..].to_vec();
            let (lstyles, lstyle_mapping) = reduce_styles(&styles, &lstyle_map);
            let lstyle_map = lstyle_map
                .iter()
                .map(|opt_i| opt_i.map(|i| lstyle_mapping[&i]))
                .collect();
            let (rstyles, rstyle_mapping) = reduce_styles(&styles, &rstyle_map);
            let rstyle_map = rstyle_map
                .iter()
                .map(|opt_i| opt_i.map(|i| rstyle_mapping[&i]))
                .collect();

            let lres = AText {
                text: ltext,
                style_map: lstyle_map,
                styles: lstyles,
            };
            let rres = AText {
                text: rtext,
                style_map: rstyle_map,
                styles: rstyles,
            };
            (Some(lres), Some(rres))
        }
    }

    pub fn append_text<T: Into<AText>>(&mut self, other: T) {
        let AText {
            text: other_text,
            style_map: mut other_style_map,
            styles: other_styles,
        } = other.into();

        // check whether any of the styles of the new text are already in
        // this docs styles, if so, store the index
        let mut mapping = HashMap::new();
        for (other_index, other_style) in enumerate(other_styles) {
            if let Some((i, _)) = self
                .styles
                .iter()
                .find_position(|my_style| *my_style == &other_style)
            {
                mapping.insert(other_index, i);
            } else {
                mapping.insert(other_index, self.styles.len());
                self.styles.push(other_style);
            }
        }

        // update the new texts style map to point to the styles in this doc
        for si in &mut other_style_map {
            *si = si.map(|i| mapping[&i])
        }

        self.text.push_str(&other_text);
        self.style_map.append(&mut other_style_map);
    }

    pub fn push_char_formatted(&mut self, c: char, style: Option<ContentStyle>) {
        self.text.push(c);
        if let Some(style) = style {
            if let Some((i, _)) = self.styles.iter().find_position(|e| *e == &style) {
                self.style_map.push(Some(i));
            } else {
                self.style_map.push(Some(self.styles.len()));
                self.styles.push(style);
            }
        } else {
            self.style_map.push(None);
        }
    }

    pub fn push_char(&mut self, c: char) {
        self.push_char_formatted(c, None)
    }

    pub fn from_multiple<T: IntoIterator<Item = T2>, T2: Into<AText>>(elems: T) -> Self {
        let mut res = Self::default();
        for sc in elems {
            res.append_text(sc);
        }
        res
    }

    pub fn len(&self) -> usize {
        self.text.len()
    }
}

/// returns a new Style Vec that contains only those elements from styles that are in the new_style_map
/// as well as a mapping from index in styles to index in the new_styles
fn reduce_styles(
    styles: &[ContentStyle],
    new_style_map: &[Option<usize>],
) -> (Vec<ContentStyle>, HashMap<usize, usize>) {
    let remaining_styles = styles
        .iter()
        .enumerate()
        .filter(|(i, _)| new_style_map.contains(&Some(*i)));

    let mut mapping = HashMap::new();
    let mut new_styles = vec![];
    for (new_index, (old_index, style)) in enumerate(remaining_styles) {
        mapping.insert(old_index, new_index);
        new_styles.push(style.clone());
    }
    (new_styles, mapping)
}

impl From<&str> for AText {
    fn from(value: &str) -> Self {
        AText {
            text: value.into(),
            style_map: vec![None; value.len()],
            styles: vec![],
        }
    }
}

impl From<String> for AText {
    fn from(value: String) -> Self {
        let len = value.len();
        AText {
            text: value,
            style_map: vec![None; len],
            styles: vec![],
        }
    }
}

impl<T: Display> From<StyledContent<T>> for AText {
    fn from(value: StyledContent<T>) -> Self {
        let c = value.content().to_string();
        let len = c.len();
        AText {
            text: c,
            style_map: vec![Some(0); len],
            styles: vec![value.style().clone()],
        }
    }
}

impl<T: Into<AText>> std::ops::Add<T> for AText {
    type Output = Self;

    fn add(self, rhs: T) -> Self::Output {
        let mut res = self.clone();
        res.append_text(rhs);
        res
    }
}

impl<T: Into<AText>> std::ops::AddAssign<T> for AText {
    fn add_assign(&mut self, rhs: T) {
        self.append_text(rhs);
    }
}

#[cfg(test)]
mod tests {
    use crossterm::style::Stylize;

    use super::*;

    #[test]
    fn test_conversion() {
        insta::assert_debug_snapshot!(AText::from("foo"));
        insta::assert_debug_snapshot!(AText::from("foo".to_string()));
        insta::assert_debug_snapshot!(AText::from("foo".green()));
    }

    #[test]
    fn test_append_text_and_split_at() {
        let foo = AText::from("hello ") + "beautiful".green() + " " + "World".blue();
        insta::assert_debug_snapshot!(foo);

        let (l, r) = foo.split_at_index(8);
        insta::assert_debug_snapshot!(l);
        insta::assert_debug_snapshot!(r);
    }

    #[test]
    fn test_replace_range() {
        let mut foo = AText::from("Hello ") + "world".green();
        foo.replace_range(0..0, "Oh, ");
        insta::assert_debug_snapshot!(foo);

        foo.replace_range(foo.len()..foo.len(), "!");
        insta::assert_debug_snapshot!(foo);

        foo.replace_range(foo.len()..(foo.len() + 1), "!");
        insta::assert_debug_snapshot!(foo);

        foo.replace_range((foo.len() - 1)..foo.len(), "");
        insta::assert_debug_snapshot!(foo);

        foo.replace_range(9..15, "");
        insta::assert_debug_snapshot!(foo);
    }
}
