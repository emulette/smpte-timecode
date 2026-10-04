# Pro Tools MTC fixtures

MIDI bytes and CoreMIDI timestamps recorded on 2026-10-04 with `tools/mtc-capture`,
using Pro Tools Ultimate 2026.4.1 on macOS and an empty session. Session audio was
48 kHz, with no audio/video pull or sample offset. Generate MTC sent to
`IAC Driver Bus 1`. These fixtures contain MIDI messages only, with no audio or
Avid SDK material, and are covered by the repository's MIT OR Apache-2.0 license.

Each file is a contiguous excerpt of its recording. Original bytes and microsecond
timestamps are preserved. Repeated playback after the first three complete live-jump
cycles was removed; no synthetic messages or gaps were inserted.

## Recording procedure

For each `*-transport.txt` session rate:

1. Start MIDI capture while stopped at `00:00:59:00`, play across the minute boundary,
   then stop.
2. Locate to `01:02:03:04` while stopped, then play.
3. While playing, locate to `00:05:00:00`, then stop. For 29.97 DF and 30 DF the
   live target was `00:05:00;04`, a valid label.
4. End capture by pressing Enter in the capture tool.

Session rates were set and confirmed with PTSL; transport targets were confirmed in
Pro Tools. PTSL was used for setup only and is not part of the library or fixtures.

The longer 24 fps recording is represented by two independent contiguous excerpts:
`24-minute-boundary.txt` crosses the minute boundary; `24-locate-jump.txt` starts
with the last two complete pre-stop cycles and retains the restart and live jump.
Each excerpt is replayed with a fresh decoder. The redundant 29.97 play/stop recording
was removed because its behavior is covered by the 29.97 transport fixture.

## Expected behavior

`tests/pro_tools.rs` contains the literal raw positions and complete-run lengths read
from the recorded nibbles independently of the library. Tests compare encoder bytes,
decoder positions, partial-cycle recovery, rate hints and stale state after silence.

At the 25, 48 and 60 fps live jumps, piece 1 is followed by piece 0 and then piece 1
within the timeout. The decoder discards that ambiguous first cycle and resumes on
the next complete forward cycle. The original transition pieces are retained.

For 48, 50, 59.94 and 60 fps sessions, Pro Tools sends half-count MTC with codes
0, 1, 3 and 3 respectively. The decoder follows the received 24/25/30 fps rate when
the session hint has a different nominal count. A 59.94 hint therefore does not
restore a 29.97 stream rate. Lower-rate captures cover 23.976, 24, 25, 29.97 NDF/DF,
30 and 30 DF.

## Verification limits

These fixtures verify byte/position compatibility and the recorded transport transitions.
They do not independently measure physical latency, scheduling accuracy or audio/video
alignment. Reverse transport, midnight/ten-minute boundaries, other Pro Tools versions,
external MIDI hardware and the remaining session rates were not captured. No full-frame
SysEx was observed in these recordings; synthetic codec tests cover full frames.
