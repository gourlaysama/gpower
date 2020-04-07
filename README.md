# GPower Tweaks
Tweak linux power management settings

- [x] Change USB autosuspend settings (on/off and idle delay)
- [ ] Change whether an USB device can wake itself up
- [ ] USB Port power control
  - `pm_qos_no_power_off`
  - show connection type (hardwired/hotplug/...)
  - warn if child devices are not set to autosuspend anyway
- [ ] PCI power management (autosuspend on/off, idle delay)
- [ ] PCI wakeup support

![example screenshot](doc/readme_screenshot.png)

## Install

GPower Tweaks is written in Rust, so you need a [Rust install] to build it. It compiles with
Rust 1.42 or newer.

GPower Tweaks requires GTK 3.22 or later and the corresponding development package to build (`gtk3-devel`
RHEL/Fedora, `libgtk-3-dev` on Debian/Ubuntu).

Build it from source with:

```sh
$ git clone https://github.com/gourlaysama/gpower-tweaks -b v0.3.0
$ cd gpower-tweaks
$ cargo build --release
```

And then run it with:

```sh
$ ./target/release/gpower-tweaks
```

#### License

<sub>
GPower Tweaks  is licensed under the <a href="COPYING">GPL General Public License v3.0 or later</a>.
</sub>
