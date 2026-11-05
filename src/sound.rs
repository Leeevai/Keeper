//! Typing sounds, ported from `@keybr/textinput-sounds`.

use crate::settings::{PlaySounds, SoundTheme};
use crate::textinput::Feedback;

/// What to play for a key press, if anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cue {
    Click,
    Blip,
}

/// Decides which sound a key press makes, the same way as keybr.com.
pub fn cue(mode: PlaySounds, feedback: Feedback) -> Option<Cue> {
    let typed = matches!(feedback, Feedback::Succeeded | Feedback::Recovered);
    match mode {
        PlaySounds::None => None,
        PlaySounds::All => Some(if typed { Cue::Click } else { Cue::Blip }),
        PlaySounds::ErrorsOnly => (!typed).then_some(Cue::Blip),
        PlaySounds::KeysOnly => typed.then_some(Cue::Click),
    }
}

/// The recorded samples of a sound theme.
pub fn samples(theme: SoundTheme) -> Vec<&'static [u8]> {
    macro_rules! files {
        ($($name:literal),*) => { vec![$(include_bytes!(concat!("../assets/sounds/", $name, ".mp3")).as_slice()),*] };
    }
    match theme {
        SoundTheme::Default => files!("default-click"),
        SoundTheme::Mechanical1 => files!(
            "mechanical1-click1",
            "mechanical1-click2",
            "mechanical1-click3",
            "mechanical1-click4",
            "mechanical1-click5",
            "mechanical1-click6",
            "mechanical1-click7",
            "mechanical1-click8",
            "mechanical1-click9"
        ),
        SoundTheme::Mechanical2 => files!(
            "mechanical2-click1",
            "mechanical2-click2",
            "mechanical2-click3",
            "mechanical2-click4",
            "mechanical2-click5"
        ),
        SoundTheme::Typewriter1 => files!(
            "typewriter1-click1",
            "typewriter1-click2",
            "typewriter1-click3",
            "typewriter1-click4",
            "typewriter1-click5",
            "typewriter1-click6",
            "typewriter1-click7",
            "typewriter1-click8",
            "typewriter1-click9"
        ),
        SoundTheme::Typewriter2 => files!(
            "typewriter2-click1",
            "typewriter2-click2",
            "typewriter2-click3",
            "typewriter2-click4",
            "typewriter2-click5"
        ),
    }
}

pub const BLIP: &[u8] = include_bytes!("../assets/sounds/blip.mp3");
