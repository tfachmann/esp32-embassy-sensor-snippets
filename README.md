# ESP32 Sensor Tests

Some simple tests for my esp32, built with Rust (`esp-rs` + `embassy`).

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
