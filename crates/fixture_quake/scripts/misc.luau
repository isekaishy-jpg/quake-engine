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
-- Ported from the parts of misc.qc that maps need without models.

local START_OFF = 1

-- Runes held, which saves will carry between levels.
serverflags = 0

function info_null(self)
	remove(self)
end

function info_notnull(_self) end

function light_use(self, _other)
	if bit32.band(self.spawnflags, START_OFF) ~= 0 then
		engine.lightstyle(self.style, "m")
		self.spawnflags -= START_OFF
	else
		engine.lightstyle(self.style, "a")
		self.spawnflags += START_OFF
	end
end

function light(self)
	if not has_text(self.targetname) then
		remove(self)
		return
	end
	if self.style >= 32 then
		self.use = light_use
		if bit32.band(self.spawnflags, START_OFF) ~= 0 then
			engine.lightstyle(self.style, "a")
		else
			engine.lightstyle(self.style, "m")
		end
	end
end

light_fluoro = light

function func_wall_use(self, _other)
	self.frame = 1 - (self.frame or 0)
end

function func_wall(self)
	self.angles = vector.zero
	self.movetype = MOVETYPE_PUSH
	self.solid = SOLID_BSP
	self.use = func_wall_use
	setmodel(self, self.model)
end

function func_illusionary(self)
	self.angles = vector.zero
	self.movetype = MOVETYPE_NONE
	self.solid = SOLID_NOT
	setmodel(self, self.model)
end

function func_episodegate(self)
	if bit32.band(serverflags, self.spawnflags) == 0 then
		return
	end
	func_wall(self)
end

function func_bossgate(self)
	if bit32.band(serverflags, 15) == 15 then
		return
	end
	func_wall(self)
end
