local FONT = "Maple Mono NF"
local GLASS = "#0a0a0cb3"
local LINE = "#ffffff2e"
local DIVIDER = "#ffffff1a"
local TEXT = "#e6e6e6"
local DIM = "#9a9aa0"
local FAINT = "#5a5a60"
local BODY_H = 572
local LEFT_W = 260
local RIGHT_W = 360
local SIZES = { 640, 300, 220, 160, 120 }
local FADES = { 1, 0.65, 0.45, 0.3, 0.2 }
local ORDER = { "red", "orange", "yellow", "green", "teal", "sky", "blue", "purple", "magenta", "pink", "dark", "light" }

local mode = "local"
local entries = {}
local palettes = {}
local loading = false
local errorText = ""
local scope = "all"
local group = "all"
local pos = 1
local wanted = nil

local render

local function hexToRgb(hex)
  local v = tonumber(string.sub(hex, 1, 6), 16) or 0
  return bit32.rshift(v, 16), bit32.band(bit32.rshift(v, 8), 255), bit32.band(v, 255)
end

local function rgbToHex(r, g, b)
  return string.format("%02x%02x%02x", math.floor(r + 0.5), math.floor(g + 0.5), math.floor(b + 0.5))
end

local function satVal(hex)
  local r, g, b = hexToRgb(hex)
  local mx = math.max(r, g, b) / 255
  local mn = math.min(r, g, b) / 255
  local s = if mx == 0 then 0 else (mx - mn) / mx
  return s, mx
end

local function lighten(hex, amount)
  local r, g, b = hexToRgb(hex)
  return rgbToHex(r + (255 - r) * amount, g + (255 - g) * amount, b + (255 - b) * amount)
end

local function accentOf(entry)
  if entry == nil then
    return "#c8c8c8"
  end
  local best, score = nil, 0.12
  for _, c in entry.colors do
    local s, v = satVal(c.hex)
    if s * v > score then
      best, score = c.hex, s * v
    end
  end
  if best == nil then
    return "#c8c8c8"
  end
  local _, v = satVal(best)
  if v < 0.75 then
    best = lighten(best, 0.75 - v)
  end
  return "#" .. best
end

local function darkest(entry)
  local hex, low = "0a0a0c", 2
  if entry ~= nil then
    for _, c in entry.colors do
      local _, v = satVal(c.hex)
      if v < low then
        hex, low = c.hex, v
      end
    end
  end
  return "#" .. hex
end

local function scopes()
  if mode == "workshop" then
    return { "wpe" }
  end
  return { "all", "static", "live" }
end

local function filtered()
  local list = {}
  for _, e in entries do
    local inScope = scope == "all" or e.kind == scope
    if inScope and (group == "all" or e.group == group) then
      table.insert(list, e)
    end
  end
  return list
end

local function groupList()
  local list = { "all" }
  for _, p in palettes[scope] or {} do
    table.insert(list, p.group)
  end
  return list
end

local function clampPos()
  local n = #filtered()
  if pos > n then
    pos = n
  end
  if pos < 1 then
    pos = 1
  end
end

local function keepGroup()
  for _, g in groupList() do
    if g == group then
      return
    end
  end
  group = "all"
end

local function apply(item)
  if item == nil then
    return
  end
  if item.kind == "static" then
    noctalia.runAsync({ "noctalia", "msg", "wallpaper-set", item.key })
  elseif item.kind == "live" then
    noctalia.runAsync({ "setsid", "-f", "wall-sites", "live-apply", item.key })
  else
    noctalia.runAsync({ "setsid", "-f", "wall-sites", "wpe-apply", item.key })
  end
  panel.close()
end

local function parse(stdout)
  entries = {}
  palettes = {}
  for line in string.gmatch(stdout or "", "[^\n]+") do
    local f = string.split(line, "\t")
    if f[1] == "entry" and #f >= 8 then
      local colors = {}
      for part in string.gmatch(f[7], "[^,]+") do
        local hex, share = string.match(part, "^(%x+):([%d%.]+)$")
        if hex then
          table.insert(colors, { hex = hex, share = tonumber(share) or 0 })
        end
      end
      table.insert(entries, { kind = f[2], key = f[3], thumb = f[4], name = f[5], group = f[6], colors = colors, meta = f[8] })
    elseif f[1] == "palette" and #f >= 4 then
      local s = if f[2] == "wpe" then "wpe" else f[2]
      palettes[s] = palettes[s] or {}
      local hexes = {}
      for hex in string.gmatch(f[5] or "", "[^,]+") do
        table.insert(hexes, hex)
      end
      table.insert(palettes[s], { group = f[3], count = tonumber(f[4]) or 0, hexes = hexes })
    end
  end
end

local function seekWanted()
  if wanted == nil then
    return
  end
  for i, e in filtered() do
    if e.key == wanted then
      pos = i
    end
  end
  wanted = nil
end

local function load()
  loading = true
  errorText = ""
  render()
  local local_bin = noctalia.expandPath("~/.local/bin/walls")
  local walls = if noctalia.fileExists(local_bin) then local_bin else "walls"
  local cmd = if mode == "workshop" then { walls, "index", "wpe" } else { walls, "index", "static", "live" }
  noctalia.runAsync(cmd, function(result)
    loading = false
    if result == nil or result.exitCode ~= 0 then
      errorText = if result and result.stderr ~= "" then result.stderr else "The wallpaper scan failed."
      render()
      return
    end
    parse(result.stdout)
    keepGroup()
    seekWanted()
    clampPos()
    render()
  end, 600000)
end

local function label(text, props)
  local p = props or {}
  p.text = text
  p.fontFamily = FONT
  p.fontSize = p.fontSize or 13
  p.color = p.color or DIM
  return ui.label(p)
end

local function hline(width)
  if width then
    return ui.box({ width = width, height = 1, fill = LINE })
  end
  return ui.box({ flexGrow = 1, height = 1, fill = LINE })
end

local function border(left, right)
  local row = { hline(16) }
  for _, item in left do
    table.insert(row, item)
  end
  table.insert(row, hline())
  for _, item in right do
    table.insert(row, item)
  end
  table.insert(row, hline(16))
  return ui.row({ gap = 8, align = "center", height = 22 }, row)
end

local function strip(hexes, width, height)
  local cells = {}
  for _, hex in hexes do
    table.insert(cells, ui.box({ flexGrow = 1, height = height, fill = "#" .. hex }))
  end
  if #cells == 0 then
    table.insert(cells, ui.box({ flexGrow = 1, height = height, fill = "#202024" }))
  end
  return ui.row({ width = width, height = height, radius = 3, clip = true, gap = 0 }, cells)
end

local function allHexes()
  local hexes = {}
  for i, p in palettes[scope] or {} do
    if i <= 5 and #p.hexes > 0 then
      table.insert(hexes, p.hexes[math.min(#p.hexes, 4)])
    end
  end
  return hexes
end

local function totalIn(s)
  local n = 0
  for _, e in entries do
    if s == "all" or e.kind == s then
      n += 1
    end
  end
  return n
end

local function paletteColumn(accent)
  local rows = { label("PALETTES", { color = FAINT, fontSize = 12 }) }
  local list = { { group = "all", count = totalIn(scope), hexes = allHexes() } }
  for _, p in palettes[scope] or {} do
    table.insert(list, p)
  end
  for _, p in list do
    local on = p.group == group
    local children = {}
    if on then
      table.insert(children, ui.box({ width = 2, height = 30, fill = accent }))
    end
    table.insert(children, strip(p.hexes, if on then 96 else 72, if on then 12 else 8))
    table.insert(children, label(p.group, { color = if on then TEXT else DIM, flexGrow = 1 }))
    table.insert(children, label(tostring(p.count), { color = if on then accent else FAINT }))
    table.insert(rows, ui.row({
      key = "pal" .. p.group,
      gap = 12,
      align = "center",
      height = 30,
      paddingH = if on then 0 else 2,
      onClick = function()
        group = p.group
        pos = 1
        render()
      end,
    }, children))
  end
  return ui.column({ width = LEFT_W, gap = 4, paddingH = 18, paddingV = 14 }, rows)
end

local function bracket(top, right)
  local bar = ui.box({ width = 16, height = 2, fill = "#ffffff" })
  local leg = ui.box({ width = 2, height = 14, fill = "#ffffff" })
  local parts = if top then { bar, leg } else { leg, bar }
  return ui.column({ gap = 0, align = if right then "end" else "start", width = 16 }, parts)
end

local function ribbonItem(list, i, offset)
  local e = list[i]
  local step = math.abs(offset) + 1
  local w = SIZES[step]
  local h = math.floor(w * 9 / 16)
  local current = offset == 0
  local path = if current and e.kind == "static" then e.key else e.thumb
  local image = if path ~= "" then ui.image({ path = path, width = w, height = h, fit = "cover" }) else ui.box({ width = w, height = h, fill = "#16161a" })
  local number = string.format("%02d", i) .. (if current then " ◆" else "")
  local body = image
  if current then
    body = ui.column({ gap = 0, align = "center" }, {
      ui.row({ width = w + 16, justify = "space_between", translateY = 8 }, { bracket(true, false), bracket(true, true) }),
      image,
      ui.row({ width = w + 16, justify = "space_between", translateY = -8 }, { bracket(false, false), bracket(false, true) }),
    })
  end
  return ui.column({
    key = "rib" .. e.kind .. e.key,
    gap = 6,
    align = "center",
    opacity = FADES[step],
    onClick = function()
      pos = i
      render()
    end,
  }, {
    label(number, { color = if current then accentOf(e) else FAINT }),
    body,
  })
end

local function center(list, current)
  if loading and #entries == 0 then
    return ui.column({ flexGrow = 1, align = "center", justify = "center" }, { label("Scanning wallpapers.") })
  end
  if errorText ~= "" then
    return ui.column({ flexGrow = 1, align = "center", justify = "center" }, { label(errorText, { color = "#e5484d" }) })
  end
  if current == nil then
    return ui.column({ flexGrow = 1, align = "center", justify = "center" }, { label("No wallpapers here.") })
  end
  local items = {}
  for offset = -4, 4 do
    local i = pos + offset
    if list[i] ~= nil then
      table.insert(items, ribbonItem(list, i, offset))
    else
      local w = SIZES[math.abs(offset) + 1]
      table.insert(items, ui.column({ key = "gap" .. offset, gap = 6 }, { label(" "), ui.box({ width = w, height = math.floor(w * 9 / 16), fill = "#00000000" }) }))
    end
  end
  local kind = if current.kind == "static" then "still" elseif current.kind == "live" then "live" else "workshop"
  return ui.column({ flexGrow = 1, align = "center", justify = "center", gap = 18, clip = true }, {
    ui.row({ gap = 24, align = "center", justify = "center" }, items),
    label(current.name, { color = TEXT, fontSize = 15, maxLines = 1, maxWidth = 1400, textAlign = "center" }),
    label(current.meta .. " · " .. kind, { textAlign = "center" }),
  })
end

local function colorsColumn(current)
  local rows = { label("COLORS", { color = FAINT, fontSize = 12 }) }
  for _, c in if current then current.colors else {} do
    table.insert(rows, ui.row({ gap = 12, align = "center", height = 26 }, {
      ui.box({ width = 14, height = 14, radius = 3, fill = "#" .. c.hex, border = "#ffffff33", borderWidth = 1 }),
      label("#" .. c.hex, { color = TEXT }),
      ui.progress({ progress = math.min(1, c.share / 100), fill = "#" .. c.hex, track = "#1c1c20", height = 3, flexGrow = 1 }),
      label(string.format("%.1f%%", c.share), { width = 56 }),
    }))
  end
  table.insert(rows, ui.box({ height = 14, width = 1 }))
  table.insert(rows, label("PREVIEW", { color = FAINT, fontSize = 12 }))
  if current then
    local accent = accentOf(current)
    local bg = darkest(current)
    local second = if #current.colors >= 2 then "#" .. current.colors[2].hex else accent
    local path = if current.kind == "static" then current.key else current.thumb
    table.insert(rows, ui.column({ gap = 0, width = 320, height = 206, clip = true, border = "#ffffff22", borderWidth = 1 }, {
      ui.row({ gap = 5, align = "center", height = 16, paddingH = 6, fill = bg .. "e6" }, {
        ui.box({ width = 22, height = 6, radius = 3, fill = accent }),
        ui.box({ width = 12, height = 6, radius = 3, fill = second }),
        ui.box({ width = 12, height = 6, radius = 3, fill = DIVIDER }),
        ui.spacer({ flexGrow = 1 }),
      }),
      if path ~= "" then ui.image({ path = path, width = 320, height = 190, fit = "cover" }) else ui.box({ width = 320, height = 190, fill = bg }),
      ui.column({ gap = 4, width = 200, padding = 8, fill = bg .. "ee", border = accent, borderWidth = 1, radius = 4, translateX = 16, translateY = -170 }, {
        ui.row({ gap = 0, align = "center" }, {
          ui.row({ fill = second, paddingH = 4 }, { label("~", { fontSize = 10, color = "#f0f0f0" }) }),
          ui.box({ width = 4, height = 1 }),
          label("main", { fontSize = 10, color = accent }),
          label("  ls", { fontSize = 10, color = TEXT }),
        }),
        label("Projects  Pictures  notes.md", { fontSize = 10, color = accent }),
        ui.row({ gap = 6, align = "center" }, {
          label(" ~ ", { fontSize = 10, color = accent }),
          ui.box({ width = 6, height = 11, fill = accent }),
        }),
      }),
    }))
  end
  return ui.column({ width = RIGHT_W, gap = 4, paddingH = 18, paddingV = 14 }, rows)
end

local function scopeLabels(accent)
  local out = {}
  if mode == "workshop" then
    table.insert(out, label("┤ WORKSHOP ├", { color = TEXT }))
    return out
  end
  table.insert(out, label("┤ WALL ├", { color = TEXT }))
  for _, s in scopes() do
    local name = if s == "all" then "ALL" elseif s == "static" then "STILL" else "LIVE"
    table.insert(out, ui.row({
      key = "scope" .. s,
      paddingH = 4,
      onClick = function()
        scope = s
        keepGroup()
        pos = 1
        render()
      end,
    }, { label(name, { color = if s == scope then accent else FAINT }) }))
  end
  return out
end

render = function()
  local list = filtered()
  clampPos()
  local current = list[pos]
  local accent = accentOf(current)
  local counts
  if mode == "workshop" then
    counts = `┤ {totalIn("wpe")} items ├`
  else
    counts = `┤ {totalIn("all")} walls · {totalIn("static")} still · {totalIn("live")} live ├`
  end
  local hints = if mode == "workshop"
    then "┤ ←→ wallpaper · ↑↓ palette · ⏎ apply · esc ├"
    else "┤ ←→ wallpaper · ↑↓ palette · tab still/live · ⏎ apply · esc ├"
  local side = ui.box({ width = 1, height = BODY_H, fill = LINE })
  local divider = ui.box({ width = 1, height = BODY_H - 28, fill = DIVIDER })
  panel.render(ui.column({ flexGrow = 1, gap = 0, fill = GLASS, paddingH = 10, paddingV = 6 }, {
    border(scopeLabels(accent), { label(counts) }),
    ui.row({ gap = 0, align = "center", height = BODY_H }, {
      side,
      paletteColumn(accent),
      divider,
      center(list, current),
      divider,
      colorsColumn(current),
      side,
    }),
    border({ label(hints, { color = FAINT }) }, { label(`┤ {if #list > 0 then pos else 0}/{#list} ├`, { color = accent }) }),
  }))
end

local REPEAT_DELAY = 350
local REPEAT_EVERY = 55
local held = nil

local function stopRepeat()
  held = nil
  panel.setNeedsFrameTick(false)
end

local function step(chord)
  local list = filtered()
  if chord == "escape" then
    panel.close()
  elseif chord == "return" then
    apply(list[pos])
  elseif chord == "left" then
    pos = math.max(1, pos - 1)
    render()
  elseif chord == "right" then
    pos = math.min(#list, pos + 1)
    render()
  elseif chord == "up" or chord == "down" then
    local groups = groupList()
    local at = 1
    for i, g in groups do
      if g == group then
        at = i
      end
    end
    at = if chord == "up" then math.max(1, at - 1) else math.min(#groups, at + 1)
    group = groups[at]
    pos = 1
    render()
  elseif chord == "tab" and mode ~= "workshop" then
    local list2 = scopes()
    local at = 1
    for i, s in list2 do
      if s == scope then
        at = i
      end
    end
    scope = list2[at % #list2 + 1]
    keepGroup()
    pos = 1
    render()
  end
end

function onKey(chord, pressed)
  local repeats = chord == "left" or chord == "right" or chord == "up" or chord == "down"
  if not pressed then
    if held ~= nil and held.chord == chord then
      stopRepeat()
    end
    return
  end
  if held ~= nil and held.chord == chord then
    return
  end
  stopRepeat()
  step(chord)
  if repeats then
    held = { chord = chord, wait = REPEAT_DELAY }
    panel.setNeedsFrameTick(true)
  end
end

function onFrameTick(deltaMs)
  if held == nil then
    panel.setNeedsFrameTick(false)
    return
  end
  held.wait -= deltaMs
  while held ~= nil and held.wait <= 0 do
    held.wait += REPEAT_EVERY
    local before = `{pos}:{group}`
    step(held.chord)
    if before == `{pos}:{group}` then
      stopRepeat()
    end
  end
end

function onClose()
  stopRepeat()
end

function onOpen(context)
  local want = if context == "workshop" then "workshop" else "local"
  if want ~= mode then
    entries = {}
    palettes = {}
    mode = want
  end
  scope = if mode == "workshop" then "wpe" else "all"
  group = "all"
  pos = 1
  wanted = nil
  for _, output in noctalia.outputs() do
    local path = noctalia.wallpaperPath(output.name)
    if wanted == nil and type(path) == "string" and path ~= "" then
      wanted = path
    end
  end
  render()
  load()
end
