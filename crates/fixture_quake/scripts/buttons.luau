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
-- Ported from buttons.qc.

function button_wait(self)
	self.state = STATE_TOP
	self.nextthink = self.ltime + self.wait
	self.think = button_return
	activator = self.enemy
	SUB_UseTargets(self)
	self.frame = 1
end

function button_done(self)
	self.state = STATE_BOTTOM
end

function button_return(self)
	self.state = STATE_DOWN
	SUB_CalcMove(self, self.pos1, self.speed, button_done)
	self.frame = 0
	if self.health ~= 0 then
		-- It can be shot again.
		self.takedamage = DAMAGE_YES
	end
end

function button_blocked(_self, _other) end

function button_fire(self)
	if self.state == STATE_UP or self.state == STATE_TOP then
		return
	end
	sound(self, CHAN_VOICE, self.noise, 1, ATTN_NORM)
	self.state = STATE_UP
	SUB_CalcMove(self, self.pos2, self.speed, button_wait)
end

function button_use(self, _other)
	self.enemy = activator
	button_fire(self)
end

function button_killed(self)
	self.enemy = damage_attacker
	self.health = self.max_health
	self.takedamage = DAMAGE_NO
	button_fire(self)
end

function button_touch(self, other)
	if other.classname ~= "player" then
		return
	end
	self.enemy = other
	button_fire(self)
end

function func_button(self)
	if self.sounds == 0 then
		self.noise = "buttons/airbut1.wav"
	elseif self.sounds == 1 then
		self.noise = "buttons/switch21.wav"
	elseif self.sounds == 2 then
		self.noise = "buttons/switch02.wav"
	elseif self.sounds == 3 then
		self.noise = "buttons/switch04.wav"
	end

	SetMovedir(self)

	self.movetype = MOVETYPE_PUSH
	self.solid = SOLID_BSP
	setmodel(self, self.model)

	self.blocked = button_blocked
	self.use = button_use

	if self.health ~= 0 then
		self.max_health = self.health
		self.th_die = button_killed
		self.takedamage = DAMAGE_YES
	else
		self.touch = button_touch
	end

	if self.speed == 0 then
		self.speed = 40
	end
	if self.wait == 0 then
		self.wait = 1
	end
	if self.lip == 0 then
		self.lip = 4
	end

	self.state = STATE_BOTTOM

	self.pos1 = self.origin
	local travel = math.abs(vector.dot(self.movedir, self.size)) - self.lip
	self.pos2 = self.pos1 + self.movedir * travel
end
