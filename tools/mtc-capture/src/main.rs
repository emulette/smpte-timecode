//! Records MIDI Time Code from an input port as fixture text.
//!
//! Usage: `cargo run --manifest-path tools/mtc-capture/Cargo.toml -- [PORT]`
//!
//! Without PORT, lists the input ports. With PORT, the first port whose name contains it is
//! recorded until Enter is pressed. Each line of standard output is one message: the backend
//! timestamp in microseconds, then the message bytes in hex. Other messages are left out.

use std::io::{self, BufRead, Write};
use std::process::ExitCode;
use std::sync::mpsc;

use midir::{Ignore, MidiInput};

fn is_time_code(message: &[u8]) -> bool {
    matches!(message, [0xF1, ..] | [0xF0, 0x7F, _, 0x01, ..])
}

fn main() -> ExitCode {
    let mut input = match MidiInput::new("mtc-capture") {
        Ok(input) => input,
        Err(error) => {
            eprintln!("cannot open MIDI input: {error}");
            return ExitCode::FAILURE;
        }
    };
    input.ignore(Ignore::ActiveSense);
    let ports = input.ports();
    let names: Vec<String> = ports
        .iter()
        .map(|port| input.port_name(port).unwrap_or_default())
        .collect();

    let Some(wanted) = std::env::args().nth(1) else {
        for name in &names {
            println!("{name}");
        }
        return ExitCode::SUCCESS;
    };
    let Some(index) = names.iter().position(|name| name.contains(&wanted)) else {
        eprintln!("no input port matches {wanted:?}");
        return ExitCode::FAILURE;
    };

    let (sender, receiver) = mpsc::channel::<(u64, Vec<u8>)>();
    let connection = input.connect(
        &ports[index],
        "mtc-capture",
        move |stamp, message, _| {
            if is_time_code(message) {
                // A closed channel only means recording has ended.
                let _ = sender.send((stamp, message.to_vec()));
            }
        },
        (),
    );
    let connection = match connection {
        Ok(connection) => connection,
        Err(error) => {
            eprintln!("cannot connect to {}: {error}", names[index]);
            return ExitCode::FAILURE;
        }
    };
    eprintln!("recording {}; press Enter to stop", names[index]);

    let writer = std::thread::spawn(move || -> io::Result<()> {
        let mut out = io::BufWriter::new(io::stdout().lock());
        for (stamp, message) in receiver {
            write!(out, "{stamp}")?;
            for byte in message {
                write!(out, " {byte:02X}")?;
            }
            writeln!(out)?;
        }
        out.flush()
    });

    let mut line = String::new();
    let _ = io::stdin().lock().read_line(&mut line);
    // Closing the connection drops the callback and its sender, which ends the writer.
    connection.close();
    match writer.join() {
        Ok(Ok(())) => ExitCode::SUCCESS,
        Ok(Err(error)) => {
            eprintln!("cannot write the capture: {error}");
            ExitCode::FAILURE
        }
        Err(_) => ExitCode::FAILURE,
    }
}
