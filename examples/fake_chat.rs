use ablet::{with_setup_terminal, Ablet};

fn main() -> Result<(), ablet::SetupError<()>> {
    with_setup_terminal(run)
}

fn run() -> Result<(), ()> {
    let ablet = Ablet::new(ablet::BufferType::Fancy);
    let out_doc = ablet.default_document_get();
    out_doc.add_line("Hello World");

    // next thing: render (as pub fn) +
    // a get_query fn that uses its own very simple line editor, updates the view and
    // listens for inputs in a loop

    // start thread that writes random message at random times into out buffer

    // loop that reads line from linput line, and also writes it into the outbuffer
    // then implement everything that's necessary to make this work
    Ok(())
}
