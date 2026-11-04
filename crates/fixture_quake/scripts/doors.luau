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
-- Ported from doors.qc.
--
-- Doors are similar to buttons, but can spawn a fat trigger field around them
-- to open without a touch, and they link together to form simultaneous
-- double or quad doors.
--
-- door.owner is the master door. If there is only one door, it points to
-- itself. door.enemy chains from the master door through all linked doors.

local DOOR_START_OPEN = 1
local DOOR_DONT_LINK = 4
local DOOR_GOLD_KEY = 8
local DOOR_SILVER_KEY = 16
local DOOR_TOGGLE = 32

local function has_flag(self, flag)
	return bit32.band(self.spawnflags or 0, flag) ~= 0
end

function door_blocked(self, other)
	T_Damage(other, self, self, self.dmg)

	-- If a door has a negative wait, it would never come back if blocked, so
	-- let it just squash the object.
	if self.wait >= 0 then
		if self.state == STATE_DOWN then
			door_go_up(self)
		else
			door_go_down(self)
		end
	end
end

function door_hit_top(self)
	sound(self, self.noise1)
	self.state = STATE_TOP
	if has_flag(self, DOOR_TOGGLE) then
		return
	end
	self.think = door_go_down
	self.nextthink = self.ltime + self.wait
end

function door_hit_bottom(self)
	sound(self, self.noise1)
	self.state = STATE_BOTTOM
end

function door_go_down(self)
	sound(self, self.noise2)
	self.state = STATE_DOWN
	SUB_CalcMove(self, self.pos1, self.speed, door_hit_bottom)
end

function door_go_up(self)
	if self.state == STATE_UP then
		return
	end
	if self.state == STATE_TOP then
		self.nextthink = self.ltime + self.wait
		return
	end
	sound(self, self.noise2)
	self.state = STATE_UP
	SUB_CalcMove(self, self.pos2, self.speed, door_hit_top)
	SUB_UseTargets(self)
end

function door_fire(self)
	if self.owner ~= self then
		objerror("door_fire: self.owner != self")
	end
	if (self.items or 0) ~= 0 then
		sound(self, self.noise4)
	end
	self.message = ""

	if has_flag(self, DOOR_TOGGLE) and (self.state == STATE_UP or self.state == STATE_TOP) then
		local door = self
		repeat
			door_go_down(door)
			door = door.enemy
		until door == self or door == nil
		return
	end

	-- Trigger all paired doors.
	local door = self
	repeat
		door_go_up(door)
		door = door.enemy
	until door == self or door == nil
end

function door_use(self, _other)
	-- A door used before it links, in its first tenth of a second, has no owner.
	local owner = self.owner
	if owner == nil then
		return
	end
	self.message = ""
	owner.message = ""
	if self.enemy then
		self.enemy.message = ""
	end
	door_fire(owner)
end

function door_trigger_touch(self, other)
	if other.health <= 0 then
		return
	end
	if time() < (self.attack_finished or 0) then
		return
	end
	self.attack_finished = time() + 1
	activator = other
	door_use(self.owner)
end

-- Prints messages and opens key doors.
function door_touch(self, other)
	if other.classname ~= "player" then
		return
	end
	-- A door touched before it links, in its first tenth of a second, has no
	-- owner.
	if self.owner == nil then
		return
	end
	if (self.owner.attack_finished or 0) > time() then
		return
	end
	self.owner.attack_finished = time() + 2

	if has_text(self.owner.message) then
		centerprint(other, self.owner.message)
		sound(other, "misc/talk.wav")
	end

	if (self.items or 0) == 0 then
		return
	end

	if bit32.band(self.items, other.items) ~= self.items then
		local key = if self.owner.items == IT_KEY1 then "silver" else "gold"
		local worldtype = world and world.worldtype or 0
		local kind = if worldtype == 2 then "keycard" elseif worldtype == 1 then "runekey" else "key"
		centerprint(other, "You need the " .. key .. " " .. kind)
		sound(self, self.noise3)
		return
	end

	other.items = other.items - self.items
	self.touch = SUB_Null
	if self.enemy then
		self.enemy.touch = SUB_Null
	end
	door_use(self)
end

local function spawn_field(owner, fmins, fmaxs)
	local trigger = spawn()
	trigger.movetype = MOVETYPE_NONE
	trigger.solid = SOLID_TRIGGER
	trigger.owner = owner
	trigger.touch = door_trigger_touch
	setsize(trigger, fmins - vector.create(60, 60, 8), fmaxs + vector.create(60, 60, 8))
	return trigger
end

local function EntitiesTouching(e1, e2)
	local a_min, a_max, b_min, b_max = e1.mins, e1.maxs, e2.mins, e2.maxs
	return not (
		a_min.x > b_max.x
		or a_min.y > b_max.y
		or a_min.z > b_max.z
		or a_max.x < b_min.x
		or a_max.y < b_min.y
		or a_max.z < b_min.z
	)
end

function LinkDoors(self)
	if self.enemy then
		return
	end
	if has_flag(self, DOOR_DONT_LINK) then
		self.owner = self
		self.enemy = self
		return
	end

	local cmins, cmaxs = self.mins, self.maxs
	local starte = self
	local door = self
	local t = self

	while true do
		door.owner = starte

		if (door.health or 0) ~= 0 then
			starte.health = door.health
		end
		if has_text(door.targetname) then
			starte.targetname = door.targetname
		end
		if has_text(door.message) then
			starte.message = door.message
		end

		t = find(t, "classname", door.classname)
		if t == nil then
			door.enemy = starte
			local master = door.owner
			if (master.health or 0) ~= 0 then
				return
			end
			if has_text(master.targetname) then
				return
			end
			if (master.items or 0) ~= 0 then
				return
			end
			master.owner.trigger_field = spawn_field(master, cmins, cmaxs)
			return
		end

		if EntitiesTouching(door, t) then
			if t.enemy then
				objerror("cross connected doors")
			end
			door.enemy = t
			door = t
			cmins = vector.min(cmins, t.mins)
			cmaxs = vector.max(cmaxs, t.maxs)
		end
	end
end

function func_door(self)
	local worldtype = world and world.worldtype or 0
	if worldtype == 0 then
		self.noise3 = "doors/medtry.wav"
		self.noise4 = "doors/meduse.wav"
	elseif worldtype == 1 then
		self.noise3 = "doors/runetry.wav"
		self.noise4 = "doors/runeuse.wav"
	else
		self.noise3 = "doors/basetry.wav"
		self.noise4 = "doors/baseuse.wav"
	end
	if self.sounds == 0 then
		self.noise1 = "misc/null.wav"
		self.noise2 = "misc/null.wav"
	elseif self.sounds == 1 then
		self.noise1 = "doors/drclos4.wav"
		self.noise2 = "doors/doormv1.wav"
	elseif self.sounds == 2 then
		self.noise2 = "doors/hydro1.wav"
		self.noise1 = "doors/hydro2.wav"
	elseif self.sounds == 3 then
		self.noise2 = "doors/stndr1.wav"
		self.noise1 = "doors/stndr2.wav"
	elseif self.sounds == 4 then
		self.noise1 = "doors/ddoor2.wav"
		self.noise2 = "doors/ddoor1.wav"
	end

	SetMovedir(self)

	self.max_health = self.health
	self.solid = SOLID_BSP
	self.movetype = MOVETYPE_PUSH
	setmodel(self, self.model)
	self.classname = "door"

	self.blocked = door_blocked
	self.use = door_use

	self.items = 0
	if has_flag(self, DOOR_SILVER_KEY) then
		self.items = IT_KEY1
	end
	if has_flag(self, DOOR_GOLD_KEY) then
		self.items = IT_KEY2
	end

	if self.speed == 0 then
		self.speed = 100
	end
	if self.wait == 0 then
		self.wait = 3
	end
	if self.lip == 0 then
		self.lip = 8
	end
	if self.dmg == 0 then
		self.dmg = 2
	end

	self.pos1 = self.origin
	local travel = math.abs(vector.dot(self.movedir, self.size)) - self.lip
	self.pos2 = self.pos1 + self.movedir * travel

	-- DOOR_START_OPEN is to allow an entity to be lighted in the closed
	-- position but spawn in the open position.
	if has_flag(self, DOOR_START_OPEN) then
		setorigin(self, self.pos2)
		self.pos2 = self.pos1
		self.pos1 = self.origin
	end

	self.state = STATE_BOTTOM

	if self.items ~= 0 then
		self.wait = -1
	end

	self.touch = door_touch

	-- LinkDoors can't be done until all of the doors have been spawned, so the
	-- sizes can be detected properly.
	self.think = LinkDoors
	self.nextthink = self.ltime + 0.1
end

-- Secret doors.

local SECRET_OPEN_ONCE = 1
local SECRET_1ST_LEFT = 2
local SECRET_1ST_DOWN = 4

function fd_secret_use(self, _other)
	if self.origin ~= self.oldorigin then
		return
	end
	self.message = ""
	SUB_UseTargets(self)
	self.velocity = vector.zero
	sound(self, self.noise1)
	self.nextthink = self.ltime + 0.1

	local temp = 1 - bit32.band(self.spawnflags, SECRET_1ST_LEFT)
	local forward, right, up = makevectors(self.mangle)

	if self.t_width == 0 then
		if has_flag(self, SECRET_1ST_DOWN) then
			self.t_width = math.abs(vector.dot(up, self.size))
		else
			self.t_width = math.abs(vector.dot(right, self.size))
		end
	end
	if self.t_length == 0 then
		self.t_length = math.abs(vector.dot(forward, self.size))
	end

	if has_flag(self, SECRET_1ST_DOWN) then
		self.dest1 = self.origin - up * self.t_width
	else
		self.dest1 = self.origin + right * (self.t_width * temp)
	end
	self.dest2 = self.dest1 + forward * self.t_length
	SUB_CalcMove(self, self.dest1, self.speed, fd_secret_move1)
	sound(self, self.noise2)
end

function fd_secret_move1(self)
	self.nextthink = self.ltime + 1
	self.think = fd_secret_move2
	sound(self, self.noise3)
end

function fd_secret_move2(self)
	sound(self, self.noise2)
	SUB_CalcMove(self, self.dest2, self.speed, fd_secret_move3)
end

function fd_secret_move3(self)
	sound(self, self.noise3)
	if not has_flag(self, SECRET_OPEN_ONCE) then
		self.nextthink = self.ltime + self.wait
		self.think = fd_secret_move4
	end
end

function fd_secret_move4(self)
	sound(self, self.noise2)
	SUB_CalcMove(self, self.dest1, self.speed, fd_secret_move5)
end

function fd_secret_move5(self)
	self.nextthink = self.ltime + 1
	self.think = fd_secret_move6
	sound(self, self.noise3)
end

function fd_secret_move6(self)
	sound(self, self.noise2)
	SUB_CalcMove(self, self.oldorigin, self.speed, fd_secret_done)
end

function fd_secret_done(self)
	sound(self, self.noise3)
end

function secret_blocked(self, other)
	if time() < (self.attack_finished or 0) then
		return
	end
	self.attack_finished = time() + 0.5
	T_Damage(other, self, self, self.dmg)
end

function secret_touch(self, other)
	if other.classname ~= "player" then
		return
	end
	if (self.attack_finished or 0) > time() then
		return
	end
	self.attack_finished = time() + 2
	if has_text(self.message) then
		centerprint(other, self.message)
		sound(other, "misc/talk.wav")
	end
end

function func_door_secret(self)
	if self.sounds == 0 then
		self.sounds = 3
	end
	if self.sounds == 1 then
		self.noise1 = "doors/latch2.wav"
		self.noise2 = "doors/winch2.wav"
		self.noise3 = "doors/drclos4.wav"
	elseif self.sounds == 2 then
		self.noise2 = "doors/airdoor1.wav"
		self.noise1 = "doors/airdoor2.wav"
		self.noise3 = "doors/airdoor2.wav"
	elseif self.sounds == 3 then
		self.noise2 = "doors/basesec1.wav"
		self.noise1 = "doors/basesec2.wav"
		self.noise3 = "doors/basesec2.wav"
	end

	if self.dmg == 0 then
		self.dmg = 2
	end

	self.mangle = self.angles
	self.angles = vector.zero
	self.solid = SOLID_BSP
	self.movetype = MOVETYPE_PUSH
	self.classname = "door"
	setmodel(self, self.model)

	self.touch = secret_touch
	self.blocked = secret_blocked
	self.speed = 50
	self.use = fd_secret_use
	self.oldorigin = self.origin
	if self.wait == 0 then
		self.wait = 5
	end
end
