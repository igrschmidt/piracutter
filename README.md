# cookiecut

Turns a flat picture into two printable STL files: a cookie cutter and a
matching stamp that presses the artwork into the dough.

It is a desktop app with a live preview and a command line mode for batches.
Everything runs locally, nothing is uploaded, and there is no AI involved. The
outline is traced from the image and offset with a distance field, so the same
picture always gives the same geometry.

## What you get

Feed it a flat illustration with a plain background, like clipart or a logo.

- **Cutter** — a blade wall standing on a flange you press with your palm.
- **Stamp** — a plate that drops inside the cutter, carrying a raised outline
  and every dark line and spot from the picture.

Both parts are exported as single closed surfaces, so slicers take them
without a repair step.

## Install

Needs [Rust](https://rustup.rs). Same commands on macOS, Windows and Linux.

```
cargo build --release
```

The binary lands in `target/release/cookiecut` (`cookiecut.exe` on Windows).

On Linux you also need the usual windowing headers, for example
`libxkbcommon-dev libgtk-3-dev` on Debian and Ubuntu.

## Use

Double-click the binary, or run it with no arguments, to get the app. Open an
image or drop one on the window, adjust the sliders, then press Export STL.
Two files are written next to the name you choose, ending in `_cutter.stl` and
`_stamp.stl`. Settings persist between sessions, and presets save to JSON.

For batches:

```
cookiecut leopard.png -o leopard.stl
cookiecut leopard.png -o leopard.stl --size 65
cookiecut leopard.png -o leopard.stl --config party-set.json
```

## Settings worth knowing

**Cookie size** is measured across the artwork, not the image file, so empty
space around the picture does not shrink the result. Pick whether it applies
to the width, the height, or the longest side.

**Mirror geometry** is on by default and should stay on. A stamp is pressed
face down, so the printed part has to be the mirror of the picture for the
cookie to come out the right way round.

**Blade thickness** at 0.8 mm prints as two clean walls with a 0.4 mm nozzle.
**Detail thicken** matters most on busy artwork: raised lines thinner than
about 0.8 mm will not survive printing, so grow them until they do.

**Background** is read from the image's transparency when it has any, and
otherwise from the colour around the border. Raise the tolerance if parts of
the background survive, lower it if the artwork is being eaten.

If the status line mentions shapes being left out, some detail was too tangled
to turn into solid geometry. Raising the resolution, thickening the detail, or
raising the minimum detail area usually clears it.

## Printing

Print both parts flat on the bed with no supports. The cutter wants no top
solid layers and a couple of perimeters. Food-safe practice is the usual
advice for printed cutters: use a sealed or food-grade filament, wash by hand,
and treat them as single-occasion tools rather than dishwasher hardware.

## Tests

```
cargo test
```

The suite builds both parts across seventeen setting combinations and fails if
any exported surface is not closed.

## Licence

MIT.
