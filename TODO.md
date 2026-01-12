# TODO

## UX Improvements

- [ ] Show AI thought process and findings during initial analysis
  - Currently hangs with no feedback for extended periods
  - Stream intermediate results as chunks are scored
  - Display reasoning/findings as they come in
  - Add progress bar showing chunk analysis progress

## Build & Install

- [ ] Add justfile for easy installation
  - `just install` - build and install to ~/.cargo/bin
  - `just build` - debug build
  - `just release` - release build
  - `just test` - run tests

## Keybindings

- [ ] Fix `/` to be search (vim-style), not next file
  - `/` should open search prompt
  - `n`/`N` for next/prev search result
  - Keep `]`/`[` for next/prev file (vim-like `]c`/`[c` pattern)
  - Or use `}`/`{` for next/prev file (vim paragraph motion)

- [ ] Configurable keybindings
  - Allow users to customize keybindings in crai.toml
  - Support vim, emacs, or custom presets
