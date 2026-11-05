# Keeper

Keeper is a typing tutor for the terminal, a port of [keybr.com](https://www.keybr.com).
It uses keybr's own algorithms and data: pronounceable pseudo-words generated
from the letters you know, letters unlocked one by one as each key reaches the
target speed, every key measured separately, and the same statistics.

```
 Keeper   F1 Practice   F2 Typing test   F3 Profile   F4 Settings   F5 Help

  Metrics:     Speed: 41.2wpm (+2.1wpm)   Accuracy: 97.3% (-0.4%)   Score: 1,234 (+56)
  All keys:    E N I A R L T O S U D Y C G H P M K B W F Z V X Q J
  Current key: T  Last speed: 32.1wpm (92%)  Top speed: 34.0wpm (97%)  Learning rate: +0.5wpm/lesson ▲
  Streaks:     3 lessons with 97% accuracy.
  Daily goal:  47%/30 minutes  █████████░░░░░░░░░░░

                 ╭ Guided lessons · English · United States ────────────────────────╮
                 │ terrain·tell·eat·tanner·eaten·tie·eat·nineteen·intent·tree·      │
                 │ elite·rate·alert·tile·nineteen·little·rat·rattle·tall            │
                 ╰──────────────────────────────────────────────────────────────────╯

                    `   1   2   3   4   5   6   7   8   9   0   -   =     ⌫
                     ⇥    Q   W   E   R   T   Y   U   I   O   P   [   ]    \
                     ⇪     A   S   D   F   G   H   J   K   L   ;   '     ⏎
                      ⇧      Z   X   C   V   B   N   M   ,   .   /      ⇧
```

In the terminal, keys are colored by confidence and by finger zone, and the key to press next is highlighted.

## Install

You need Rust 1.88 or newer.

```bash
cargo install --path .
```

Then run `keeper`. To build without typing sounds (and without the audio libraries), use
`cargo install --path . --no-default-features`.

## How it works

Guided lessons start with the six most frequent letters of your language. Words are
generated from a Markov chain of the language's letters, so they are pronounceable, and
every word contains the *current key*: the key you are slowest at. Each key's speed is
measured from the time it takes you to press it, smoothed over lessons. Keys are colored
from red to green as they approach the target speed (35 wpm by default). Once every
unlocked key has reached it, a new letter is unlocked. When all letters are unlocked,
raise the target speed and start over.

Speed is in words per minute, a word being five characters. Accuracy is the share of
characters typed without errors. The score grows with speed, accuracy, length and the
number of distinct keys. The numbers in brackets compare the last lesson with your
average.

## Screens

| Key | Screen | |
| --- | --- | --- |
| F1 | Practice | Adaptive lessons with live metrics, key confidence, accuracy streaks, daily goal and an on-screen keyboard. |
| F2 | Typing test | 15, 30 or 60 seconds, or 100, 500 or 1000 characters of common words, pseudo-words or a book, with a report that compares you with every keybr.com user. |
| F3 | Profile | All-time and today's statistics, the longest accuracy streaks, your speed and accuracy against other users, charts of speed, accuracy and keys per lesson, the learning progress and speed of every key, key frequency histograms and heatmaps, and a practice calendar. |
| F4 | Settings | Every option of keybr.com, with explanations and a live preview. |
| F5 | Help | How it works and the keys. |

## Keys

| Keys | |
| --- | --- |
| Tab, Shift+Tab, F1 to F5 | switch screens |
| Esc | restart the lesson; on other screens go back |
| Ctrl+N or Ctrl+→ | skip to a new lesson |
| Ctrl+V | normal, compact or bare practice view |
| Backspace | delete a wrong character (when the cursor does not stop on errors) |
| Ctrl+W, Alt+Backspace | delete the current word |
| Ctrl+D, Ctrl+T | typing test length and text |
| ↑ ↓, ← → | choose and change settings; scroll the profile and pick a key |
| Ctrl+Q or Ctrl+C | quit |

## Settings

The settings keep keybr's names, defaults and ranges.

- **Lessons**: guided lessons, common words, books (6 classics in 4 languages), custom
  text with its own editor, source code in 16 syntaxes (C/C++, C#, CSS, Go, HTML, Java,
  JavaScript, PHP, Laravel, Python, Regex, Rust, Shell, TypeScript) and numbers.
  Natural words, keyboard order, unlocking more letters, recovering keys, word list size,
  capitals, punctuation, repeated words, lesson length, target speed and daily goal.
- **Typing**: stop the cursor on errors, forgive errors, space skips words, cursor shape
  (block, box, line, underline), whitespace (none, bar, bullet), and sounds: off, errors
  only, keys only or all, with keybr's five sound themes and a volume.
- **Keyboard**: 34 languages, 100 layouts (QWERTY, Dvorak, Colemak, Colemak-DH, Workman,
  Canary, Graphite, BÉPO, Neo, and the national layouts), 11 keyboard geometries (ANSI,
  ISO, Korean, Brazilian, Japanese, and matrix), finger zones, colored keys and key
  highlighting.
- **Layout emulation**: practice a layout without changing your system settings. Keys
  you type on your system layout (US by default) are translated into the practiced
  layout by key position. Turn it off if your system already uses that layout.
- **Miscellaneous**: speed in words or characters per minute or second, nine themes
  (System, which uses your terminal's colors, and keybr's Light, Dark, Gray, Yellow,
  Garden, Coffee, Chocolate and Honey), 24-bit or 256 colors.

## Your data

Settings and results are stored in `~/Library/Application Support/keeper` on macOS and
`~/.local/share/keeper` on Linux, or in `$KEEPER_HOME` if set (`keeper path` prints it).
Every lesson is appended to `results.jsonl`, one JSON object per line, in the format
keybr.com uses when you download your data from its profile page.

```bash
keeper stats                    # a summary of your progress
keeper export results.json      # all results as a JSON array
keeper import typing-data.json  # results from Keeper or keybr.com
```

Importing your keybr.com history continues your progress where you left off.

## Not ported

Features that need a server (accounts, high scores, multiplayer) or a browser (fonts,
text size, smooth caret movement, custom theme designer) are not part of Keeper.

## Development

```bash
cargo test                      # unit tests and end-to-end tests in a pseudo terminal
cargo test -- --ignored         # plays sounds on the audio output
cargo clippy --all-targets
```

The end-to-end tests in `tests/e2e.rs` run the real binary in a pseudo terminal, type into
it and read the screen back through a terminal emulator.

| Path | |
| --- | --- |
| `src/phonetic.rs` | the Markov chain word generator |
| `src/lesson.rs` | lessons, key unlocking, focus and learning rate |
| `src/textinput.rs` | the typing state machine and statistics |
| `src/result.rs` | results, key statistics, streaks, daily goal |
| `src/keyboard.rs`, `src/language.rs` | layouts, geometries, languages |
| `src/app/` | the screens; `src/ui/` the keyboard and text widgets |
| `assets/` | data extracted from keybr.com by `tools/extract-keybr-data.mjs` |

## Data from keybr.com

Keeper reuses the language models, word lists, books, keyboard layouts, code grammars,
user speed distributions and typing sounds of
[keybr.com](https://github.com/aradzie/keybr.com) by Aliaksandr Radzivanovich, which is
licensed under the GNU Affero General Public License v3. The extracted files live in
`assets/` and are regenerated with:

```bash
git clone --depth 1 https://github.com/aradzie/keybr.com /tmp/keybr.com
node tools/extract-keybr-data.mjs /tmp/keybr.com
```

## License

AGPL-3.0, the same license as keybr.com. See [LICENSE](LICENSE).
