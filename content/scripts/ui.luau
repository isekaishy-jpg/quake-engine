-- The status bar and menus (proposal 0118). The engine calls `draw_status`,
-- `draw_menu`, `open_menu` and `menu_key`, and asks for what's in `settings`.

settings = { "volume", "bgmvolume", "sensitivity", "gamma" }

local INK = { 0.87, 0.85, 0.80, 1 }
local MUTED = { 0.53, 0.52, 0.51, 1 }
local ACCENT = { 0.73, 0.29, 0.24, 1 }
local POINTER = "ui/art/pointers/kris-right-128.png"

-- A level has begun. This is where a game says what it's played from.
function level_started()
	ui.camera("first")
end

-- Levels a player can start, with what they're called.
local LEVELS = {
	{ map = "vesper_deep", title = "Vesper Deep" },
	{ map = "ember_keep", title = "The Ember Keep" },
	{ map = "night_yard", title = "The Night Yard" },
}

-- What a slider can set: its name, what it's called, its range and its step.
local SLIDERS = {
	{ name = "volume", title = "Sound", low = 0, high = 1, step = 0.1 },
	{ name = "bgmvolume", title = "Music", low = 0, high = 1, step = 0.1 },
	{ name = "sensitivity", title = "Look speed", low = 1, high = 11, step = 0.5 },
	{ name = "gamma", title = "Brightness", low = 0.5, high = 1.5, step = 0.05 },
}

local screen = "main"
local selected = 1
local changed_at = -10
local pressed_at = -10

-- What each screen has on it, given what's going on.
local function items(state)
	if screen == "levels" then
		local list = {}
		for _, level in LEVELS do
			table.insert(list, { title = level.title, run = "map " .. level.map })
		end
		table.insert(list, { title = "Back", go = "main" })
		return "Choose a level", list
	end
	if screen == "options" then
		local list = {}
		for _, slider in SLIDERS do
			table.insert(list, { title = slider.title, slider = slider })
		end
		table.insert(list, { title = "Back", go = "main" })
		return "Options", list
	end
	local list = {}
	if state.in_game then
		table.insert(list, { title = "Back to the game", close = true })
	end
	table.insert(list, { title = "Start a level", go = "levels" })
	table.insert(list, { title = "Options", go = "options" })
	table.insert(list, { title = "Quit", run = "quit" })
	return "Cultist Engine", list
end

function open_menu(state)
	screen = "main"
	selected = 1
	changed_at = state.time
end

-- A slider's value, moved by `steps`, and sent to the engine.
local function nudge(state, slider, steps)
	local value = state.settings[slider.name] or slider.low
	value = math.clamp(value + slider.step * steps, slider.low, slider.high)
	ui.run(slider.name .. " " .. tostring(value))
	ui.sound("misc/menu3.wav")
end

function menu_key(state, key)
	local _, list = items(state)
	local item = list[selected]
	if key == "UPARROW" or key == "UP" or key == "W" then
		selected = if selected <= 1 then #list else selected - 1
		changed_at = state.time
		ui.sound("misc/menu1.wav")
	elseif key == "DOWNARROW" or key == "DOWN" or key == "S" then
		selected = if selected >= #list then 1 else selected + 1
		changed_at = state.time
		ui.sound("misc/menu1.wav")
	elseif item.slider and (key == "LEFTARROW" or key == "LEFT" or key == "A") then
		nudge(state, item.slider, -1)
	elseif item.slider and (key == "RIGHTARROW" or key == "RIGHT" or key == "D") then
		nudge(state, item.slider, 1)
	elseif key == "ESCAPE" then
		if screen == "main" then
			ui.close()
		else
			screen = "main"
			selected = 1
		end
		ui.sound("misc/menu2.wav")
	elseif key == "ENTER" or key == "SPACE" then
		pressed_at = state.time
		ui.sound("misc/menu2.wav")
		if item.close then
			ui.close()
		elseif item.go then
			screen = item.go
			selected = 1
			changed_at = state.time
		elseif item.run then
			ui.run(item.run)
			ui.close()
		end
	end
end

function draw_menu(state)
	local title, list = items(state)
	local row = 30
	local width = 300
	local height = 74 + #list * row
	local left = (state.width - width) / 2
	local top = (state.height - height) / 2
	ui.fill({ 0, 0, state.width, state.height }, { 0, 0, 0, 0.6 })
	ui.frame("panel", { left, top, left + width, top + height })
	ui.text(title, {
		style = "display",
		x = state.width / 2,
		y = top + 16,
		size = 22,
		color = INK,
		align = 0.5,
	})
	for number, item in list do
		local y = top + 56 + (number - 1) * row
		local chosen = number == selected
		local inset = 20
		ui.frame(if chosen then "button-selected" else "button-normal", {
			left + inset,
			y,
			left + width - inset,
			y + row - 6,
		})
		if chosen then
			ui.moving("smear-swipe", {
				left + inset,
				y - 4,
				left + width - inset,
				y + row - 2,
			}, state.time - changed_at)
			ui.stretched(POINTER, { left - 32, y + 2, left + 22, y + 22 })
		end
		ui.text(item.title, {
			x = left + inset + 12,
			y = y + 3,
			size = 13,
			color = if chosen then INK else MUTED,
		})
		if item.slider then
			local slider = item.slider
			local value = state.settings[slider.name] or slider.low
			local share = (value - slider.low) / (slider.high - slider.low)
			local start = left + width - inset - 110
			local stop = left + width - inset - 12
			ui.fill({ start, y + 11, stop, y + 13 }, MUTED)
			ui.fill({ start, y + 11, start + (stop - start) * share, y + 13 }, ACCENT)
		end
	end
	local since = state.time - pressed_at
	if since < 0.6 then
		local y = top + 56 + (selected - 1) * row + 12
		local x = left + width / 2
		ui.moving("impact-pop", { x - 60, y - 60, x + 60, y + 60 }, since)
	end
end

function draw_status(state)
	local stats = state.stats
	if stats.health == nil then
		return
	end
	local y = state.height - 40
	local function reading(x, label, value, align)
		ui.text(label, { x = x, y = y, size = 9, color = MUTED, align = align })
		local color = if value <= 25 and label == "HEALTH" then ACCENT else INK
		ui.text(
			tostring(value),
			{ style = "bold", x = x, y = y + 10, size = 20, color = color, align = align }
		)
	end
	reading(24, "HEALTH", stats.health, 0)
	reading(100, "ARMOUR", stats.armor, 0)
	reading(state.width - 24, "AMMO", stats.ammo, 1)
end
