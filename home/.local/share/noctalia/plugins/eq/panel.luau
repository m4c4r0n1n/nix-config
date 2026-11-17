local FREQS = { 60, 170, 310, 600, 1000, 3000, 6000, 12000, 14000, 16000 }
local FREQ_LABELS = { "60", "170", "310", "600", "1k", "3k", "6k", "12k", "14k", "16k" }
local DEVICES = { { id = "arctis", label = "ARCTIS" }, { id = "arena9", label = "ARENA 9" } }
local LABELS = { flat = "Flat", anime = "Anime", movies = "Movies", classical = "Classical", club = "Club", dance = "Dance", fullbass = "Full Bass", fullbasstreble = "Full Bass & Treble", fulltreble = "Full Treble", headphones = "Headphones", largehall = "Large Hall", live = "Live", party = "Party", pop = "Pop", reggae = "Reggae", rock = "Rock", ska = "Ska", soft = "Soft", softrock = "Soft Rock", techno = "Techno", gaming = "Gaming", vocal = "Vocal", smooth = "Smooth (harsh audio)" }
local LOCAL_EQ = noctalia.expandPath("~/.local/bin/eq")
local EQ = if noctalia.fileExists(LOCAL_EQ) then LOCAL_EQ else "eq"
local RUN = noctalia.getenv("XDG_RUNTIME_DIR") or "/tmp"
local VIZ = RUN .. "/eq-viz"
local ALIVE = RUN .. "/eq-viz.alive"
local FONT = "Maple Mono NF"
local GLASS = "#0a0a0cb3"
local LINE = "#ffffff2e"
local DIVIDER = "#ffffff1a"
local TEXT = "#e6e6e6"
local DIM = "#9a9aa0"
local FAINT = "#5a5a60"
local BARS = 32
local CURVE_POINTS = 240
local SMOOTH_RADIUS = 9

local device = 2
local gains = { 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 }
local preset = "flat"
local builtins = {}
local saved = {}
local levels = {}
local nameText = ""
local nameKey = 0
local isOpen = false

local render

for i = 1, BARS do
  levels[i] = 0
end

local function devId()
  return DEVICES[device].id
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

local function fmt(g)
  if g > 0 then
    return string.format("+%.1f", g)
  end
  return string.format("%.1f", g)
end

local function loadState()
  local text = noctalia.readFile(noctalia.expandPath("~/.local/state/eq/" .. devId()))
  gains = { 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 }
  preset = "flat"
  if not text then
    return
  end
  local lines = string.split(text, "\n")
  preset = lines[1] or "flat"
  local i = 1
  for value in string.gmatch(lines[2] or "", "%S+") do
    if i <= 10 then
      gains[i] = tonumber(value) or 0
    end
    i += 1
  end
end

local function loadPresets(after)
  noctalia.runAsync({ EQ, "presets", devId() }, function(result)
    builtins, saved = {}, {}
    for line in string.gmatch(if result then result.stdout or "" else "", "[^\n]+") do
      local kind, name = string.match(line, "^(%w+)\t(.+)$")
      if kind == "user" then
        table.insert(saved, name)
      elseif kind == "builtin" then
        table.insert(builtins, name)
      else
        table.insert(builtins, line)
      end
    end
    if after then
      after()
    end
    render()
  end)
end

local function startViz()
  noctalia.writeFile(ALIVE, "")
  if EQ == LOCAL_EQ then
    noctalia.runAsync({ "setsid", "-f", EQ, "viz", devId() })
  end
end

local function run(argv, after)
  noctalia.runAsync(argv, function(result)
    if result == nil or result.exitCode ~= 0 then
      noctalia.notify("Equalizer", if result and result.stderr ~= "" then result.stderr else "eq failed")
    end
    loadState()
    if after then
      after()
    end
    render()
  end)
end

local function applyCustom()
  local argv = { EQ, devId(), "set" }
  for _, g in gains do
    table.insert(argv, string.format("%.1f", g))
  end
  preset = "custom"
  noctalia.runAsync(argv)
end

local function peakingDb(f, f0, g)
  if g == 0 then
    return 0
  end
  local fs = 48000
  local q = 1.41
  local a = 10 ^ (g / 40)
  local w0 = 2 * math.pi * f0 / fs
  local alpha = math.sin(w0) / (2 * q)
  local cw = math.cos(w0)
  local b0, b1, b2 = 1 + alpha * a, -2 * cw, 1 - alpha * a
  local a0, a1, a2 = 1 + alpha / a, -2 * cw, 1 - alpha / a
  local w = 2 * math.pi * f / fs
  local c1, s1, c2, s2 = math.cos(w), math.sin(w), math.cos(2 * w), math.sin(2 * w)
  local nr, ni = b0 + b1 * c1 + b2 * c2, -(b1 * s1 + b2 * s2)
  local dr, di = a0 + a1 * c1 + a2 * c2, -(a1 * s1 + a2 * s2)
  return 10 * math.log10((nr * nr + ni * ni) / (dr * dr + di * di))
end

local function curve()
  local values = {}
  for p = 0, CURVE_POINTS - 1 do
    local f = 25 * (18000 / 25) ^ (p / (CURVE_POINTS - 1))
    local db = 0
    for i, f0 in FREQS do
      db += peakingDb(f, f0, gains[i])
    end
    table.insert(values, db)
  end
  local smooth = {}
  for p = 1, #values do
    local sum, weight = 0, 0
    for k = -SMOOTH_RADIUS, SMOOTH_RADIUS do
      local v = values[p + k]
      if v ~= nil then
        local w = math.exp(-(k * k) / (2 * (SMOOTH_RADIUS / 2) ^ 2))
        sum += v * w
        weight += w
      end
    end
    table.insert(smooth, math.clamp(0.5 + (sum / weight) / 36, 0, 1))
  end
  return smooth
end

local function curveArea()
  local vals = curve()
  local count = 48
  local half = 32
  local cols = {}
  for c = 1, count do
    local v = vals[math.floor((c - 1) * (#vals - 1) / (count - 1)) + 1]
    local db = (v - 0.5) * 36
    local d = math.floor(math.min(1, math.abs(db) / 9) * (half - 2) + 0.5)
    local up = if db >= 0 then d else 0
    local down = if db < 0 then d else 0
    table.insert(cols, ui.column({ key = "cv" .. c, width = 11, gap = 0, height = half * 2 }, {
      ui.box({ width = 11, height = half - up - 1, fill = "#00000000" }),
      ui.box({ width = 11, height = up, radius = 2, fill = "primary/0.75" }),
      ui.box({ width = 11, height = 2, fill = "primary/0.35" }),
      ui.box({ width = 11, height = down, radius = 2, fill = "primary/0.45" }),
      ui.box({ width = 11, height = half - down - 1, fill = "#00000000" }),
    }))
  end
  return ui.row({ gap = 4, align = "start", justify = "center", height = half * 2 }, cols)
end

local function peak()
  local m = 0
  for _, g in gains do
    m = math.max(m, g)
  end
  return m
end

local function presetLabel(name)
  return LABELS[name] or name
end

local function visualizer()
  local bars = {}
  for i = 1, BARS do
    local v = levels[i] or 0
    local h = 3 + math.floor(v * 147)
    local strength = 0.25 + v * 0.75
    table.insert(bars, ui.box({ key = "bar" .. i, flexGrow = 1, height = h, radius = 2, fill = string.format("primary/%.2f", strength) }))
  end
  return ui.row({ gap = 4, align = "end", height = 150 }, bars)
end

local function bandRow(i)
  local g = gains[i]
  return ui.row({ gap = 10, align = "center", height = 30 }, {
    label(FREQ_LABELS[i], { width = 34, color = FAINT }),
    ui.slider({
      key = `band_{device}_{i}`,
      min = -18,
      max = 18,
      step = 0.5,
      value = g,
      flexGrow = 1,
      onChange = function(value)
        gains[i] = tonumber(value) or 0
        render()
      end,
      onDragEnd = function()
        applyCustom()
        render()
      end,
    }),
    label(fmt(g), { width = 48, color = if g == 0 then FAINT else TEXT }),
  })
end

local function presetRow(name, mine)
  local on = name == preset
  local children = {
    ui.box({ width = 2, height = 22, fill = if on then "primary" else "#00000000" }),
    label(presetLabel(name), { color = if on then TEXT else DIM, flexGrow = 1, maxLines = 1 }),
  }
  if mine then
    table.insert(children, ui.row({
      paddingH = 6,
      onClick = function()
        run({ EQ, devId(), "delete", name }, function()
          loadPresets()
        end)
      end,
      tooltip = "Delete " .. name,
    }, { label("×", { color = FAINT }) }))
  end
  return ui.row({
    key = (if mine then "u:" else "b:") .. name,
    gap = 10,
    align = "center",
    height = 26,
    onClick = function()
      run({ EQ, devId(), name })
    end,
  }, children)
end

local function presetColumn()
  local rows = { label("PRESETS", { color = FAINT, fontSize = 12 }) }
  for _, name in builtins do
    table.insert(rows, presetRow(name, false))
  end
  local list = { ui.column({ gap = 2 }, rows) }
  local mine = { label("SAVED", { color = FAINT, fontSize = 12 }) }
  if #saved == 0 then
    table.insert(mine, label("Nothing saved yet.", { color = FAINT, fontSize = 12 }))
  end
  for _, name in saved do
    table.insert(mine, presetRow(name, true))
  end
  table.insert(list, ui.box({ height = 10, width = 1 }))
  table.insert(list, ui.column({ gap = 2 }, mine))
  return ui.column({ width = 280, gap = 10, paddingH = 18, paddingV = 12 }, {
    ui.scroll({ flexGrow = 1, gap = 2 }, list),
    label("SAVE AS", { color = FAINT, fontSize = 12 }),
    ui.row({ gap = 8, align = "center" }, {
      ui.input({
        key = "name" .. nameKey,
        placeholder = "",
        value = "",
        flexGrow = 1,
        submitOnEnter = true,
        onChange = function(value)
          nameText = value or ""
        end,
        onSubmit = function()
          if nameText ~= "" then
            run({ EQ, devId(), "save", nameText }, function()
              nameText = ""
              nameKey += 1
              loadPresets()
            end)
          end
        end,
      }),
      ui.button({
        text = "Save",
        variant = "outline",
        onClick = function()
          if nameText ~= "" then
            run({ EQ, devId(), "save", nameText }, function()
              nameText = ""
              nameKey += 1
              loadPresets()
            end)
          end
        end,
      }),
    }),
  })
end

local function deviceLabels()
  local out = { label("┤ EQ ├", { color = TEXT }) }
  for i, d in DEVICES do
    table.insert(out, ui.row({
      key = "dev" .. d.id,
      paddingH = 4,
      onClick = function()
        if device ~= i then
          device = i
          for b = 1, BARS do
            levels[b] = 0
          end
          loadState()
          loadPresets()
          startViz()
        end
      end,
    }, { label(d.label, { color = if i == device then "primary" else FAINT }) }))
  end
  return out
end

render = function()
  if not isOpen then
    return
  end
  local bandsLeft, bandsRight = {}, {}
  for i = 1, 5 do
    table.insert(bandsLeft, bandRow(i))
    table.insert(bandsRight, bandRow(i + 5))
  end
  local current = if preset == "custom" then "Custom" else presetLabel(preset)
  local side = ui.box({ width = 1, height = 470, fill = LINE })
  panel.render(ui.column({ flexGrow = 1, gap = 0, fill = GLASS, paddingH = 10, paddingV = 6 }, {
    border(deviceLabels(), { label(`┤ {current} ├`, { color = "primary" }) }),
    ui.row({ gap = 0, align = "start", height = 470 }, {
      side,
      ui.column({ flexGrow = 1, gap = 10, paddingH = 18, paddingV = 12 }, {
        visualizer(),
        curveArea(),
        ui.row({ gap = 24, align = "start" }, {
          ui.column({ flexGrow = 1, gap = 2 }, bandsLeft),
          ui.column({ flexGrow = 1, gap = 2 }, bandsRight),
        }),
      }),
      ui.box({ width = 1, height = 446, fill = DIVIDER }),
      presetColumn(),
      side,
    }),
    border({ label("┤ drag a band · click a preset · name it and save ├", { color = FAINT }) }, { label(`┤ preamp {fmt(-peak())} dB ├`, { color = DIM }) }),
  }))
end

local sinceRead, sinceAlive = 0, 0

function onFrameTick(deltaMs)
  if not isOpen then
    panel.setNeedsFrameTick(false)
    return
  end
  sinceRead += deltaMs
  sinceAlive += deltaMs
  if sinceAlive >= 1000 then
    sinceAlive = 0
    noctalia.writeFile(ALIVE, "")
  end
  if sinceRead < 33 then
    return
  end
  sinceRead = 0
  local text = noctalia.readFile(VIZ)
  if text then
    local i = 1
    for value in string.gmatch(text, "%S+") do
      if i <= BARS then
        levels[i] = tonumber(value) or 0
      end
      i += 1
    end
    render()
  end
end

function onOpen(_context)
  isOpen = true
  panel.setNeedsFrameTick(true)
  loadState()
  loadPresets()
  startViz()
  render()
end

function onClose()
  isOpen = false
  panel.setNeedsFrameTick(false)
end
