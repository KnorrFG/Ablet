use ablet::{split_tree, with_setup_terminal, Ablet};
use crossterm::event::{Event, KeyCode, KeyEvent};

fn main() -> Result<(), ablet::SetupError<()>> {
    with_setup_terminal(run)
}

fn run() -> Result<(), ()> {
    let mut ablet = Ablet::new(ablet::BufferType::Raw);
    let out_doc = ablet.default_document_get();
    let def_buffer = ablet.default_buffer_get();
    out_doc.add_line("Hello World");

    let tree = split_tree! {
        Vertical: {
            2: {
                1: def_buffer,
                1: def_buffer,
            },
            1: def_buffer
        }
    };

    ablet.split_tree_set(tree);

    loop {
        ablet.render().unwrap();
        if let Event::Key(ke) = crossterm::event::read().unwrap() {
            if ke.code == KeyCode::Char('q') {
                return Ok(());
            }
        }
    }
}
