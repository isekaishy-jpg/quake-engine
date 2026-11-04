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
-- Ported from subs.qc and the constants of defs.qc.

SOLID_NOT = 0
SOLID_TRIGGER = 1
SOLID_BSP = 4

MOVETYPE_NONE = 0
MOVETYPE_PUSH = 7

STATE_TOP = 0
STATE_BOTTOM = 1
STATE_UP = 2
STATE_DOWN = 3

IT_KEY1 = 131072
IT_KEY2 = 262144

-- The entity that set off the current chain of events.
activator = nil

-- The world entity, set by worldspawn.
world = nil

-- Stands in for sound until audio arrives.
function sound(_entity, _sample) end

-- Prints to the player, until text can be drawn.
function centerprint(_entity, message)
	engine.print(message)
end

-- Stands in for damage until combat arrives.
function T_Damage(_target, _inflictor, _attacker, _damage) end

function objerror(message)
	error(message)
end

function has_text(text)
	return text ~= nil and text ~= ""
end

-- Map entities can be limited to some skill levels, or to deathmatch, as
-- `ED_LoadFromFile` does in pr_edict.c.
local SPAWNFLAG_NOT_EASY = 256
local SPAWNFLAG_NOT_MEDIUM = 512
local SPAWNFLAG_NOT_HARD = 1024

-- The difficulty, from 0 to 3.
skill = 1

function spawn_filter(self)
	local flag = if skill == 0
		then SPAWNFLAG_NOT_EASY
		elseif skill == 1 then SPAWNFLAG_NOT_MEDIUM
		else SPAWNFLAG_NOT_HARD
	return bit32.band(self.spawnflags, flag) == 0
end

function SUB_Null(_self) end

function SUB_Remove(self)
	remove(self)
end

-- QuakeEd only writes a single float for angles, so up and down are just
-- constant angles.
function SetMovedir(self)
	if self.angles == vector.create(0, -1, 0) then
		self.movedir = vector.create(0, 0, 1)
	elseif self.angles == vector.create(0, -2, 0) then
		self.movedir = vector.create(0, 0, -1)
	else
		self.movedir = makevectors(self.angles)
	end
	self.angles = vector.zero
end

function InitTrigger(self)
	-- Trigger angles are used for one-way touches. An angle of 0 is assumed to
	-- mean no restrictions, so use a yaw of 360 instead.
	if self.angles ~= vector.zero then
		SetMovedir(self)
	else
		self.movedir = vector.zero
	end
	self.solid = SOLID_TRIGGER
	setmodel(self, self.model)
	self.movetype = MOVETYPE_NONE
	self.visible = false
	self.model = ""
end

-- Calculates velocity and nextthink to reach `tdest` at `tspeed`.
function SUB_CalcMove(self, tdest, tspeed, func)
	if tspeed == 0 then
		objerror("No speed is defined!")
	end
	self.think1 = func
	self.finaldest = tdest
	self.think = SUB_CalcMoveDone

	if tdest == self.origin then
		self.velocity = vector.zero
		self.nextthink = self.ltime + 0.1
		return
	end

	local delta = tdest - self.origin
	local traveltime = vector.magnitude(delta) / tspeed
	if traveltime < 0.1 then
		self.velocity = vector.zero
		self.nextthink = self.ltime + 0.1
		return
	end

	self.nextthink = self.ltime + traveltime
	self.velocity = delta * (1 / traveltime)
end

-- After moving, set origin to the exact final destination.
function SUB_CalcMoveDone(self)
	setorigin(self, self.finaldest)
	self.velocity = vector.zero
	self.nextthink = -1
	if self.think1 then
		self.think1(self)
	end
end

function DelayThink(self)
	activator = self.enemy
	SUB_UseTargets(self)
	remove(self)
end

-- Fires everything `self.target` names. `activator` should be set to the
-- entity that started it. A delay creates a temporary entity to fire later.
function SUB_UseTargets(self)
	if (self.delay or 0) ~= 0 then
		local t = spawn()
		t.classname = "DelayedUse"
		t.nextthink = time() + self.delay
		t.think = DelayThink
		t.enemy = activator
		t.message = self.message
		t.killtarget = self.killtarget
		t.target = self.target
		return
	end

	if activator and activator.classname == "player" and has_text(self.message) then
		centerprint(activator, self.message)
		if not self.noise then
			sound(activator, "misc/talk.wav")
		end
	end

	if has_text(self.killtarget) then
		for _, t in find_all("targetname", self.killtarget) do
			remove(t)
		end
		return
	end

	if has_text(self.target) then
		local act = activator
		for _, t in find_all("targetname", self.target) do
			if t.use and not is_removed(t) then
				t.use(t, self)
			end
			activator = act
		end
	end
end
