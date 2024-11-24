use ablet::{Ablet, AbletConfig};

fn main() {
    let ablet = Ablet::doc();
    let out_buffer = ablet.get_default_buffer().doc();
    let input_line = out_buffer.parent().add(BufferType::InputLine).iline();

    // start thread that writes random message at random times into out buffer

    // loop that reads line from linput line, and also writes it into the outbuffer
    // then implement everything that's necessary to make this work
}
