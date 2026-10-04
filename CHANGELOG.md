# Changelog

All notable changes to smpte-timecode are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Releases follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-10-04

### Added

- `Timecode`, `Fields` and `FrameRate`: positions within one day at 19 frame rates from 23.976
  to 120 DF, label parsing and formatting with drop-frame rules, and frame arithmetic that wraps
  at midnight.
- Exact conversion between positions, rational seconds (`Ratio`) and sample positions with an
  explicit `Rounding`.
- `Pull` with exact 0.1% and 4% pull-up and pull-down factors for frame rates and sample rates.
- `MtcDecoder`: quarter-frame and full-frame decoding with cycle validation, direction, two-frame
  forward compensation, a rate hint for rates the MIDI Time Code rate code cannot name, and lock
  state on a caller-supplied clock. Malformed MTC messages discard partial cycles before decoding
  resumes, so fragments on either side of an error cannot form a mixed position.
- `MtcEncoder`: forward and reverse quarter-frame streams, full-frame messages and the exact
  quarter-frame interval.

[Unreleased]: https://github.com/emulette/smpte-timecode/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/emulette/smpte-timecode/releases/tag/v0.1.0
