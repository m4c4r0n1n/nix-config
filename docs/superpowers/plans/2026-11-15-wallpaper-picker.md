# Wallpaper Picker (Boxed HUD Ribbon) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the grid picker `chrome/walls:picker` with the boxed HUD ribbon from the spec, fed by real palette data from the `walls` tool.

**Architecture:** `walls index` (Rust) scans and caches as now. It also writes each wallpaper's main group, its top colors with shares, and the size and type data, plus one palette line for each group. The Luau panel reads that output and draws three columns (palettes, ribbon, colors and preview) in a translucent box. Enter applies through the commands the grid picker uses now.

**Tech Stack:** Rust (`nix-config-tools` crate, `image` 0.25), Noctalia 5.5.0 Luau plugin (`plugin_api = 43`, declarative `ui.*`).

**Spec:** `docs/superpowers/specs/2026-11-15-wallpaper-picker-design.md`

## Global Constraints

- No `git push`, no commits. The user does all git work. Make it live on the machine only.
- No code comments. No em dashes in any text. STE English in UI text.
- Tools are Rust, never Python.
- Every color on screen comes from a real wallpaper. Never show a group with zero wallpapers.
- Live paths: the binary goes to `~/.local/bin/walls`, which comes before `/run/current-system/sw/bin` in `PATH`. The plugin goes to `~/.local/share/noctalia/plugins/walls/`. Copy both to the repo (`nixos/tools/src/bin/walls.rs`, `home/.local/share/noctalia/plugins/walls/`).
- Build with `nix shell nixpkgs#cargo nixpkgs#rustc nixpkgs#gcc -c cargo ...` in `~/Projects/nix-config/nixos/tools`.
- Glass: panel fill `#0a0a0cb3` (70%), border lines `#ffffff2e`, dividers `#ffffff1a`, selected palette row fill `#6aa9d624`. Blur comes from the existing `noctalia-panel` layer rule.
- Keys: Left and Right move the ribbon. Up and Down move the palette list. Enter applies and closes. Esc closes with no change. Tab moves to the next of ALL, STILL, LIVE.
- Keybinds stay as they are: `Mod+Shift+W` is still and live, `Mod+Alt+W` is workshop (`panel-toggle chrome/walls:picker workshop`), and `Mod+Ctrl+W` is Wallhaven.

## Review Focus

- A wallpaper that does not decode (bad file, a video with no frame) must not stop the scan. Keep it with group `dark` and no colors. Test: `index_survives_bad_file` in Task 1.
- A palette that becomes empty after the ALL, STILL, LIVE switch must leave the list, and the selection must move to `all`. Test: manual check in Task 3, step 6.
- Old cache lines from the grid picker have the old field count. They must be scanned again, not read wrongly. Test: `old_cache_line_is_rescanned` in Task 1.
- A file name with tabs or new lines must not break the line format. Fields are already cleaned with `replace(['\t', '\n'], " ")`. Keep that for the new fields.
- Workshop mode with 611 items: the ribbon draws only the visible window (about 7), never all items.

---

### Task 1: Palette data in `walls`

**Files:**
- Modify: `nixos/tools/src/bin/walls.rs` (the `colors`, `bucket`, `process`, and `index` functions and the output)
- Test: a `#[cfg(test)] mod tests` block at the end of `walls.rs`

**Interfaces:**
- Produces, on stdout of `walls index <kinds...>`:
  - Entry lines: `entry\t<kind>\t<key>\t<thumb>\t<name>\t<group>\t<colors>\t<meta>`
    - `<group>` is one of `red orange yellow green teal sky blue purple magenta pink dark light`.
    - `<colors>` is up to 5 of `rrggbb:share`, comma separated, largest share first, with share as a percent and 1 decimal (`010102:88.6`).
    - `<meta>` is `WxH · ext · size`, for example `1680x1050 · jpg · 150K`.
  - Palette lines: `palette\t<scope>\t<group>\t<count>\t<hex,hex,hex,hex,hex>`. The scope is each kind asked for, plus `all` for all of them together. There is no line for a group with a count of 0.
- Cache: `~/.cache/walls/index.tsv` lines get the same new fields. Read a line only if it has the new field count. Otherwise scan the file again.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, Rgb, RgbImage};

    fn solid(w: u32, h: u32, parts: &[([u8; 3], u32)]) -> DynamicImage {
        let mut img = RgbImage::new(w, h);
        let mut x0 = 0;
        for (c, cols) in parts {
            for x in x0..x0 + cols { for y in 0..h { img.put_pixel(x, y, Rgb(*c)); } }
            x0 += cols;
        }
        DynamicImage::ImageRgb8(img)
    }

    #[test]
    fn sky_is_not_blue() {
        let img = solid(64, 36, &[([0x3d, 0x66, 0x99], 64)]);
        assert_eq!(analyze(&img).group, "sky");
        let img = solid(64, 36, &[([0x50, 0x50, 0xb0], 64)]);
        assert_eq!(analyze(&img).group, "blue");
    }

    #[test]
    fn grey_splits_dark_and_light() {
        assert_eq!(analyze(&solid(64, 36, &[([0x16, 0x16, 0x16], 64)])).group, "dark");
        assert_eq!(analyze(&solid(64, 36, &[([0xe6, 0xe5, 0xe6], 64)])).group, "light");
    }

    #[test]
    fn colors_have_shares_largest_first() {
        let a = analyze(&solid(64, 36, &[([0x01, 0x01, 0x02], 48), ([0x0b, 0x9a, 0xf4], 16)]));
        assert_eq!(a.colors[0].1 > a.colors[1].1, true);
        assert!((a.colors.iter().map(|c| c.1).sum::<f32>() - 100.0).abs() < 0.5);
        assert!(a.colors.len() <= 5);
    }

    #[test]
    fn palette_skips_empty_groups() {
        let lines = palette_lines("static", &[("sky", vec![([0x2d, 0x44, 0x5f], 60.0), ([0x10, 0x17, 0x20], 40.0)])]);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].starts_with("palette\tstatic\tsky\t1\t"));
    }

    #[test]
    fn old_cache_line_is_rescanned() {
        assert!(parse_cache_line("static\t/a.jpg\t1\t/t.jpg\tblue,red").is_none());
    }

    #[test]
    fn index_survives_bad_file() {
        let a = analyze_path("/nonexistent/broken.jpg");
        assert_eq!(a.group, "dark");
        assert!(a.colors.is_empty());
    }
}
```

- [ ] **Step 2: Run the tests and see them fail**

Run: `nix shell nixpkgs#cargo nixpkgs#rustc nixpkgs#gcc -c cargo test --bin walls`
Expected: compile errors for `analyze`, `palette_lines`, `parse_cache_line`, `analyze_path`.

- [ ] **Step 3: Implement**

- `struct Analysis { group: &'static str, colors: Vec<([u8; 3], f32)> }`
- `fn analyze(img: &DynamicImage) -> Analysis`: resize to 96x54. Quantize each pixel to 4 bits per channel and count. Merge each bin into its average color. Keep the 5 largest bins, as percents of all pixels (normalize to 100 over all bins, then keep the top 5). Group: the hue bucket with the largest `s * v` weight among pixels with `s > 0.2 && v > 0.15 && max - min > 0.08`. If that weight is under 6% of the pixel count, the group is `dark` when mean `v < 0.4`, else `light`.
- New buckets: red `<15 or >=345`, orange `<40`, yellow `<65`, green `<160`, teal `<195`, sky `<225`, blue `<255`, purple `<290`, magenta `<320`, pink otherwise.
- `fn analyze_path(path: &str) -> Analysis`: if `image::open` fails, return `dark` with no colors.
- `fn palette_lines(scope: &str, members: &[(&str, Vec<([u8; 3], f32)>)]) -> Vec<String>`: for each group, add up the shares of each color (4-bit quantized key) over its members, and take the top 5 average colors. Sort the groups by count, largest first.
- `fn parse_cache_line(line: &str) -> Option<CacheRow>`: accept only the new field count.
- `index`: write entry lines with the `entry` prefix, then palette lines for each kind asked for and for `all`. Meta: dimensions from the decoded image or frame, extension, file size from `fs::metadata` (`K` or `M`, as the mockup shows).

- [ ] **Step 4: Run the tests and see them pass**

Run: `nix shell nixpkgs#cargo nixpkgs#rustc nixpkgs#gcc -c cargo test --bin walls`
Expected: 6 passed.

- [ ] **Step 5: Install and run it on the real files**

Run: `cargo build --release --bin walls && install -m755 target/release/walls ~/.local/bin/walls && walls index static live | grep -c '^entry'` (in the same nix shell)
Expected: `212`. Then `walls index static live | grep '^palette	all'` shows only groups with a count greater than 0, and the counts add up to 212. Then `walls index wpe | grep -c '^entry'` gives the workshop count (611 at the time of writing).

- [ ] **Step 6: Check the groups by eye**

Make a contact sheet for each group from the thumbnails (`magick montage`) and look at it. If a lot of colorful wallpapers are in `dark` or `light`, lower the 6% cutoff and run Step 4 and Step 5 again. Write down the final cutoff.

### Task 2: The panel layout

**Files:**
- Modify: `~/.local/share/noctalia/plugins/walls/plugin.toml`
- Rewrite: `~/.local/share/noctalia/plugins/walls/picker.luau`

**Interfaces:**
- Consumes: the `walls index` lines from Task 1. Run `{"walls","index","static","live"}` or `{"walls","index","wpe"}` with `noctalia.runAsync`, as the current `load()` does.
- Panel argument: `workshop` gives workshop mode. No argument gives still and live mode. Keep how the current picker reads it.

- [ ] **Step 1: Manifest**

Set `width = "fill"`, `height = 600`, `placement = "floating"`, `position = "center"`, `keyboard_focus = "exclusive"`, `capture_keys = ["left", "right", "up", "down", "return", "escape", "tab"]`. If `"fill"` gives no side margin, use `width = 3300`.

- [ ] **Step 2: Parse and state**

State: `entries`, `palettes` (scope to list of groups), `scope` (`all`, `static`, `live`, or `wpe`), `group` (`all` or a group id), `pos` (index in the filtered list). `filtered()` gives the entries in `scope` and `group`, in index order.

- [ ] **Step 3: Draw**

The root is a `ui.column` with fill `#0a0a0cb3`. All labels use `fontFamily = "monospace"`.
- **Top and bottom border rows:** a `ui.row` of a short `ui.box` line, a label, a `flexGrow` line, a label, and a short line. The lines are `height = 1`, fill `#ffffff2e`. Top left is `┤ WALL ├`. In workshop mode it is `┤ WORKSHOP ├`. In still and live mode, the scope labels `ALL STILL LIVE` follow. They can be clicked, and the selected one is in the accent color. Top right: `┤ N walls · S still · L live ├`. Bottom left: the key hints. Bottom right: `┤ pos/total ├` in the accent color.
- **Side lines:** a 1px `ui.box` column at each side of the body row.
- **Left column (width 200):** `PALETTES`, then an `all` row, then one row for each palette in the scope. A row holds a `ui.row` of 5 `ui.box` (each `flexGrow = 1`, fill = hex) inside a `ui.row` with `radius = 2` and `clip = true`, then the name and the count. The selected row is a strip 84 wide and 12 high, the row fill is `#6aa9d624`, and a 2px accent box is at its left.
- **Center:** the ribbon is a `ui.row` with `justify = "center"` and `align = "center"`. It shows `pos-3 .. pos+3`, with image widths `70, 100, 140, 360, 140, 100, 70` times 1.6 at 3440 wide, opacities `.3, .45, .65, 1, .65, .45, .3`, and 16:9 heights. Above each image is its number (`%02d`), and `◆` after the current one. Corner brackets: four 14x14 `ui.box` with `borderWidth = 2` and border `#ffffff`. The column has no overlay, so place them with `translateX` and `translateY` on a zero-height row above and below the image. Under the ribbon: the name, then the meta.
- **Right column (width 300):** `COLORS` rows: a 14px swatch with `radius = 3`, the hex, a `ui.progress` (`progress = share/100`, `fill = hex`, `track = "#1c1c20"`, `height = 3`), and the percent. Then `PREVIEW`: a `ui.image` of the current thumbnail, 280x158. Under it, a bar row and a prompt row drawn from the current colors: the second color is the accent, and the darkest color is the background. The plugin UI has no overlay, so the bar and prompt sit as rows below the image, not on top of it.
- **Accent color:** the most saturated color in the current wallpaper's `colors`. If none, use `#c8c8c8`.

- [ ] **Step 4: Copy the files live and open the panel**

Run: `noctalia msg panel-toggle chrome/walls:picker`
Expected: the panel opens over the desktop with the three columns. Take a screenshot (`grim ~/.claude/jobs/6be390e4/tmp/p.png`) and compare it to the mockup `hud-glass.html` option B.

### Task 3: Keys, scope switch, apply

**Files:**
- Modify: `~/.local/share/noctalia/plugins/walls/picker.luau`

- [ ] **Step 1:** `onKey(chord, pressed)`: on press, Left and Right change `pos` and stop at the ends. Up and Down change `group` through `{"all", ...palettes in scope}` and set `pos = 1`. Tab cycles `all, static, live` (not in workshop mode), sets `group = "all"` and `pos = 1`. Return applies. Escape closes.
- [ ] **Step 2:** Clicks: a ribbon image sets `pos`, a palette row sets `group`, a scope label sets `scope`.
- [ ] **Step 3:** Apply: `static` runs `{"noctalia","msg","wallpaper-set",key}`. `live` runs `{"setsid","-f","wall-sites","live-apply",key}`. `wpe` runs `{"setsid","-f","wall-sites","wpe-apply",key}`. Then close the panel the way the current picker does.
- [ ] **Step 4:** Open the panel. Moving does not change the desktop. Enter changes it. Esc closes, and the desktop stays as it was.
- [ ] **Step 5:** Open with `Mod+Alt+W`. Check that `WORKSHOP` shows, that Tab does nothing, and that Enter applies a workshop item.
- [ ] **Step 6:** In LIVE scope, check that groups with no live wallpapers are not in the list, and that the selection goes back to `all` after Tab.

### Task 4: Tune the glass on the real screen

- [ ] **Step 1:** Open the panel over the current wallpaper and take a screenshot. If the text is hard to read, raise the panel fill alpha toward `cc`. If it looks like a flat panel, lower it toward `99`. Write down the final value.
- [ ] **Step 2:** If the panel shows no blur, check the panel namespace with the compositor and add it to the `noctalia-(...)` layer rule in `~/.config/umbriel/config.toml`.

### Task 5: Copy to the repo (no commit)

- [ ] **Step 1:** Copy `~/.local/share/noctalia/plugins/walls/*` to `~/Projects/nix-config/home/.local/share/noctalia/plugins/walls/`. `walls.rs` already lives in the repo.
- [ ] **Step 2:** Tell the user the command that makes the system binary match. It needs sudo: `sudo cp -r ~/Projects/nix-config/nixos/tools/src /etc/nixos/tools/ && sudo nixos-rebuild switch`. Until then, `~/.local/bin/walls` is the live binary.
- [ ] **Step 3:** Delete the job temp files and stop the mockup server.
