pub(crate) const NUM_CHANNELS: usize = 2;
pub(crate) const NUM_INSTRUMENTS: usize = 8;
pub(crate) const NUM_PATTERNS: usize = 10;

pub(crate) const MAX_OVERLAPPING_NOTES: usize = 8;

pub(crate) const HEADER_LENGTH: usize = 4;
pub(crate) const INSTRUMENT_LENGTH: usize = 0x1a0;
pub(crate) const FOOTER_LENGTH: usize = 1;
pub(crate) const SONG_LENGTH: usize =
    HEADER_LENGTH + INSTRUMENT_LENGTH * NUM_INSTRUMENTS + FOOTER_LENGTH;
pub(crate) const OSCILLATOR_LENGTH: usize = 6;

pub(crate) const SEQUENCE_LENGTH: usize = 48;
pub(crate) const PATTERN_LENGTH: usize = 32;
