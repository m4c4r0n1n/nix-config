local FREQS = { "60", "170", "310", "600", "1k", "3k", "6k", "12k", "14k", "16k" }
local DEVICES = { { id = "arctis", label = "Arctis" }, { id = "arena9", label = "Arena 9" } }
local PRESETS = { "flat", "classical", "club", "dance", "fullbass", "fullbasstreble", "fulltreble", "headphones", "largehall", "live", "party", "pop", "reggae", "rock", "ska", "soft", "softrock", "techno" }
local PRESET_LABELS = { "Flat", "Classical", "Club", "Dance", "Full Bass", "Full Bass & Treble", "Full Treble", "Headphones", "Large Hall", "Live", "Party", "Pop", "Reggae", "Rock", "Ska", "Soft", "Soft Rock", "Techno" }

local device = 1
local gains = { 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 }
local preset = "flat"

local function presetIndex()
  for i, name in PRESETS do
    if name == preset then
      return i - 1
    end
  end
  return -1
end

local function loadState()
  local text = noctalia.readFile(noctalia.expandPath("~/.local/state/eq/" .. DEVICES[device].id))
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

local function fmt(g)
  if g > 0 then
    return string.format("+%.1f", g)
  end
  return string.format("%.1f", g)
end

local function applyCustom()
  local argv = { "eq", DEVICES[device].id, "set" }
  for _, g in gains do
    table.insert(argv, string.format("%.1f", g))
  end
  preset = "custom"
  noctalia.runAsync(argv)
end

local render

local function bandRow(i)
  return ui.row({ gap = 12, align = "center" }, {
    ui.row({ width = 44 }, { ui.label({ text = FREQS[i], fontSize = 13, color = "on_surface_variant" }) }),
    ui.slider({
      key = `band_{device}_{i}`,
      min = -18,
      max = 18,
      step = 0.5,
      value = gains[i],
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
    ui.row({ width = 52, justify = "end" }, { ui.label({ text = fmt(gains[i]), fontSize = 13, fontWeight = "bold" }) }),
  })
end

render = function()
  local deviceButtons = {}
  for i, d in DEVICES do
    table.insert(deviceButtons, ui.button({
      text = d.label,
      variant = if i == device then "primary" else "ghost",
      flexGrow = 1,
      onClick = function()
        device = i
        loadState()
        render()
      end,
    }))
  end

  local bands = {}
  for i = 1, 10 do
    table.insert(bands, bandRow(i))
  end

  panel.render(ui.column({ gap = 14, align = "stretch" }, {
    ui.row({ align = "center", justify = "space_between", gap = 8 }, {
      ui.row({ gap = 6, align = "center" }, {
        ui.button({
          glyph = "arrow-left",
          variant = "ghost",
          tooltip = "Back to Audio",
          onClick = function()
            noctalia.runAsync({ "noctalia", "msg", "panel-open", "control-center", "audio" })
          end,
        }),
        ui.label({ text = "Equalizer", fontSize = 18, fontWeight = "bold" }),
      }),
      ui.button({ glyph = "close", variant = "ghost", onClick = function() panel.close() end }),
    }),
    ui.row({ gap = 6, align = "center" }, deviceButtons),
    ui.row({ gap = 8, align = "center" }, {
      ui.select({
        key = `preset_{device}`,
        options = PRESET_LABELS,
        selectedIndex = presetIndex(),
        placeholder = "Custom",
        flexGrow = 1,
        onChange = function(index)
          local name = PRESETS[(tonumber(index) or 0) + 1]
          if name then
            noctalia.runAsync({ "eq", DEVICES[device].id, name }, function()
              loadState()
              render()
            end)
          end
        end,
      }),
      ui.button({
        text = "Reset",
        variant = "outline",
        onClick = function()
          noctalia.runAsync({ "eq", DEVICES[device].id, "flat" }, function()
            loadState()
            render()
          end)
        end,
      }),
    }),
    ui.column({ gap = 6, align = "stretch" }, bands),
  }))
end

function onOpen(_context)
  loadState()
  render()
end
