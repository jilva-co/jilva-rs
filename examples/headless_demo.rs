//! Deterministic terminal session without a real terminal or async runtime.
//!
//! Run `cargo run -p jilva --no-default-features --example headless_demo`.

use std::io;
use std::time::Duration;

use jilva::{Backend, OutputCommand, TerminalCapabilities, TestDoubleBackend};

fn main() -> io::Result<()> {
    let mut backend = TestDoubleBackend::new(TerminalCapabilities::conservative(0));
    backend.enter()?;
    let result = (|| {
        backend.submit(&[
            OutputCommand::move_to(0, 0),
            OutputCommand::print_segment("Jilva headless terminal session"),
        ])?;
        backend.flush()?;
        backend.script(TestDoubleBackend::key('q'));
        while backend.poll(Duration::ZERO)? {
            if let Some(event) = backend.read()? {
                println!("decoded input: {event:?}");
            }
        }
        println!("captured output: {:?}", backend.captured());
        Ok(())
    })();
    let restoration = backend.leave();
    result.and(restoration)
}
