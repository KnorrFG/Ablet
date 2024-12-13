use std::collections::HashMap;

use crossterm::event::Event;

use crate::DocumentRef;

/// Eddy is you friendly editor
pub struct Eddy {
    doc: DocumentRef,
}
