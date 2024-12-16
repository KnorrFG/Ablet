use std::{
    io,
    sync::mpsc::{self, Receiver, TryRecvError},
    thread::{self, sleep},
    time::{Duration, Instant},
};

use ablet::{
    split_tree, with_setup_terminal, AText, Buffer, BufferRef, SimpleLineHandler,
    SimpleLineHandlerResult, SplitTree,
};
use crossterm::style::Stylize;

fn main() -> Result<(), ablet::SetupError<io::Error>> {
    with_setup_terminal(run)
}

fn run() -> io::Result<()> {
    let def_buffer = Buffer::new().into_ref();
    let prompt_buffer = Buffer::new().into_ref();
    let prompt_doc = prompt_buffer.get_doc();
    prompt_buffer.set_cursor_visible(true);

    let tree = split_tree! {
        Vertical: {
            1: def_buffer,
            1!: prompt_buffer,
        }
    };

    let (tx_kill, rx_kill) = mpsc::sync_channel::<()>(1);
    start_background_thread(tree.clone(), def_buffer.clone(), rx_kill);

    let mut handler = SimpleLineHandler;
    loop {
        use SimpleLineHandlerResult::*;
        match ablet::edit_buffer(&prompt_buffer, &tree, &mut handler)? {
            LineDone => {
                def_buffer.add_line(AText::from("> ".grey()) + prompt_doc.take());
            }
            Abort => {
                _ = tx_kill.send(());
                return Ok(());
            }
        }
    }
}

fn start_background_thread(splits: SplitTree, buf: BufferRef, rx_kill: Receiver<()>) {
    thread::spawn(move || {
        let mut last_msg_ts = Instant::now();
        loop {
            sleep(Duration::from_millis(1));
            if matches!(rx_kill.try_recv(), Ok(()) | Err(TryRecvError::Disconnected)) {
                return;
            }

            let now = Instant::now();
            if now.duration_since(last_msg_ts) > Duration::from_secs(2) {
                buf.add_line(AText::from("< ".green()) + "Hello at " + format!("{now:?}").yellow());
                ablet::render(&splits).unwrap();
                last_msg_ts = now;
            }
        }
    });
}
