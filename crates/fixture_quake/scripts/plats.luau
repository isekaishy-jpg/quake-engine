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
-- Ported from plats.qc.

local PLAT_LOW_TRIGGER = 1

function plat_spawn_inside_trigger(self)
	local trigger = spawn()
	trigger.touch = plat_center_touch
	trigger.movetype = MOVETYPE_NONE
	trigger.solid = SOLID_TRIGGER
	trigger.enemy = self

	local tmin = self.mins + vector.create(25, 25, 0)
	local tmax = self.maxs - vector.create(25, 25, -8)
	local min_z = tmax.z - (self.pos1.z - self.pos2.z + 8)
	local max_z = tmax.z
	if bit32.band(self.spawnflags, PLAT_LOW_TRIGGER) ~= 0 then
		max_z = min_z + 8
	end
	local min_x, max_x, min_y, max_y = tmin.x, tmax.x, tmin.y, tmax.y
	if self.size.x <= 50 then
		min_x = (self.mins.x + self.maxs.x) / 2
		max_x = min_x + 1
	end
	if self.size.y <= 50 then
		min_y = (self.mins.y + self.maxs.y) / 2
		max_y = min_y + 1
	end
	setsize(trigger, vector.create(min_x, min_y, min_z), vector.create(max_x, max_y, max_z))
end

function plat_hit_top(self)
	sound(self, CHAN_VOICE, self.noise1, 1, ATTN_NORM)
	self.state = STATE_TOP
	self.think = plat_go_down
	self.nextthink = self.ltime + 3
end

function plat_hit_bottom(self)
	sound(self, CHAN_VOICE, self.noise1, 1, ATTN_NORM)
	self.state = STATE_BOTTOM
end

function plat_go_down(self)
	sound(self, CHAN_VOICE, self.noise, 1, ATTN_NORM)
	self.state = STATE_DOWN
	SUB_CalcMove(self, self.pos2, self.speed, plat_hit_bottom)
end

function plat_go_up(self)
	sound(self, CHAN_VOICE, self.noise, 1, ATTN_NORM)
	self.state = STATE_UP
	SUB_CalcMove(self, self.pos1, self.speed, plat_hit_top)
end

function plat_center_touch(self, other)
	if other.classname ~= "player" or other.health <= 0 then
		return
	end
	local plat = self.enemy
	if plat.state == STATE_BOTTOM then
		plat_go_up(plat)
	elseif plat.state == STATE_TOP then
		plat.nextthink = plat.ltime + 1
	end
end

function plat_trigger_use(self, _other)
	if self.think then
		return
	end
	plat_go_down(self)
end

-- Sends the lift `entity` is on up, for what isn't a player and doesn't set off the
-- trigger on it.
function engine.call_lift(entity, place)
	for _, plat in find_all("classname", "plat") do
		local near =
			vector.magnitude(vector.create(plat.origin.x - place.x, plat.origin.y - place.y, 0))
		local middle = (plat.absmin + plat.absmax) * 0.5
		local across = vector.magnitude(vector.create(middle.x - place.x, middle.y - place.y, 0))
		if (near < 128 or across < 128) and plat.state == STATE_BOTTOM then
			plat_go_up(plat)
			return
		end
	end
end

function plat_crush(self, other)
	T_Damage(other, self, self, 1)
	if self.state == STATE_UP then
		plat_go_down(self)
	elseif self.state == STATE_DOWN then
		plat_go_up(self)
	else
		objerror("plat_crush: bad self.state")
	end
end

function plat_use(self, _other)
	self.use = SUB_Null
	if self.state ~= STATE_UP then
		objerror("plat_use: not in up state")
	end
	plat_go_down(self)
end

function func_plat(self)
	if self.t_length == 0 then
		self.t_length = 80
	end
	if self.t_width == 0 then
		self.t_width = 10
	end
	if self.sounds == 0 then
		self.sounds = 2
	end
	if self.sounds == 1 then
		self.noise = "plats/plat1.wav"
		self.noise1 = "plats/plat2.wav"
	elseif self.sounds == 2 then
		self.noise = "plats/medplat1.wav"
		self.noise1 = "plats/medplat2.wav"
	end

	self.mangle = self.angles
	self.angles = vector.zero

	self.classname = "plat"
	self.solid = SOLID_BSP
	self.movetype = MOVETYPE_PUSH
	setmodel(self, self.model)

	self.blocked = plat_crush
	if self.speed == 0 then
		self.speed = 150
	end

	-- pos1 is the top position, pos2 is the bottom.
	self.pos1 = self.origin
	local origin = self.origin
	local drop = if self.height ~= 0 then self.height else self.size.z - 8
	self.pos2 = vector.create(origin.x, origin.y, origin.z - drop)

	self.use = plat_trigger_use

	plat_spawn_inside_trigger(self)

	if has_text(self.targetname) then
		self.state = STATE_UP
		self.use = plat_use
	else
		setorigin(self, self.pos2)
		self.state = STATE_BOTTOM
	end
end

function train_blocked(self, other)
	if time() < (self.attack_finished or 0) then
		return
	end
	self.attack_finished = time() + 0.5
	T_Damage(other, self, self, self.dmg)
end

function train_use(self, _other)
	if self.think ~= func_train_find then
		return
	end
	train_next(self)
end

function train_wait(self)
	if (self.wait or 0) ~= 0 then
		self.nextthink = self.ltime + self.wait
		sound(self, CHAN_VOICE, self.noise, 1, ATTN_NORM)
	else
		self.nextthink = self.ltime + 0.1
	end
	self.think = train_next
end

function train_next(self)
	local targ = find(nil, "targetname", self.target)
	if targ == nil then
		objerror("train_next: no next target")
	end
	self.target = targ.target
	if not has_text(self.target) then
		objerror("train_next: no next target")
	end
	self.wait = targ.wait or 0
	sound(self, CHAN_VOICE, self.noise1, 1, ATTN_NORM)
	SUB_CalcMove(self, targ.origin - self.mins, self.speed, train_wait)
end

function func_train_find(self)
	local targ = find(nil, "targetname", self.target)
	if targ == nil then
		objerror("func_train_find: no target")
	end
	self.target = targ.target
	setorigin(self, targ.origin - self.mins)
	if not has_text(self.targetname) then
		-- Not triggered, so start immediately.
		self.nextthink = self.ltime + 0.1
		self.think = train_next
	end
end

function func_train(self)
	if self.speed == 0 then
		self.speed = 100
	end
	if not has_text(self.target) then
		objerror("func_train without a target")
	end
	if self.dmg == 0 then
		self.dmg = 2
	end
	if self.sounds == 0 then
		self.noise = "misc/null.wav"
		self.noise1 = "misc/null.wav"
	elseif self.sounds == 1 then
		self.noise = "plats/train2.wav"
		self.noise1 = "plats/train1.wav"
	end

	self.cnt = 1
	self.solid = SOLID_BSP
	self.movetype = MOVETYPE_PUSH
	self.blocked = train_blocked
	self.use = train_use
	self.classname = "train"
	setmodel(self, self.model)

	-- Start trains on the second frame, to make sure their targets have had a
	-- chance to spawn.
	self.nextthink = self.ltime + 0.1
	self.think = func_train_find
end

-- Carries the player's way to the final boss. Its model waits for models.
function misc_teleporttrain(self)
	if self.speed == 0 then
		self.speed = 100
	end
	if not has_text(self.target) then
		objerror("func_train without a target")
	end

	self.cnt = 1
	self.solid = SOLID_NOT
	self.movetype = MOVETYPE_PUSH
	self.blocked = train_blocked
	self.use = train_use
	self.avelocity = vector.create(100, 200, 300)
	self.noise = "misc/null.wav"
	self.noise1 = "misc/null.wav"
	setmodel(self, "progs/teleport.mdl")
	setsize(self, self.mins, self.maxs)
	setorigin(self, self.origin)

	self.nextthink = self.ltime + 0.1
	self.think = func_train_find
end
