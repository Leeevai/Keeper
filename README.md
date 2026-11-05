# Keeper

Keeper is a terminal typing tutor ported from [keybr.com](https://www.keybr.com).

## Data from keybr.com

Keeper reuses the language models, word lists, books, keyboard layouts and
typing sounds of [keybr.com](https://github.com/aradzie/keybr.com), which is
licensed under the GNU Affero General Public License v3. The extracted files
live in `assets/` and are regenerated with:

```bash
git clone --depth 1 https://github.com/aradzie/keybr.com /tmp/keybr.com
node tools/extract-keybr-data.mjs /tmp/keybr.com
```

## License

AGPL-3.0, the same license as keybr.com. See [LICENSE](LICENSE).
