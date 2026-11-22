# install-chromedriver

A CLI tool that automatically installs the correct version of `chromedriver` to match the currently installed `google-chrome` on Linux.

## What it does

1. Detects the installed Chrome version via `google-chrome --version`.
2. Checks whether `chromedriver` is already installed and at the matching version.
3. If not, downloads the matching `chromedriver` zip from the Chrome for Testing public storage.
4. Falls back to the [Chrome for Testing API](https://googlechromelabs.github.io/chrome-for-testing/) to find the closest available version if an exact match is not published.
5. Kills any running `chromedriver` processes, then installs the binary to `/usr/local/bin/chromedriver`.

## Requirements

- Linux x86-64
- `google-chrome` installed and on `PATH`
- Write access to `/usr/local/bin` (run with `sudo` if needed)

## Build

```bash
cargo build --release
```

The binary is placed at `target/release/install-chromedriver`.

## Usage

Run directly (requires write access to `/usr/local/bin`):

```bash
sudo ./target/release/install-chromedriver
```

Or use the provided helper script:

```bash
sudo ./install-chromedriver.sh
```

## Notes

- When an exact chromedriver build for your Chrome version is not available, the tool selects the closest older patch release with the same major version and prompts for confirmation before proceeding.
- If a newer Chrome version is available via `apt`, the tool prints a reminder but does not upgrade Chrome automatically.
