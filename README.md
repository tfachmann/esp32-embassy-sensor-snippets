# ESP32 Embassy Sensor Snippets

Some sample snippets for esp32 board, built with Rust (`esp-rs` + `embassy`).

References:
- https://github.com/embassy-rs/embassy
- https://github.com/esp-rs/esp-hal
- https://github.com/rust-embedded-community
- https://esp32.implrust.com/

## Compiling + Flashing

```sh
source ~/export-esp.sh  # assumes espup is installed
cargo run --bin led_internal_blink --release
```

For requirements, see below.

## Requirements

```sh
# get espup + espflash (a special Rust toolchain for esp32 by esp-rs)
pacman -S espup espflash

# get esp-generate (for this script)
cargo install esp-generate

# install espup (which must be sourced later)
espup install
```
