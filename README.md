# systray

`systray` provides a `tray` command for creating and managing Status Notifier tray items from shell scripts and lightweight automation.

The binary speaks to a small background daemon over a Unix socket. In normal use, you only call the CLI:

- `tray show` creates or updates an item
- `tray hide` removes an item by id
- `tray list` prints active item ids
- `tray run` shows an item while a command is running
- `tray daemon` runs the background daemon explicitly
- `tray -v` / `tray --version` prints the CLI version

## Features

- Auto-starts the daemon when `tray show` is called and no daemon is running
- Shuts the daemon down automatically after the last item is removed
- Accepts PNG and SVG icons
- Can associate items with a PID and remove them automatically when that process exits
- Supports click handlers via shell commands

## Requirements

- Linux desktop session with a working D-Bus session bus
- A tray host that supports the Status Notifier Item protocol
- Rust toolchain if building from source

## Build

```bash
cargo build --release
```

The release binary will be available at `target/release/tray`.

## Install

Install directly from the checked-out source:

```bash
cargo install --path . --locked
```

An Arch Linux `PKGBUILD` is also included for local packaging:

```bash
makepkg -si
```

## Usage

Create a tooltip-only item:

```bash
tray show --id example --tooltip "Example item"
```

Create an item with an icon and click handler:

```bash
tray show \
  --id sync \
  --icon /path/to/icon.svg \
  --tooltip "Sync in progress" \
  --on-click 'notify-send "sync clicked"'
```

Attach an item to an existing process:

```bash
tray show --id worker --tooltip "Worker running" --pid "$$"
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

Run a command with a temporary tray item:

```bash
tray run --icon media-record --id app -- sleep 50
```

Run a command and show elapsed time in the tray tooltip:

```bash
tray run --icon media-record --id app --duration -- sleep 50
```

## Behavior Notes

- `tray show` requires at least one of `--icon` or `--tooltip`
- `tray hide` returns a non-zero exit status if the id does not exist
- `tray list` does not start the daemon; when no daemon is running it exits successfully with no output
- `tray run` returns the same exit status as the wrapped command
- `tray run` forwards `SIGINT` and `SIGTERM` to the wrapped command and removes the tray item after exit
- `tray run --duration` updates elapsed time once per second (`HH:MM:SS`)
- when `tray run --duration` is used without `--tooltip`, the timer becomes the hover title; with `--tooltip`, the timer appears below the tooltip text
- `--on-click` is executed through `sh -lc`, so it must be treated as shell code
- The socket is created under `$XDG_RUNTIME_DIR` when available, otherwise under `/tmp/tray-<uid>/tray.sock`

## Man Page

Install the included man page manually if needed:

```bash
install -Dm644 tray.1 ~/.local/share/man/man1/tray.1
```

Then use:

```bash
man tray
```

## License

This project is available under the MIT License. See [LICENSE](LICENSE).
