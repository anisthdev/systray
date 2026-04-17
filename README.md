# systray

`systray` is a small Rust helper for creating and managing system tray entries from shell scripts and lightweight automation.

It provides a single `tray` binary with these subcommands:

- `show` creates or updates a tray item
- `hide` removes a tray item by id
- `list` prints the ids of active tray items
- `daemon` runs the background daemon directly

## Behavior

- `tray show` starts the daemon on demand if it is not already running.
- The daemon exits automatically after the last tray item is removed.
- `tray hide` fails with a non-zero exit code if the id does not exist.
- `tray list` does not start the daemon. If the daemon is not running, it returns successfully with no output.

## Build

```bash
cargo build --release
```

## Install

This repository includes an Arch Linux `PKGBUILD` for local packaging from the checked-out source.

```bash
makepkg -si
```

## Usage

Show an item:

```bash
tray show --id example --tooltip "Example item"
```

Show an item with a local icon:

```bash
tray show --id example --icon /path/to/icon.png --tooltip "Example item"
```

Hide an item:

```bash
tray hide --id example
```

List active items:

```bash
tray list
```

Run the daemon explicitly:

```bash
tray daemon
```

## Notes

- `show` requires at least one of `--icon` or `--tooltip`.
- `on_click` is executed through `sh -lc`, so it should be treated as shell code.
- The daemon expects a working D-Bus session bus.
