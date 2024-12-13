use std::{
    io,
    sync::mpsc::{self, Receiver, TryRecvError},
    thread::{self, sleep},
    time::{self, Duration, Instant},
};

use ablet::{
    split_tree, with_setup_terminal, AText, Ablet, SimpleLineHandler, SimpleLineHandlerResult,
};
use crossterm::event::{Event, KeyCode, KeyEvent};

fn main() -> Result<(), ablet::SetupError<io::Error>> {
    with_setup_terminal(run)
}

fn run() -> io::Result<()> {
    let mut ablet = Ablet::new();
    let def_buffer = ablet.default_buffer_get();
    let prompt_buffer = ablet.prompt_buffer_get();
    let prompt_doc = prompt_buffer.get_doc();

    let tree = split_tree! {
        Vertical: {
            1: def_buffer
        }
    };
    ablet.split_tree_set(tree);

    let (tx_kill, rx_kill) = mpsc::sync_channel::<()>(1);
    start_background_thread(ablet.clone(), rx_kill);

    let mut handler = SimpleLineHandler;
    loop {
        use SimpleLineHandlerResult::*;
        match ablet.edit_prompt(&mut handler)? {
            LineDone => {
                def_buffer.add_line(AText::from("> ") + prompt_doc.take());
            }
            Abort => {
                _ = tx_kill.send(());
                return Ok(());
            }
        }
    }
}

fn start_background_thread(ablet: ablet::Ablet, rx_kill: Receiver<()>) {
    thread::spawn(move || {
        let buf = ablet.default_buffer_get();
        let mut last_msg_ts = Instant::now();
        loop {
            sleep(Duration::from_millis(1));
            if matches!(rx_kill.try_recv(), Ok(()) | Err(TryRecvError::Disconnected)) {
                return;
            }

            let now = Instant::now();
            if now.duration_since(last_msg_ts) > Duration::from_secs(2) {
                buf.add_line(format!("Hello at {now:?}"));
                ablet.render().unwrap();
                last_msg_ts = now;
            }
        }
    });
}
