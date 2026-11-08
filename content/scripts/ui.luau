-- All that's over the game: the status bar, scores, messages, what's said in the middle of the
-- screen, the end of a level, and the menus (proposals 0118 and 0139). What this does and why
-- is in docs/research/ui-and-menus.md, and how it looks follows the kit's own previews.
--
-- The engine calls `level_started`, `draw_status`, `draw_menu`, `open_menu`, `menu_key` and
-- `menu_pointer`, and tells the script what's in `settings` and `key_commands`.

settings = {
	"volume",
	"bgmvolume",
	"ambient_level",
	"sensitivity",
	"gamma",
	"cl_alwaysrun",
	"m_pitch",
	"vid_vsync",
	"show_fps",
	"fov",
	"ui_contrast",
	"hud_plate",
	"crosshair",
	"r_edges",
	"r_fog_grid",
	"r_antialias",
	"r_msaa",
	"topcolor",
	"bottomcolor",
}

local BRIGHT = { 0.96, 0.94, 0.90, 1 }
local INK = { 0.87, 0.85, 0.80, 1 }
local MUTED = { 0.53, 0.52, 0.51, 1 }
local ACCENT = { 0.73, 0.29, 0.24, 1 }
local WHITE = { 1, 1, 1, 1 }
local SHADOW = { 0, 0, 0, 0.85 }
local POINTER = "ui/art/pointers/kris-right-256.png"
local SMEAR = "ui/art/textures/splatter-smear-512.png"
local BACKDROP = "ui/cultist-engine-conback-4k.png"
local CLICK = "misc/menu1.wav"
local CHANGE = "misc/menu3.wav"
local CHOOSE = "misc/menu2.wav"

-- How fast the end of the game types its words, in letters a second.
local TYPING_SPEED = 8

-- The smallest text gets, to fit where it's going.
local LEAST_TEXT = 8

-- How long a message stays, in seconds, and how long it takes to fade.
local MESSAGE_SECONDS = 4
local MESSAGE_FADE = 1

-- A level has begun. This is where a game says what it's played from.
function level_started()
	ui.camera("first")
end

-- Levels a player can start, with what they're called.
local LEVELS = {
	{ map = "start", title = "Quake", about = "The original game, from its first hall." },
	{
		map = "proving_ground",
		title = "The Proving Ground",
		about = "Where the engine's features are tried.",
	},
	{ map = "vesper_deep", title = "Vesper Deep", about = "A ruined quarter over a drowned city." },
	{ map = "ember_keep", title = "The Ember Keep", about = "A fortress of lifts and lava." },
	{ map = "night_yard", title = "The Night Yard", about = "A walled yard after dark." },
}

-- Levels to host, which others can join.
local HOST_LEVELS = {
	"dm1",
	"dm2",
	"dm3",
	"dm4",
	"dm5",
	"dm6",
	"start",
	"e1m1",
	"e2m1",
	"e3m1",
	"e4m1",
	"end",
}

-- What can be bound, and what it's called.
local ACTIONS = {
	{ "+attack", "Attack" },
	{ "impulse 10", "Next weapon" },
	{ "+jump", "Jump, or swim up" },
	{ "+forward", "Walk forward" },
	{ "+back", "Walk back" },
	{ "+moveleft", "Step left" },
	{ "+moveright", "Step right" },
	{ "+left", "Turn left" },
	{ "+right", "Turn right" },
	{ "+speed", "Run" },
	{ "+strafe", "Sidestep" },
	{ "+lookup", "Look up" },
	{ "+lookdown", "Look down" },
	{ "centerview", "Center view" },
	{ "+mlook", "Mouse look" },
	{ "+klook", "Keyboard look" },
	{ "+moveup", "Swim up" },
	{ "+movedown", "Swim down" },
}

key_commands = {}
for _, action in ACTIONS do
	table.insert(key_commands, action[1])
end

-- What's typed or chosen in the menus, kept while the game runs.
local values = {
	address = "",
	password = "",
	code = "",
	host_password = "",
	coop = 1,
	teamplay = 1,
	skill = 2,
	fraglimit = 1,
	timelimit = 1,
	level = 1,
}

local screen = "main"
local selected = 1
local changed_at = -10
local pressed_at = -10
local grabbing = false
local hot = {}
local hovered_tab = nil
-- How many rows each column has where a screen has two, and how many rows are in them.
local column_rows = 0
local rows_in_columns = 0
local note = ""
local asked = nil
-- The row each screen was left on, so that going back lands where you were.
local left_on = {}

-- A row that moves a setting between two numbers.
local function slider(title, name, low, high, step, about, percent)
	return {
		title = title,
		about = about,
		slider = { name = name, low = low, high = high, step = step, percent = percent },
	}
end

-- A row that turns a setting on and off.
local function switch(title, name, about)
	return { title = title, switch = name, about = about }
end

-- A row that sets a setting to one of `options`, each a name and the value it sets.
local function pick(title, name, options, about)
	return { title = title, pick = name, options = options, about = about }
end

-- Which of a picking row's options the setting is at, or the first.
local function picked(state, row)
	local value = state.settings[row.pick]
	for index, option in row.options do
		if option[2] == value then
			return index
		end
	end
	return 1
end

-- A row of choices kept here.
local function choice(title, name, options, about)
	return { title = title, choice = name, options = options, about = about }
end

-- A row to type into.
local function field(title, name, about)
	return { title = title, field = name, about = about }
end

-- A row that goes to another screen.
local function go(title, name, about)
	return { title = title, go = name, about = about }
end

-- A row that asks first, in a dialog.
local function ask(title, question, run, about)
	return { title = title, ask = { question = question, run = run }, about = about }
end

-- What keys are called in plain words.
local KEY_NAMES = {
	MOUSE1 = "Left Mouse",
	MOUSE2 = "Right Mouse",
	MOUSE3 = "Middle Mouse",
	MWHEELUP = "Wheel Up",
	MWHEELDOWN = "Wheel Down",
	UPARROW = "Up",
	DOWNARROW = "Down",
	LEFTARROW = "Left",
	RIGHTARROW = "Right",
	CTRL = "Ctrl",
	SHIFT = "Shift",
	ALT = "Alt",
	SPACE = "Space",
	ENTER = "Enter",
	TAB = "Tab",
	BACKSPACE = "Backspace",
	PGUP = "Page Up",
	PGDN = "Page Down",
	HOME = "Home",
	END = "End",
	INS = "Insert",
	DEL = "Delete",
	ESCAPE = "Escape",
	PAUSE = "Pause",
	SEMICOLON = "Semicolon",
	[","] = "Comma",
	["."] = "Period",
	["/"] = "Slash",
	["\\"] = "Backslash",
	["'"] = "Quote",
	["-"] = "Minus",
	["="] = "Equals",
	["["] = "Left Bracket",
	["]"] = "Right Bracket",
}

-- `key` as a person would say it.
local function key_name(key)
	return KEY_NAMES[key] or key
end

-- The first key that does `command`, in words.
local function key_of(state, command)
	local bound = state.keys[command] or {}
	return if #bound == 0 then "no key" else key_name(bound[#bound])
end

-- What each screen has on it, given what's going on.
local function screen_rows(state)
	if screen == "new" then
		local list = {
			choice(
				"Difficulty",
				"skill",
				{ "Easy", "Normal", "Hard", "Nightmare" },
				"How hard the game is. Press left or right to change it."
			),
		}
		for _, level in LEVELS do
			local command = string.format(
				"disconnect; deathmatch 0; coop 0; skill %d; map %s",
				values.skill - 1,
				level.map
			)
			table.insert(list, { title = level.title, run = command, about = level.about })
		end
		table.insert(list, go("Back", "main"))
		return "New Game", list
	end
	if screen == "load" or screen == "save" then
		local list = {}
		local saving = screen == "save"
		for slot, name in state.saves do
			-- A save's name is padded for a fixed font, which means nothing here.
			local text = string.match(name, "^%s*(.-)%s*$")
			local command = (if saving then "save s" else "load s") .. (slot - 1)
			if saving and text ~= "" then
				table.insert(list, ask(text, "Save over this game?", command))
			elseif saving then
				table.insert(list, { title = "Empty slot", run = command })
			elseif text ~= "" then
				table.insert(list, { title = text, run = command })
			end
		end
		table.insert(list, go("Back", "main"))
		return if saving then "Save Game" else "Load Game", list
	end
	if screen == "options" then
		return "Game",
			{
				slider(
					"Mouse Speed",
					"sensitivity",
					1,
					11,
					0.5,
					"How far the view turns for the mouse."
				),
				{
					title = "Invert Mouse",
					sign = "m_pitch",
					about = "Pushing the mouse forward looks down.",
				},
				switch("Always Run", "cl_alwaysrun", "Run without holding a key."),
				switch("Crosshair", "crosshair", "A small cross at the middle of the screen."),
				{
					title = "Console",
					run = "toggleconsole",
					about = "Where the engine says what it's doing, and takes commands.",
				},
				ask(
					"Reset Everything",
					"Put all settings and keys back as they were?",
					"reset_defaults",
					"Puts every setting and key back."
				),
				go("Back", "main"),
			}
	end
	if screen == "video" then
		return "Video",
			{
				slider("Brightness", "gamma", 0.5, 1.5, 0.05, "How bright the picture is."),
				slider("Field of View", "fov", 60, 130, 5, "How wide the view is, in degrees."),
				switch("Vertical Sync", "vid_vsync", "Stops tearing, and adds a little delay."),
				pick(
					"Anti-aliasing",
					"r_antialias",
					{ { "Off", 0 }, { "SMAA", 2 }, { "TAA", 3 } },
					"Smooths jagged edges. TAA also settles shimmer, over a few frames."
				),
				pick(
					"Multisampling",
					"r_msaa",
					{ { "Off", 1 }, { "2x", 2 }, { "4x", 4 }, { "8x", 8 } },
					"Smooths edges by drawing more of each pixel, and keeps what's thinner than one, far off. Each step costs more."
				),
				pick(
					"Render Scale",
					"r_scale",
					{
						{ "50%", 0.5 },
						{ "67%", 0.67 },
						{ "75%", 0.75 },
						{ "100%", 1 },
						{ "150%", 1.5 },
						{ "200%", 2 },
					},
					"How big the picture's drawn. Under 100% is faster and softer, brought up to the screen, best with TAA. Over is sharper and costs more."
				),
				switch("Rounded Corners", "r_edges", "Softens the hard corners of walls a little."),
				switch(
					"Lit Fog",
					"r_fog_grid",
					"Fog that's bright by lights and dark away from them."
				),
				switch("Frame Rate", "show_fps", "Shows frames a second in the corner."),
				go("Back", "main"),
			}
	end
	if screen == "audio" then
		return "Audio",
			{
				slider("Sound", "volume", 0, 1, 0.05, "How loud everything is.", true),
				slider("Music", "bgmvolume", 0, 1, 0.05, "How loud the music is.", true),
				slider(
					"Ambience",
					"ambient_level",
					0,
					1,
					0.05,
					"How loud the world's background sound is.",
					true
				),
				go("Back", "main"),
			}
	end
	if screen == "access" then
		return "Accessibility",
			{
				switch(
					"High Contrast",
					"ui_contrast",
					"Solid backgrounds and the strongest colours in the menus."
				),
				switch(
					"Status Plate",
					"hud_plate",
					"A dark plate behind what's on screen in the game."
				),
				go("Back", "main"),
			}
	end
	if screen == "keys" then
		local list = {}
		for _, action in ACTIONS do
			table.insert(list, { title = action[2], bind = action[1] })
		end
		table.insert(list, go("Back", "main"))
		return "Controls", list
	end
	if screen == "multiplayer" then
		return "Multiplayer",
			{
				go("Join a Game", "join", "Play on someone else's game."),
				go("Host a Game", "host", "Start a game for others to join."),
				go("Your Player", "setup", "Your name and colours."),
				go("Back", "main"),
			}
	end
	if screen == "join" then
		return "Join a Game",
			{
				field("Address", "address", "Where the game is, such as example.com:26000."),
				field("Password", "password", "If the game has one."),
				field("Join Code", "code", "If the host gave you one."),
				{ title = "Connect", join = true, about = "Joins the game. It needs an address." },
				go("Back", "multiplayer"),
			}
	end
	if screen == "host" then
		local minutes = {}
		for number = 0, 12 do
			table.insert(minutes, if number == 0 then "None" else tostring(number * 5))
		end
		local frags = {}
		for number = 0, 10 do
			table.insert(frags, if number == 0 then "None" else tostring(number * 10))
		end
		return "Host a Game",
			{
				choice("Game", "coop", { "Deathmatch", "Cooperative" }),
				choice("Teamplay", "teamplay", { "Off", "No Friendly Fire", "Friendly Fire" }),
				choice("Difficulty", "skill", { "Easy", "Normal", "Hard", "Nightmare" }),
				choice("Frag Limit", "fraglimit", frags),
				choice("Time Limit", "timelimit", minutes, "In minutes."),
				choice("Level", "level", HOST_LEVELS),
				field("Password", "host_password", "Leave it empty for anyone to join."),
				{ title = "Begin", host = true },
				go("Back", "multiplayer"),
			}
	end
	if screen == "setup" then
		return "Your Player",
			{
				field("Name", "name"),
				slider("Shirt", "topcolor", 0, 13, 1),
				slider("Pants", "bottomcolor", 0, 13, 1),
				{ title = "Done", setup = true },
			}
	end
	if screen == "help" then
		return "Help", { go("Back", "main") }
	end
	if screen == "ask" then
		return asked.question, { { title = "Yes", run = asked.run }, go("No", asked.back) }
	end
	local list = {}
	if state.in_game then
		table.insert(list, { title = "Resume", close = true })
		if state.stats.can_save == 1 then
			table.insert(list, go("Save Game", "save"))
		end
		table.insert(list, go("Load Game", "load"))
		table.insert(list, go("Options", "options"))
		table.insert(list, go("Help", "help"))
		table.insert(
			list,
			ask("Leave Game", "Leave this game? Anything not saved is lost.", "disconnect")
		)
	else
		if (state.texts.latest_save or "") ~= "" then
			table.insert(list, { title = "Continue", run = "load " .. state.texts.latest_save })
		end
		table.insert(list, go("New Game", "new"))
		table.insert(list, go("Load Game", "load"))
		table.insert(list, go("Multiplayer", "multiplayer"))
		table.insert(list, go("Options", "options"))
		table.insert(list, go("Help", "help"))
	end
	table.insert(list, ask("Quit", "Quit the game?", "quit"))
	return "Cultist Engine", list
end

-- Goes to the screen called `name`, on the row it was left on.
local function open_screen(state, name)
	left_on[screen] = selected
	screen = name
	selected = left_on[name] or 1
	grabbing = false
	note = ""
	changed_at = state.time
	if name == "setup" then
		values.name = state.texts.name or ""
	end
end

function open_menu(state, name)
	left_on = {}
	if name == "quit" then
		asked = { question = "Quit the game?", run = "quit", back = "main" }
		open_screen(state, "ask")
		selected = 2
		return
	end
	open_screen(state, if name == "" then "main" else name)
	selected = 1
end

-- Sets a setting.
local function set(name, value)
	ui.run(string.format("%s %g", name, math.round(value * 1000) / 1000))
	ui.sound(CHANGE)
end

-- A slider's value, moved by `steps`.
local function nudge(state, row, steps)
	local track = row.slider
	local value = state.settings[track.name] or track.low
	set(track.name, math.clamp(value + track.step * steps, track.low, track.high))
end

-- Turns a row one way, if it turns at all.
local function turn(state, row, steps)
	if row.slider then
		nudge(state, row, steps)
	elseif row.switch then
		set(row.switch, if (state.settings[row.switch] or 0) ~= 0 then 0 else 1)
	elseif row.sign then
		set(row.sign, -(state.settings[row.sign] or 0))
	elseif row.choice then
		values[row.choice] = (values[row.choice] - 1 + steps) % #row.options + 1
		ui.sound(CHANGE)
	elseif row.pick then
		local next = (picked(state, row) - 1 + steps) % #row.options + 1
		set(row.pick, row.options[next][2])
	end
end

-- Joins the game typed in.
local function join()
	local code = if values.code == "" then "" else string.format(' "%s"', values.code)
	ui.run(string.format('password "%s"; connect "%s"%s', values.password, values.address, code))
	ui.close()
end

-- Starts a game by the rules chosen, for others to join.
local function host()
	local coop = values.coop - 1
	ui.run(
		string.format(
			'disconnect; sv_password "%s"; coop %d; deathmatch %d; teamplay %d; skill %d; fraglimit %d; timelimit %d; map %s; host',
			values.host_password,
			coop,
			1 - coop,
			values.teamplay - 1,
			values.skill - 1,
			(values.fraglimit - 1) * 10,
			(values.timelimit - 1) * 5,
			HOST_LEVELS[values.level]
		)
	)
	ui.close()
end

-- Does what a row does, on being chosen.
local function press(state, row)
	pressed_at = state.time
	ui.sound(CHOOSE)
	if row.close then
		ui.close()
	elseif row.go then
		open_screen(state, row.go)
	elseif row.ask then
		asked = { question = row.ask.question, run = row.ask.run, back = screen }
		open_screen(state, "ask")
		selected = 2
	elseif row.run then
		ui.run(row.run)
		ui.close()
	elseif row.bind then
		grabbing = true
		note = ""
	elseif row.join then
		if values.address ~= "" then
			join()
		else
			note = "Type an address first."
		end
	elseif row.host then
		host()
	elseif row.setup then
		ui.run(string.format('name "%s"', values.name or ""))
		open_screen(state, "multiplayer")
	else
		turn(state, row, 1)
	end
end

-- Moves what's chosen by `steps`, going round.
local function move(list, steps, state)
	selected = (selected - 1 + steps) % #list + 1
	changed_at = state.time
	ui.sound(CLICK)
end

-- Where a screen goes back to.
local BACK = {
	new = "main",
	load = "main",
	save = "main",
	options = "main",
	video = "main",
	audio = "main",
	access = "main",
	keys = "main",
	multiplayer = "main",
	join = "multiplayer",
	host = "multiplayer",
	setup = "multiplayer",
	help = "main",
}

-- The tabs of a screen of settings, and which screens have them.
local OPTION_TABS = {
	{ "Game", "options" },
	{ "Video", "video" },
	{ "Audio", "audio" },
	{ "Controls", "keys" },
	{ "Accessibility", "access" },
}
local PLAY_TABS = { { "Join", "join" }, { "Host", "host" }, { "Player", "setup" } }
local TABS = {
	options = OPTION_TABS,
	video = OPTION_TABS,
	audio = OPTION_TABS,
	keys = OPTION_TABS,
	access = OPTION_TABS,
	join = PLAY_TABS,
	host = PLAY_TABS,
	setup = PLAY_TABS,
}

-- Goes to the tab `steps` along, going round, so that a keyboard reaches them all.
local function step_tab(state, steps)
	local tabs = TABS[screen]
	for number, tab in tabs do
		if tab[2] == screen then
			ui.sound(CHOOSE)
			open_screen(state, tabs[(number - 1 + steps) % #tabs + 1][2])
			return
		end
	end
end

-- Moves what's chosen to the next column, on a screen with two.
local function cross(steps, state)
	if selected > rows_in_columns then
		return
	end
	local goal = math.clamp(selected + steps * column_rows, 1, rows_in_columns)
	local same_column = (goal - 1) // column_rows == (selected - 1) // column_rows
	if not same_column then
		selected = goal
		changed_at = state.time
		ui.sound(CLICK)
	end
end

-- Goes back a screen, or leaves the menus.
local function go_back(state)
	ui.sound(CHOOSE)
	if screen == "ask" then
		open_screen(state, asked.back)
	elseif BACK[screen] then
		open_screen(state, BACK[screen])
	else
		ui.close()
	end
end

-- What's bound to `key` now, of what the menus know about.
local function action_on(state, key)
	for _, action in ACTIONS do
		for _, bound in state.keys[action[1]] or {} do
			if bound == key then
				return action
			end
		end
	end
	return nil
end

-- Takes `key` for the action being bound. A key does one thing, so what it did is said aloud.
local function finish_binding(state, row, key)
	grabbing = false
	ui.sound(CLICK)
	if key == "ESCAPE" or key == "`" then
		note = ""
	elseif key == "BACKSPACE" then
		for _, bound in state.keys[row.bind] or {} do
			ui.run(string.format('unbind "%s"', bound))
		end
		note = row.title .. " is on no key now."
	else
		local before = action_on(state, key)
		for _, bound in state.keys[row.bind] or {} do
			ui.run(string.format('unbind "%s"', bound))
		end
		ui.run(string.format('bind "%s" "%s"', key, row.bind))
		note = if before and before[1] ~= row.bind
			then string.format("%s was on %s, which has no key now.", key_name(key), before[2])
			else string.format("%s is on %s.", row.title, key_name(key))
	end
end

function menu_key(state, key)
	local _, list = screen_rows(state)
	selected = math.clamp(selected, 1, #list)
	local row = list[selected]
	if grabbing then
		finish_binding(state, row, key)
		return
	end
	if key == "UPARROW" or key == "MWHEELUP" then
		move(list, -1, state)
	elseif key == "DOWNARROW" or key == "TAB" or key == "MWHEELDOWN" then
		move(list, 1, state)
	elseif key == "HOME" then
		move(list, 1 - selected, state)
	elseif key == "END" then
		move(list, #list - selected, state)
	elseif (key == "PGUP" or key == "PGDN") and TABS[screen] then
		step_tab(state, if key == "PGUP" then -1 else 1)
	elseif key == "LEFTARROW" or key == "RIGHTARROW" then
		local steps = if key == "LEFTARROW" then -1 else 1
		if column_rows > 0 then
			cross(steps, state)
		else
			turn(state, row, steps)
		end
	elseif key == "ESCAPE" or key == "MOUSE2" then
		go_back(state)
	elseif key == "ENTER" or (key == "SPACE" and not row.field) then
		press(state, row)
	elseif row.bind and (key == "BACKSPACE" or key == "DEL") then
		finish_binding(state, row, "BACKSPACE")
	elseif row.field then
		local text = values[row.field] or ""
		if key == "BACKSPACE" then
			values[row.field] = string.sub(text, 1, -2)
		elseif state.typed ~= "" and #text < 32 then
			values[row.field] = text .. state.typed
		end
	elseif screen == "ask" and key == "Y" then
		press(state, list[1])
	elseif screen == "ask" and key == "N" then
		go_back(state)
	end
end

-- Sets a slider from where the pointer is along it.
local function slide_to(row, place, x)
	local track = row.slider
	local share =
		math.clamp((x - place.slider_left) / (place.slider_right - place.slider_left), 0, 1)
	local value = track.low + (track.high - track.low) * share
	set(track.name, math.round(value / track.step) * track.step)
end

-- The pointer moved, or clicked. Hovering chooses a row, a click does what it does, and a
-- slider follows a held button.
function menu_pointer(state, x, y, clicked)
	local _, list = screen_rows(state)
	if grabbing then
		if clicked then
			finish_binding(state, list[selected], "MOUSE1")
		end
		return
	end
	hovered_tab = nil
	for _, place in hot do
		if x >= place.left and x <= place.right and y >= place.top and y <= place.bottom then
			if place.tab then
				hovered_tab = place.tab
				if clicked and place.tab ~= screen then
					ui.sound(CHOOSE)
					open_screen(state, place.tab)
				end
				return
			end
			if place.number ~= selected then
				selected = place.number
				changed_at = state.time
				ui.sound(CLICK)
			end
			local row = list[selected]
			if row and row.slider and place.slider_left and x >= place.slider_left - 8 then
				if clicked or state.numbers.held == 1 then
					slide_to(row, place, x)
				end
			elseif clicked and row then
				press(state, row)
			end
			return
		end
	end
end

-- What a slider says beside it.
local function amount_of(track, value)
	if track.percent then
		return string.format("%d%%", math.round(value * 100))
	end
	if track.step >= 1 then
		return string.format("%d", math.round(value))
	end
	return string.format("%.2f", value)
end

-- How wide `text` is at `size`, by the font it's drawn in.
local function width_of(text, size, style, tracking)
	return ui.measure(text, { size = size, style = style, tracking = tracking })
end

-- The size that `text` can be to fit in `width`, which is `size` if there's room. Nothing's cut.
local function size_to_fit(text, width, size, style, tracking)
	local wide = width_of(text, size, style, tracking)
	if wide <= width then
		return size
	end
	return math.max(size * width / wide * 0.97, LEAST_TEXT)
end

-- `text` in lines that fit in `width` at `size`.
local function wrapped(text, width, size, style)
	local lines = {}
	local line = ""
	for _, word in string.split(text, " ") do
		local joined = if line == "" then word else line .. " " .. word
		if line ~= "" and width_of(joined, size, style) > width then
			table.insert(lines, line)
			line = word
		else
			line = joined
		end
	end
	table.insert(lines, line)
	return lines
end

-- How `count` rows go in a height of `room`: the rows in a column, and how tall each is. One
-- column if they fit at `least` each, or two, and never a list to scroll.
local function lay_out_rows(count, room, tallest, least)
	local per_column = if count * least > room then math.ceil(count / 2) else count
	return per_column, math.min(tallest, room / math.max(per_column, 1))
end

-- What colours are in use, for high contrast or not.
local function colours(state)
	if (state.settings.ui_contrast or 0) ~= 0 then
		return { off = BRIGHT, on = { 1, 1, 1, 1 } }
	end
	return { off = MUTED, on = BRIGHT }
end

-- A part of the kit that's a fixed picture, by its top left corner.
local function piece(name, x, y, width, height)
	ui.piece(name, { x, y, x + width, y + height }, WHITE)
end

-- The backdrop of the menus: the whole picture, or a solid one for high contrast.
local function draw_backdrop(state)
	local whole = { 0, 0, state.width, state.height }
	if (state.settings.ui_contrast or 0) ~= 0 then
		ui.fill(whole, { 0, 0, 0, 1 })
		return
	end
	-- It covers the screen in its own shape, held to the bottom right where its art is.
	local shape = 16 / 9
	local width = math.max(state.width, state.height * shape)
	local height = width / shape
	ui.stretched(
		BACKDROP,
		{ state.width - width, state.height - height, state.width, state.height }
	)
end

-- The line of what can be pressed, along the foot of the screen.
local function draw_hints(state, row)
	local text = "Enter chooses"
	if row and (row.slider or row.choice or row.pick or row.switch or row.sign) then
		text = "Left and Right change this"
	elseif row and row.bind then
		text = "Enter changes this key     Backspace clears it"
	elseif row and row.field then
		text = "Type here     Tab goes to the next"
	end
	if TABS[screen] then
		text ..= "     PgUp and PgDn change page"
	end
	text ..= "     Esc goes back"
	local size = size_to_fit(text, state.width - 32, 9)
	ui.text(text, { x = 16, y = state.height - 18, size = size, color = colours(state).off })
end

-- The kris and the smear beside a chosen row of plain text. Beside another column the kris
-- is `blade` long, so that it leaves that column alone.
local function draw_chosen(state, x, y, width, blade)
	local age = state.time - changed_at
	local place = { x - 16, y - 12, math.min(x + width + 80, state.width), y + 30 }
	if age < 0.4 then
		ui.moving("smear-swipe", place, age)
	else
		ui.stretched(SMEAR, place, 0.5)
	end
	local tall = blade * 28 / 74
	ui.stretched(POINTER, { x - 14 - blade, y + 7 - tall / 2, x - 14, y + 7 + tall / 2 })
	local since = state.time - pressed_at
	if since < 0.6 then
		ui.moving("impact-pop", { x - 40, y - 54, x + 76, y + 62 }, since)
	end
end

-- A screen of plain rows, as the title screen is. A long list takes two columns.
local function draw_list(state, title, list)
	local shades = colours(state)
	local title_size = size_to_fit(string.upper(title), state.width - 32, 24, "display", 5)
	ui.text(string.upper(title), {
		style = "display",
		x = state.width / 2,
		y = state.height * 0.09,
		size = title_size,
		color = INK,
		align = 0.5,
		tracking = 5,
	})
	local top = state.height * 0.25
	local per_column, row_height = lay_out_rows(#list, state.height - top - 50, 30, 26)
	local two = per_column < #list
	column_rows = if two then per_column else 0
	rows_in_columns = #list
	local width = if two then state.width * 0.43 - 56 else state.width * 0.58 - 24
	local blade = if two then 40 else 74
	if two then
		-- Over the backdrop's art, a long list needs something behind it to be read.
		local bottom = top + per_column * row_height + 6
		ui.frame("panel", { state.width * 0.12 - 66, top - 16, state.width - 16, bottom })
	end
	for number, row in list do
		local x = state.width * (if two then (if number > per_column then 0.55 else 0.12) else 0.42)
		local y = top + (number - 1) % per_column * row_height
		local chosen = number == selected
		table.insert(hot, {
			number = number,
			left = x - 14 - blade,
			right = x + width,
			top = y - 4,
			bottom = y + row_height - 6,
		})
		if chosen then
			draw_chosen(state, x, y, if two then width - 80 else 140, blade)
		end
		local color = if chosen then shades.on else shades.off
		if row.choice or row.pick then
			local option = if row.choice
				then row.options[values[row.choice]]
				else row.options[picked(state, row)][1]
			local text = row.title .. ":  " .. option
			ui.text(text, { x = x, y = y, size = size_to_fit(text, width, 15), color = color })
		else
			local text = if #row.title < 22 then string.upper(row.title) else row.title
			ui.text(text, {
				x = x,
				y = y,
				size = size_to_fit(text, width, 15, nil, 0.6),
				color = color,
				tracking = 0.6,
			})
		end
	end
	local row = list[selected]
	if row and row.about then
		local x = state.width * 0.42
		local lines = wrapped(row.about, state.width - x - 12, 11)
		for number, line in lines do
			ui.text(line, {
				x = x,
				y = state.height - 22 - (#lines - number + 1) * 14,
				size = 11,
				color = shades.off,
			})
		end
	end
	draw_hints(state, row)
end

-- The control at the right of a row of settings.
local function draw_control(state, row, chosen, left, right, y)
	local middle = y + 12
	local shades = colours(state)
	if row.slider then
		local track = row.slider
		local value = state.settings[track.name] or track.low
		local share = math.clamp((value - track.low) / (track.high - track.low), 0, 1)
		local stop = right - 42
		local reach = left + (stop - left) * share
		ui.frame("slider-track", { left, middle - 4, stop, middle + 4 })
		if share > 0.04 then
			ui.frame("slider-fill", { left, middle - 4, reach, middle + 4 })
		end
		piece(
			if chosen then "slider-thumb-hover" else "slider-thumb-normal",
			reach - 7,
			middle - 9,
			15,
			18
		)
		ui.text(
			amount_of(track, value),
			{ x = right, y = y + 5, size = 12, color = shades.on, align = 1 }
		)
		return stop
	elseif row.switch or row.sign then
		local on = if row.switch
			then (state.settings[row.switch] or 0) ~= 0
			else (state.settings[row.sign] or 0) < 0
		piece(if on then "toggle-on" else "toggle-off", right - 31, middle - 10, 31, 20)
		ui.text(
			if on then "On" else "Off",
			{ x = right - 40, y = y + 5, size = 12, color = shades.off, align = 1 }
		)
	elseif row.field then
		local text = values[row.field] or ""
		ui.frame(if chosen then "field-focus" else "field-normal", { left, y, right, y + 24 })
		local caret = if chosen and math.floor(state.time * 2) % 2 == 0 then "_" else ""
		ui.text(text .. caret, {
			x = left + 9,
			y = y + 5,
			size = size_to_fit(text .. "_", right - left - 18, 12),
			color = INK,
		})
	elseif row.choice or row.pick then
		ui.frame(if chosen then "field-focus" else "field-normal", { left, y, right, y + 24 })
		local text = if row.choice
			then row.options[values[row.choice]]
			else row.options[picked(state, row)][1]
		ui.text(text, {
			x = left + 9,
			y = y + 5,
			size = size_to_fit(text, right - left - 32, 12),
			color = INK,
		})
		piece("icon-chevron-right", right - 20, y + 4, 16, 16)
	elseif row.bind then
		local bound = state.keys[row.bind] or {}
		ui.frame(if chosen then "field-focus" else "field-normal", { left, y, right, y + 24 })
		local names = {}
		for number, key in bound do
			if number <= 2 then
				table.insert(names, key_name(key))
			end
		end
		local text = if #bound == 0 then "No key" else table.concat(names, ", ")
		if #bound > 2 then
			text ..= string.format(" +%d", #bound - 2)
		end
		local size = size_to_fit(text, right - left - 18, 12)
		ui.text(
			text,
			{ x = left + 9, y = y + 5, size = size, color = if #bound == 0 then ACCENT else INK }
		)
	else
		piece("icon-chevron-right", right - 20, y + 4, 16, 16)
	end
	return right
end

-- The row of settings at `number`, at the height `at`.
local function draw_setting(state, row, number, column, control_left, right_edge, at)
	local shades = colours(state)
	local chosen = number == selected
	if chosen then
		ui.stretched(SMEAR, { column - 8, at - 6, control_left + 30, at + 30 }, 0.3)
	end
	local size = size_to_fit(row.title, control_left - column - 12, 12)
	ui.text(row.title, {
		x = column + 6,
		y = at + 5,
		size = size,
		color = if chosen then shades.on else shades.off,
	})
	local stop = draw_control(state, row, chosen, control_left, right_edge, at)
	table.insert(hot, {
		number = number,
		left = column,
		right = right_edge,
		top = at - 2,
		bottom = at + 26,
		slider_left = control_left,
		slider_right = stop,
	})
end

-- The way out of a screen of settings.
local function draw_back_button(state, row, number, place)
	local shades = colours(state)
	local chosen = selected == number
	ui.frame(if chosen then "button-selected" else "button-normal", place)
	ui.text(row.title, {
		x = (place[1] + place[3]) / 2,
		y = place[2] + 7,
		size = 12,
		color = if chosen then shades.on else shades.off,
		align = 0.5,
	})
	table.insert(
		hot,
		{ number = number, left = place[1], right = place[3], top = place[2], bottom = place[4] }
	)
end

-- A screen of settings, with tabs down the side as the kit lays them out. A long list takes
-- two columns.
local function draw_settings(state, title, list)
	local shades = colours(state)
	local width = math.min(state.width - 16, 624)
	local height = math.min(state.height - 24, 340)
	local left = (state.width - width) / 2
	local top = (state.height - 16 - height) / 2
	local column = left + 136
	ui.frame("panel", { left, top, left + width, top + height })
	ui.text("CULTIST", { x = left + 18, y = top + 16, size = 15, color = INK, tracking = 2 })
	ui.text(
		"SETTINGS",
		{ x = left + 18, y = top + 36, size = 8, color = shades.off, tracking = 1.2 }
	)
	ui.frame("separator-vertical", { column - 12, top + 14, column - 10, top + height - 14 })
	local y = top + 56
	for _, tab in TABS[screen] do
		local active = tab[2] == screen
		local place = { left + 14, y, column - 22, y + 28 }
		ui.frame(
			if active
				then "tab-active"
				elseif hovered_tab == tab[2] then "tab-hover"
				else "tab-inactive",
			place
		)
		local size = size_to_fit(tab[1], place[3] - place[1] - 20, 12)
		ui.text(tab[1], {
			x = place[1] + 10,
			y = y + 7,
			size = size,
			color = if active then shades.on else shades.off,
		})
		table.insert(
			hot,
			{ tab = tab[2], left = place[1], right = place[3], top = place[2], bottom = place[4] }
		)
		y += 34
	end
	ui.text(title, { x = column + 6, y = top + 14, size = 16, color = INK })
	local rows_top = top + 42
	-- The last row is Back or Done, which goes at the foot of the tabs.
	local rows = #list - 1
	local per_column, row_height = lay_out_rows(rows, height - 88, 30, 26)
	local two = per_column < rows
	column_rows = if two then per_column else 0
	rows_in_columns = rows
	local right_edge = left + width - 16
	local gap = 16
	local span = if two then (right_edge - column - gap) / 2 else right_edge - column
	for number = 1, rows do
		local second = number > per_column
		local x = if second then column + span + gap else column
		local edge = x + span
		local control = if two then span * 0.52 else 210
		local at = rows_top + (number - 1) % per_column * row_height
		draw_setting(state, list[number], number, x, edge - control, edge, at)
	end
	local row = list[selected]
	local words = if note ~= "" then note elseif row and row.about then row.about else ""
	local y_words = top + height - 42
	for _, line in wrapped(words, right_edge - column - 6, 10) do
		ui.text(line, { x = column + 6, y = y_words, size = 10, color = shades.off })
		y_words += 13
	end
	local foot = top + height - 42
	draw_back_button(state, list[#list], #list, { left + 14, foot, column - 22, foot + 28 })
	draw_hints(state, row)
end

-- A question or a few words, in the kit's dialog.
local function draw_dialog(state, title, list, words)
	local shades = colours(state)
	local width = math.min(state.width - 24, 400)
	local heading = wrapped(title, width - 48, 14)
	local lines = {}
	for _, line in words do
		for _, part in wrapped(line, width - 48, 11) do
			table.insert(lines, part)
		end
	end
	local body = 24 + #heading * 18
	local height = body
		+ #lines * 17
		+ (if #lines > 0 then 8 else 0)
		+ (if #list > 0 then 44 else 0)
	local left = (state.width - width) / 2
	local top = (state.height - height) / 2
	ui.frame("dialog", { left, top, left + width, top + height })
	for number, line in heading do
		ui.text(line, { x = left + 24, y = top + 2 + number * 18, size = 14, color = INK })
	end
	for number, line in lines do
		ui.text(
			line,
			{ x = left + 24, y = top + body + (number - 1) * 17, size = 11, color = shades.off }
		)
	end
	local gap = 12
	local button = (width - 48 - gap * math.max(#list - 1, 0)) / math.max(#list, 1)
	local y = top + height - 40
	for number, row in list do
		local x = left + 24 + (number - 1) * (button + gap)
		local chosen = number == selected
		ui.frame(
			if chosen then "button-selected" else "button-normal",
			{ x, y, x + button, y + 28 }
		)
		ui.text(row.title, {
			x = x + button / 2,
			y = y + 7,
			size = 12,
			color = if chosen then shades.on else shades.off,
			align = 0.5,
		})
		table.insert(
			hot,
			{ number = number, left = x, right = x + button, top = y, bottom = y + 28 }
		)
	end
end

-- What a new player needs, in the keys they really have.
local function help_lines(state)
	return {
		string.format(
			"Move with %s, %s, %s and %s.",
			key_of(state, "+forward"),
			key_of(state, "+back"),
			key_of(state, "+moveleft"),
			key_of(state, "+moveright")
		),
		string.format(
			"Attack with %s, jump with %s.",
			key_of(state, "+attack"),
			key_of(state, "+jump")
		),
		"Look with the mouse.",
		"Change weapon with the number keys.",
		"Doors open as you come near. Some want a button or a key.",
		"Escape opens this menu. Change anything under Options.",
	}
end

function draw_menu(state)
	local title, list = screen_rows(state)
	selected = math.clamp(selected, 1, #list)
	hot = {}
	column_rows = 0
	draw_backdrop(state)
	if TABS[screen] then
		draw_settings(state, title, list)
	elseif screen == "ask" then
		draw_dialog(state, title, list, {})
	elseif screen == "help" then
		draw_dialog(state, title, list, help_lines(state))
	else
		draw_list(state, title, list)
	end
	if grabbing then
		ui.fill({ 0, 0, state.width, state.height }, { 0, 0, 0, 0.5 })
		local row = list[selected]
		draw_dialog(
			state,
			"Press a key for " .. row.title,
			{},
			{ "Escape cancels. Backspace clears it." }
		)
	end
end

-- Minutes and seconds.
local function clock(seconds)
	local whole = math.floor(seconds)
	return string.format("%d:%02d", whole // 60, whole % 60)
end

-- Text over the game, which has a shadow to stand out from whatever's behind it.
local function say(text, options)
	local shadow = table.clone(options)
	shadow.x += 1
	shadow.y += 1
	shadow.color = { 0, 0, 0, (options.color[4] or 1) * SHADOW[4] }
	ui.text(text, shadow)
	ui.text(text, options)
end

-- Everyone's scores, in a dialog.
local function draw_scores(state, title)
	local list = state.scores
	local row = math.min(20, (state.height - 72) / math.max(#list, 1))
	local width = 300
	local height = 56 + #list * row
	local left = (state.width - width) / 2
	local top = (state.height - height) / 2
	ui.frame("dialog", { left, top, left + width, top + height })
	ui.text(title, {
		style = "display",
		x = state.width / 2,
		y = top + 12,
		size = 18,
		color = INK,
		align = 0.5,
	})
	for number, score in list do
		local y = top + 40 + (number - 1) * row
		local color = if score.own then BRIGHT else MUTED
		local size = math.min(13, row - 3)
		ui.text(
			tostring(score.frags),
			{ x = left + 70, y = y, size = size, color = color, align = 1 }
		)
		ui.text(score.name, {
			x = left + 84,
			y = y,
			size = size_to_fit(score.name, width - 104, size),
			color = color,
		})
	end
end

-- The numbers of a level that's over.
local function draw_results(state)
	local stats = state.stats
	local width = 320
	local height = 170
	local left = (state.width - width) / 2
	local top = (state.height - height) / 2
	ui.frame("dialog", { left, top, left + width, top + height })
	ui.text("Level Complete", {
		style = "display",
		x = state.width / 2,
		y = top + 16,
		size = 20,
		color = INK,
		align = 0.5,
	})
	local function line(number, label, text)
		local y = top + 46 + number * 34
		ui.text(label, { x = left + 30, y = y + 5, size = 13, color = MUTED })
		ui.text(
			text,
			{ style = "bold", x = left + width - 30, y = y, size = 20, color = INK, align = 1 }
		)
	end
	line(0, "Time", clock(state.numbers.completed_time))
	line(1, "Secrets", string.format("%d / %d", stats.secrets or 0, stats.total_secrets or 0))
	line(2, "Monsters", string.format("%d / %d", stats.monsters or 0, stats.total_monsters or 0))
end

-- What's said in the middle of the screen. At the end of the game it's typed out.
local function draw_center(state, typed)
	local text = state.texts.center or ""
	if text == "" then
		return
	end
	-- Laid out whole, so that words being typed don't jump from line to line.
	local lines = {}
	for _, line in string.split(text, "\n") do
		for _, part in wrapped(line, state.width - 48, 14) do
			table.insert(lines, part)
		end
	end
	local spacing = math.min(20, (state.height - 64) / #lines)
	local y = if #lines <= 4 then state.height * 0.35 else (state.height - #lines * spacing) / 2
	local left = if typed then math.floor(state.numbers.center_age * TYPING_SPEED) else #text
	for _, line in lines do
		say(
			string.sub(line, 1, math.max(left, 0)),
			{ x = state.width / 2, y = y, size = 14, color = BRIGHT, align = 0.5 }
		)
		left -= #line + 1
		y += spacing
	end
end

-- What's happened that's for the player, at the top left, fading.
local function draw_messages(state)
	local y = 12
	for _, message in state.messages do
		local left = MESSAGE_SECONDS - message.age
		if left > 0 then
			local alpha = math.clamp(left / MESSAGE_FADE, 0, 1)
			for _, line in wrapped(message.text, state.width - 32, 11) do
				say(line, { x = 16, y = y, size = 11, color = { INK[1], INK[2], INK[3], alpha } })
				y += 15
			end
		end
	end
end

-- What a powerup does to the view.
local function powerup_color(stats)
	if stats.quad == 1 then
		return { 0, 0, 1, 30 / 255 }
	elseif stats.suit == 1 then
		return { 0, 1, 0, 20 / 255 }
	elseif stats.invisibility == 1 then
		return { 0.39, 0.39, 0.39, 100 / 255 }
	elseif stats.invulnerability == 1 then
		return { 1, 1, 0, 30 / 255 }
	end
	return nil
end

-- What's held, along the bottom.
local function draw_held(state, y)
	local stats = state.stats
	local x = state.width / 2 - 110
	for _, ammo in
		{
			{ "SHELLS", "shells" },
			{ "NAILS", "nails" },
			{ "ROCKETS", "rockets" },
			{ "CELLS", "cells" },
		}
	do
		say(ammo[1], { x = x, y = y, size = 8, color = MUTED })
		say(tostring(stats[ammo[2]] or 0), { x = x, y = y + 10, size = 13, color = INK })
		x += 58
	end
	local carried = {}
	if stats.key1 == 1 then
		table.insert(carried, "Silver Key")
	end
	if stats.key2 == 1 then
		table.insert(carried, "Gold Key")
	end
	if #carried > 0 then
		say(
			table.concat(carried, "  "),
			{ x = state.width / 2, y = y - 14, size = 10, color = INK, align = 0.5 }
		)
	end
end

function draw_status(state)
	local stats = state.stats
	local playing = state.numbers.playing == 1
	local ending = state.texts.intermission or ""
	if (state.texts.rate or "") ~= "" then
		say(state.texts.rate, { x = state.width - 12, y = 8, size = 9, color = MUTED, align = 1 })
	end
	if playing then
		draw_messages(state)
	end
	if ending == "results" and playing then
		if (stats.deathmatch or 0) ~= 0 then
			draw_scores(state, "Scores")
		else
			draw_results(state)
		end
		return
	elseif (ending == "finale" or ending == "cutscene") and playing then
		draw_center(state, true)
		return
	end
	local color = powerup_color(stats)
	if color then
		ui.fill({ 0, 0, state.width, state.height }, color)
	end
	if (state.settings.crosshair or 0) ~= 0 and playing then
		local x, y = state.width / 2, state.height / 2
		ui.fill({ x - 5, y - 0.5, x + 5, y + 0.5 }, BRIGHT)
		ui.fill({ x - 0.5, y - 5, x + 0.5, y + 5 }, BRIGHT)
	end
	if playing and (state.numbers.center_left or 0) > 0 then
		draw_center(state, false)
	end
	if stats.health == nil then
		return
	end
	local y = state.height - 48
	if (state.settings.hud_plate or 0) ~= 0 then
		ui.fill({ 0, y - 6, state.width, state.height }, { 0, 0, 0, 0.75 })
	end
	local function number(x, label, value, align)
		say(label, { x = x, y = y, size = 8, color = MUTED, align = align, tracking = 1 })
		local shade = if value <= 25 and label == "HEALTH" then ACCENT else INK
		say(
			tostring(value),
			{ style = "bold", x = x, y = y + 9, size = 20, color = shade, align = align }
		)
	end
	number(32, "HEALTH", stats.health, 0)
	number(108, "ARMOUR", stats.armor, 0)
	number(state.width - 32, "AMMO", stats.ammo, 1)
	draw_held(state, y)
	if state.numbers.paused == 1 then
		say("Paused", {
			style = "display",
			x = state.width / 2,
			y = state.height * 0.4,
			size = 24,
			color = BRIGHT,
			align = 0.5,
		})
	end
	if state.numbers.show_scores == 1 or stats.health <= 0 then
		if (stats.deathmatch or 0) ~= 0 then
			draw_scores(state, "Scores")
		elseif state.numbers.show_scores == 1 then
			local text = string.format(
				"%s    Monsters %d / %d    Secrets %d / %d    %s",
				state.texts.level_title or "",
				stats.monsters or 0,
				stats.total_monsters or 0,
				stats.secrets or 0,
				stats.total_secrets or 0,
				clock(state.numbers.game_time)
			)
			say(text, {
				x = state.width / 2,
				y = 16,
				size = size_to_fit(text, state.width - 32, 12),
				color = INK,
				align = 0.5,
			})
		end
	end
end
