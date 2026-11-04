--  Copyright (C) 1996-1997  Id Software, Inc.
--
--  This program is free software; you can redistribute it and/or modify
--  it under the terms of the GNU General Public License as published by
--  the Free Software Foundation; either version 2 of the License, or
--  (at your option) any later version.
--
--  This program is distributed in the hope that it will be useful,
--  but WITHOUT ANY WARRANTY; without even the implied warranty of
--  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
--  GNU General Public License for more details.
--
--  See the LICENSE file for details.
--
-- Ported from world.qc. Letters run from a for dark to z for bright, with m as
-- normal.

local LIGHT_STYLES = {
	[0] = "m",
	[1] = "mmnmmommommnonmmonqnmmo",
	[2] = "abcdefghijklmnopqrstuvwxyzyxwvutsrqponmlkjihgfedcba",
	[3] = "mmmmmaaaaammmmmaaaaaabcdefgabcdefg",
	[4] = "mamamamamama",
	[5] = "jklmnopqrstuvwxyzyxwvutsrqponmlkj",
	[6] = "nmonqnmomnmomomno",
	[7] = "mmmaaaabcdefgmmmmaaaammmaamm",
	[8] = "mmmaaammmaaammmabcdefaaaammmmabcdefmmmaaaa",
	[9] = "aaaaaaaazzzzzzzz",
	[10] = "mmamammmmammamamaaamammma",
	[11] = "abcdefghijklmnopqrrqponmlkjihgfedcba",
	[63] = "a",
}

function worldspawn(self)
	world = self
	-- A client draws these the moment a shot's fired, before the server has
	-- spawned one to name them by.
	engine.precache_model("progs/missile.mdl")
	engine.precache_model("progs/grenade.mdl")
	if self.sounds ~= 0 then
		engine.music(self.sounds)
	end
	for index, pattern in LIGHT_STYLES do
		engine.lightstyle(index, pattern)
	end
end
