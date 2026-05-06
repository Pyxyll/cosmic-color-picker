# cosmic-color-picker

A color picker for COSMIC, hacked together because nothing else worked.

`hyprpicker` doesn't run because cosmic-comp doesn't expose `zwlr_screencopy_v1`. The portal's [`PickColor`](https://github.com/pop-os/xdg-desktop-portal-cosmic/blob/master/src/screenshot.rs) is a `// XXX implement` stub. So this is what I shipped while I waited.

Expect this to be **obsolete** the moment System76 fills in `xdg-desktop-portal-cosmic`'s color picker, which is the proper place for it. Until then, here we are.

## What it does

Trigger a hotkey, screen freezes, magnifier follows your cursor with a live hex readout, click to copy, Esc to cancel.

![demo](demo.gif)

## What it is

About 600 lines of Rust. It:

1. Shells out to `grim` for the screenshot
2. Opens a `wlr-layer-shell` overlay surface per monitor, each rendering its slice of the capture
3. Draws a magnifier circle at the cursor with an 8x zoom and a reticle, plus a hex pill below it
4. On click, samples the captured image at the cursor and pipes the hex to `wl-copy` and `notify-send`

There's a hand-rolled 5x7 pixel font for the hex digits because embedding a TTF for 17 glyphs felt silly.

## Caveats

- The magnifier doesn't appear until you move the mouse after triggering the hotkey. Cosmic doesn't fire `Pointer.Enter` for a fresh layer-shell surface, and seeding a default cursor position broke worse on multi-monitor.
- Capture is a frozen screenshot, not live frames. Animations stop while you're picking. This is the same as basically every other color picker.
- Tested on COSMIC Epoch alpha with a stacked dual-monitor setup. Other layouts probably work but were not validated.

## Dependencies

- `grim`
- `wl-clipboard`
- `libnotify`
- A Wayland compositor with `wlr-layer-shell` (COSMIC, Hyprland, Sway, river)

## Install

### Source (any distro)

```sh
git clone https://github.com/Pyxyll/cosmic-color-picker.git
cd cosmic-color-picker
cargo build --release
install -Dm0755 target/release/cosmic-color-picker ~/.local/bin/cosmic-color-picker
```

Make sure `~/.local/bin` is on `$PATH`.

### Arch / CachyOS / Manjaro

```sh
sudo pacman -S --needed rust grim wl-clipboard libnotify
```

then the source steps above.

### Fedora

```sh
sudo dnf install rust cargo grim wl-clipboard libnotify wayland-devel libxkbcommon-devel pkgconf-pkg-config
```

then the source steps above.

### Pop!_OS / Debian / Ubuntu

```sh
sudo apt install rustc cargo grim wl-clipboard libnotify-bin libwayland-dev libxkbcommon-dev pkg-config
```

then the source steps above.

## Bind a key

It's a one-shot command, bind however your compositor expects.

COSMIC: edit `~/.config/cosmic/com.system76.CosmicSettings.Shortcuts/v1/custom`:

```ron
(
    modifiers: [Super, Shift],
    key: "c",
): Spawn("/home/USER/.local/bin/cosmic-color-picker"),
```

Or use **COSMIC Settings, Keyboard, Custom Shortcuts**.

Hyprland:

```
bind = SUPER SHIFT, C, exec, cosmic-color-picker
```

Sway:

```
bindsym $mod+Shift+c exec cosmic-color-picker
```

## License

MIT.
