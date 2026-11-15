# Wallpaper picker: boxed HUD ribbon

## Intent

Replace the grid picker (`chrome/walls:picker`) with a new picker that looks like a TUI or game HUD. It is not a grid and not a copy of Caelestia or Ryoku. The user picks a wallpaper by its look and by its colors. The color filter shows only palettes that the user's own wallpapers have.

## Goals

- A ribbon of wallpapers: the current one is large, the ones to the sides get smaller and fainter.
- A palette filter that the scan builds from the real wallpapers. A palette shows only if at least one wallpaper is in it.
- A hex list and a theme preview for the current wallpaper.
- Preview only: the desktop does not change until Enter.
- The full layout uses the width of the 3440x1440 screen.
- Keyboard first. The mouse also works.

## Non-goals

- No downloads. The Wallhaven browser (`Mod+Ctrl+W`) stays the way to get new wallpapers.
- No changes to how the theme is made after Enter. The current hooks (`wall-static`, `ghostty-wallcolors`, Discord theme) run as they do now.

## Anti-goals

- A grid of tiles.
- Invented colors. Every color on screen comes from a real wallpaper.
- A palette that has zero wallpapers.
- Plain square chips, diamonds, or a fixed rainbow row.

## Approaches

1. **Noctalia Luau plugin plus the Rust `walls` tool (picked).** The `walls` tool already scans, caches thumbnails, and sorts by hue. Extend it to put out the real palette data. Replace `picker.luau` with the new layout. The panel then gets the shell theme, keyboard focus, and the compositor blur that other Noctalia panels have.
2. **Quickshell QML window,** like the power menu. Free layout and real overlays, but it is a second shell beside Noctalia, with its own blur rule and theme wiring.
3. **All in Luau.** Scan the colors inside the plugin. Too slow for 822 files, and it repeats work the Rust tool already does.

Approach 1 is the way the user's other wallpaper tools work, and it uses the cache that exists. Approach 2 is the fallback only if the plugin UI cannot draw the layout (see "Risks").

## Layout

All text is monospace. A thin box with labels set into its border, like a TUI window.

```
┌┤ WALL ├──────────────────────────────────────────┤ 212 walls · 173 still · 39 live ├┐
│ PALETTES        │      03    04 ◆    05                │ COLORS                     │
│ ▬▬▬▬ red     17 │   ▫▫  ┌──────────┐  ▫▫               │ ■ #010102 ━━━━━━━━━  88.6% │
│ ▬▬▬▬ orange  18 │  ▫▫▫  │  current │  ▫▫▫              │ ■ #0b9af4 ━          6.4%  │
│ ▬▬▬▬ sky     49 │   ▫▫  └──────────┘  ▫▫               │ ■ #183e5d             3.1% │
│ ...             │   anime-flcl-dark-blue_wallhaven-k9k  │ PREVIEW                    │
│ ▬▬▬▬ dark    55 │   1680x1050 · jpg · 150K · still      │ [bar + Starship prompt]    │
└┤ ←→ wallpaper · ↑↓ palette · ⏎ apply · esc ├──────────────────────────────┤ 04/49 ├┘
```

- **Top border:** `WALL` on the left. The totals on the right.
- **Left column, PALETTES:** one row for each palette that has wallpapers. A row has a strip of 5 real colors, the name, and the count. The selected row has a wider strip with a soft glow, a light tint of the accent color, and a 2px line at its left edge.
- **Center, the ribbon:** about 7 wallpapers at 3440 wide. The current one is the largest. The sides step down in size and opacity (about 65%, 45%, 30%). Each one has its number in the palette above it. The current one has white corner brackets and `◆` beside its number. Under the ribbon: the file name, then size, type, file size, and still or live.
- **Right column, COLORS:** the top colors of the current wallpaper. Each row has a swatch, the hex code, a bar for the share of the image, and the percent.
- **Right column, PREVIEW:** a small picture of the desktop in that theme. It shows the bar and a Starship prompt drawn over the wallpaper, in the wallpaper's real colors.
- **Bottom border:** key hints on the left. The position in the palette (`04/49`) on the right, in the accent color.

## Glass

The user picked option B from the glass check:

- The panel background is about 70% dark (`rgba(10,10,12,.70)` in the mockup).
- Medium blur. The compositor does the blur. The `noctalia-panel` layer rule in `umbriel/config.toml` already has `blur = true` and `blur_optimized = false`.
- The border lines are white at about 18% opacity, and the column dividers are white at about 10%.
- A soft drop shadow under the panel.

The browser mockup only imitates the blur. Check the real result on the screen and tune the opacity there.

## Color scan

Extend `nixos/tools/src/bin/walls.rs`. The cache, the thumbnails, and the stamp check stay as they are.

- **For each wallpaper,** store its top 5 colors with the share of the image for each one. Live wallpapers use one frame (the current `video_frame`).
- **Main group:** the hue group with the most weight (saturation times value). The groups now are red, orange, yellow, green, teal, sky, blue, purple, magenta, pink. `sky` (195 to 225) splits off from `blue` (225 to 255), because 49 of the user's wallpapers are in sky.
- **No-color wallpapers** (the current `mono` rule) split into `dark` and `light` by mean lightness (cutoff 0.4).
- **Group palette:** the 5 most common colors over all the wallpapers in the group, from their thumbnails. Leave out the padding color.
- **Output:** the current tab-separated lines, plus fields for the main group and the colors with shares. The plugin builds the palette rows from those. A group with zero wallpapers is never sent.
- **Tuning:** the first scan put some colorful wallpapers in `dark` or `light`. Tune the cutoff against the real set before the work is done.

Scan result on 2026-11-15 (still plus live): sky 49, orange 18, red 17, blue 11, purple 8, yellow 6, green 6, teal 5, pink 5, magenta 2, dark 55, light 30.

## Keys

| Key | Action |
|-----|--------|
| Left, Right | Move in the ribbon. The hex list and preview follow. |
| Up, Down | Move in the palette list. The ribbon shows only that palette. |
| Enter | Apply the current wallpaper and close. |
| Tab | Next of ALL, STILL, LIVE (still and live mode only). |
| Esc | Close. Nothing changes. |

The top of the palette list has an `all` row, so the user can see every wallpaper.

## Apply

The same calls the grid picker uses now:

- still: `noctalia msg wallpaper-set <path>`
- live: `wall-sites live-apply <path>`
- workshop: `wall-sites wpe-apply <id>`

## Modes and keybinds

The user decided this on 2026-11-15:

| Key | Opens |
|-----|-------|
| `Mod+Shift+W` | This picker with still and live wallpapers. |
| `Mod+Alt+W` | The same picker, same style, with Wallpaper Engine workshop items only (611 now). |
| `Mod+Ctrl+W` | The Wallhaven browser, as now, to get new wallpapers. |

- **Still and live switch:** in the `Mod+Shift+W` picker, the top border has three clickable labels: `ALL`, `STILL`, `LIVE`. The selected one is in the accent color. `Tab` moves to the next one (my call). The palette list, counts, and ribbon change to match.
- **Workshop mode:** the top border shows `WORKSHOP` in place of the switch. The scan makes palettes for the workshop items the same way, from their preview images.
- One plugin panel with a mode argument, as now (`panel-toggle chrome/walls:picker workshop`).
- Because the workshop keeps its own key, this work frees no keybind for the launcher.

## Left to the builder

- How to build the ribbon sizes, the corner brackets, and the palette strips with the plugin UI (`ui.row`, `ui.column`, `ui.box`, `ui.image`).
- The number of wallpapers in the ribbon at 3440 wide.
- The animation when the ribbon moves, if the plugin UI has one.

## Risks

- The plugin UI may have no overlay or opacity property. Then the corner brackets sit outside the image, not on it, and the side wallpapers fade by a dark box in place of opacity. If the layout cannot be built at all, go to approach 2.
- Panel opacity comes from Noctalia, not from the plugin. If the plugin cannot set its own background opacity, use the closest shell setting and tell the user what it changes.
