# smpte-timecode

smpte-timecode converts SMPTE timecode to and from frames, seconds and samples with exact
drop-frame handling, and decodes and encodes MIDI Time Code.

- Parse and format `HH:MM:SS:FF` and drop-frame `HH:MM:SS;FF` labels at 19 frame rates, from
  23.976 to 120 DF, with frame arithmetic that wraps at midnight.
- Convert positions to exact rational seconds, and to and from sample positions with an explicit
  rounding rule. NTSC rates such as 29.97 rarely land on a whole sample.
- Apply exact 0.1% and 4% pull-up and pull-down factors to frame rates and sample rates.
- Decode MIDI Time Code quarter frames and full frames into checked positions. A position is
  produced only from eight consecutive pieces in one direction, so locates, lost pieces and
  direction changes never mix two positions. Forward positions are compensated for the two-frame
  cycle latency, and the decoder reports its direction and lock state.
- Encode quarter-frame streams and full-frame messages.

The crate is `no_std`, does not allocate and has no dependencies. The minimum supported Rust
version is 1.85.

## Install

```toml
[dependencies]
smpte-timecode = "0.1.0"
```

## Example

```rust
use core::time::Duration;
use smpte_timecode::{FrameRate, MtcDecoder, Pull, Ratio, Rounding, Timecode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rate = FrameRate::FPS_29_97_DF;
    let hour = Timecode::parse("00:59:59;29", rate)?.wrapping_add_frames(1);
    assert_eq!(hour.to_string(), "01:00:00;00");

    // 01:00:00;00 is 172 799 827.2 samples at 48 kHz.
    let khz48 = Ratio::from_integer(48_000);
    assert_eq!(hour.to_samples(khz48, Rounding::Down)?, 172_799_827);

    // 48 kHz pulled down 0.1% is exactly 48 000 000/1001 Hz.
    let pulled = Pull::DOWN_0_1_PERCENT.apply(khz48).ok_or("pull overflow")?;
    assert_eq!((pulled.numer(), pulled.denom()), (48_000_000, 1001));

    // Feed complete MIDI messages with the time each arrived, on any one clock.
    let mut decoder = MtcDecoder::new();
    decoder.set_rate_hint(Some(rate));
    let full_frame = [0xF0, 0x7F, 0x7F, 0x01, 0x01, 0x41, 0x00, 0x00, 0x00, 0xF7];
    let event = decoder.feed(&full_frame, Duration::from_millis(5))?;
    assert_eq!(event.map(|event| event.timecode), Some(hour));
    Ok(())
}
```

## Scope and limits

- A `Timecode` is a position within one 24-hour day. Subframes are not represented.
- 29.97 DF and 59.94 DF skip 2 and 4 labels per minute as SMPTE ST 12-1 defines. 30 DF, 60 DF,
  119.88 DF and 120 DF apply the same rule with 2, 4, 8 and 8 labels, matching Pro Tools labels.
- MIDI Time Code rate codes name only 24, 25, 29.97 DF and 30. The encoder sends 23.976 as 24,
  29.97 as 30 and 30 DF as 29.97 DF, and cannot send 48 frames per second or faster. Give the
  decoder a rate hint, such as the DAW session rate, to recover the rates the code cannot name
  when their nominal frame counts match. Otherwise the decoder follows the stream rate.
- The decoder takes complete MIDI messages. It does not open ports, assemble SysEx from a raw
  byte stream, read user bits, MIDI Machine Control or MIDI Cueing, or estimate speed. The encoder
  builds bytes; the caller sends them at the interval it reports.
- LTC and VITC are not supported.

The [API reference](https://docs.rs/smpte-timecode) documents every type, and
[`llms.txt`](https://github.com/emulette/smpte-timecode/blob/main/llms.txt) summarizes the contract.

## Development

Use the toolchain in `rust-toolchain.toml` with rustfmt and Clippy. Dependency checks use
`cargo-deny`. The [Pro Tools capture notes](https://github.com/emulette/smpte-timecode/blob/main/tests/fixtures/pro-tools-2026.4.1/README.md)
document the recording procedure and verification limits.

```sh
cargo test --locked
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc --no-deps --locked
cargo deny --locked --all-features check licenses bans sources
```

## License

Licensed under either the [Apache License, Version 2.0](https://github.com/emulette/smpte-timecode/blob/main/LICENSE-APACHE)
or the [MIT license](https://github.com/emulette/smpte-timecode/blob/main/LICENSE-MIT), at your option.
Both license texts are included in the published crate.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
this project by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without
any additional terms or conditions.
