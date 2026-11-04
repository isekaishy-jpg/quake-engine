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
-- Ported from the level changing and player sound parts of client.qc. The
-- intermission waits for a HUD.

function changelevel_touch(self, other)
	if other.classname ~= "player" then
		return
	end
	activator = other
	SUB_UseTargets(self)
	self.touch = SUB_Null
	engine.changelevel(self.map)
end

function trigger_changelevel(self)
	if not has_text(self.map) then
		objerror("changelevel trigger doesn't have map")
	end
	InitTrigger(self)
	self.touch = changelevel_touch
end

local CONTENT_WATER = -3
local CONTENT_SLIME = -4
local CONTENT_LAVA = -5

function PlayerJump(self)
	if self.waterjump then
		return
	end

	if self.waterlevel >= 2 then
		-- Play a swimming sound.
		if (self.swim_flag or 0) < time() then
			self.swim_flag = time() + 1
			local sample = if random() < 0.5 then "misc/water1.wav" else "misc/water2.wav"
			sound(self, CHAN_BODY, sample, 1, ATTN_NORM)
		end
		return
	end

	if not self.onground then
		return
	end

	-- Don't pogo stick.
	if not self.jump_released then
		return
	end

	self.jump_released = false
	sound(self, CHAN_BODY, "player/plyrjmp8.wav", 1, ATTN_NORM)
end

function WaterMove(self)
	if self.noclip then
		return
	end

	if self.waterlevel == 0 then
		if self.in_water then
			sound(self, CHAN_BODY, "misc/outwater.wav", 1, ATTN_NORM)
			self.in_water = false
		end
		return
	end

	if not self.in_water then
		if self.watertype == CONTENT_LAVA then
			sound(self, CHAN_BODY, "player/inlava.wav", 1, ATTN_NORM)
		elseif self.watertype == CONTENT_WATER then
			sound(self, CHAN_BODY, "player/inh2o.wav", 1, ATTN_NORM)
		elseif self.watertype == CONTENT_SLIME then
			sound(self, CHAN_BODY, "player/slimbrn2.wav", 1, ATTN_NORM)
		end
		self.in_water = true
	end
end

-- Runs before each of the player's moves.
function PlayerPreThink(self)
	WaterMove(self)
	if self.button2 then
		PlayerJump(self)
	else
		self.jump_released = true
	end
end

-- Runs after each of the player's moves.
function PlayerPostThink(self)
	local fall = self.jump_flag or 0
	if fall < -300 and self.onground then
		if self.watertype == CONTENT_WATER then
			sound(self, CHAN_BODY, "player/h2ojump.wav", 1, ATTN_NORM)
		elseif fall < -650 then
			T_Damage(self, world, world, 5)
			sound(self, CHAN_VOICE, "player/land2.wav", 1, ATTN_NORM)
		else
			sound(self, CHAN_VOICE, "player/land.wav", 1, ATTN_NORM)
		end
	end
	self.jump_flag = self.velocity.z
end
