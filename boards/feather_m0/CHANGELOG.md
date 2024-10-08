# Unreleased

- Upgrade PACs to latest SVD and `svd2rust`:
  - All peripheral types are now `PascalCase`
  - All register field accessors are now methods instead of struct members
  - Members of the `Peripherals` struct are now `snake_case`
- update path of Cargo config
- Added 'winc' feature for Feather with a WINC1500 Wifi chip

# v0.14.0

- Implement `embedded-hal` `1.0` for GPIO, SPI, I2C, UART and fix examples
- Update the PACs to svd2rust 0.30.2.

# v0.13.0

- Replace homebrew time library with `fugit` (#672)

# v0.12.1

- Update to `atsamd-hal` version `0.15.1`
- Update .cargo/config

# v0.12.0

- Update `lib.rs` and examples to reflect removal of `v1` APIs and promotion of `v2` APIs
- Update `i2c_master` convenience function to use the new `sercom::v2::i2c` API
- Add an `i2c` example
- Make use of `bsp_peripherals`, `periph_alias` and `pin_alias` macros
- Updated to 2021 edition, updated dependencies, removed unused dependencies (#562)

# v0.11.0

- remove extraneous `embedded-hal` dependencies from BSPs
- cleanup `cortex_m` dependency
* move `usbd-x` crates used only in examples to `[dev-dependencies]`
* removed unnecessary dependency on `nb` and `panic_rtt` (#510)
* Bump `cortex-m`/`cortex-m-rt` dependencies to fix a build issue
- Update to use refactored SPI module (#467)

# v0.10.1

* Bump dependencies `rtic-monotonic` to `0.1.0-rc.1` and `cortex-m-rtic` to `0.6.0-rc.2`.

* Change Cargo feature resolver to `resolver = "2"`

---

Changelog tracking started at v0.10.0
