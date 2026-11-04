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
SOLID_BBOX = 2
SOLID_SLIDEBOX = 3
SOLID_BSP = 4

MOVETYPE_NONE = 0
MOVETYPE_WALK = 3
MOVETYPE_STEP = 4
MOVETYPE_FLY = 5
MOVETYPE_TOSS = 6
MOVETYPE_PUSH = 7
MOVETYPE_NOCLIP = 8
MOVETYPE_FLYMISSILE = 9
MOVETYPE_BOUNCE = 10

STATE_TOP = 0
STATE_BOTTOM = 1
STATE_UP = 2
STATE_DOWN = 3

CHAN_AUTO = 0
CHAN_WEAPON = 1
CHAN_VOICE = 2
CHAN_ITEM = 3
CHAN_BODY = 4

ATTN_NONE = 0
ATTN_NORM = 1
ATTN_IDLE = 2
ATTN_STATIC = 3

IT_SHOTGUN = 1
IT_SUPER_SHOTGUN = 2
IT_NAILGUN = 4
IT_SUPER_NAILGUN = 8
IT_GRENADE_LAUNCHER = 16
IT_ROCKET_LAUNCHER = 32
IT_LIGHTNING = 64
IT_SHELLS = 256
IT_NAILS = 512
IT_ROCKETS = 1024
IT_CELLS = 2048
IT_AXE = 4096
IT_ARMOR1 = 8192
IT_ARMOR2 = 16384
IT_ARMOR3 = 32768
IT_SUPERHEALTH = 65536
IT_KEY1 = 131072
IT_KEY2 = 262144
IT_INVISIBILITY = 524288
IT_INVULNERABILITY = 1048576
IT_SUIT = 2097152
IT_QUAD = 4194304

FL_FLY = 1
FL_SWIM = 2
FL_CLIENT = 8
FL_INWATER = 16
FL_MONSTER = 32
FL_GODMODE = 64
FL_NOTARGET = 128
FL_ITEM = 256
FL_ONGROUND = 512
FL_PARTIALGROUND = 1024

EF_BRIGHTFIELD = 1
EF_MUZZLEFLASH = 2
EF_BRIGHTLIGHT = 4
EF_DIMLIGHT = 8

DAMAGE_NO = 0
DAMAGE_YES = 1
DAMAGE_AIM = 2

DEAD_NO = 0
DEAD_DYING = 1
DEAD_DEAD = 2
DEAD_RESPAWNABLE = 3

CONTENT_EMPTY = -1
CONTENT_SOLID = -2
CONTENT_WATER = -3
CONTENT_SLIME = -4
CONTENT_LAVA = -5
CONTENT_SKY = -6

TE_SPIKE = 0
TE_SUPERSPIKE = 1
TE_GUNSHOT = 2
TE_EXPLOSION = 3
TE_TAREXPLOSION = 4
TE_LIGHTNING1 = 5
TE_LIGHTNING2 = 6
TE_WIZSPIKE = 7
TE_KNIGHTSPIKE = 8
TE_LIGHTNING3 = 9
TE_LAVASPLASH = 10
TE_TELEPORT = 11

RANGE_MELEE = 0
RANGE_NEAR = 1
RANGE_MID = 2
RANGE_FAR = 3

AS_STRAIGHT = 1
AS_SLIDING = 2
AS_MELEE = 3
AS_MISSILE = 4

VEC_ORIGIN = vector.zero
VEC_HULL_MIN = vector.create(-16, -16, -24)
VEC_HULL_MAX = vector.create(16, 16, 32)
VEC_HULL2_MIN = vector.create(-32, -32, -24)
VEC_HULL2_MAX = vector.create(32, 32, 64)

-- Fields that read as zero until they're set, as all of QuakeC's do.
set_field_defaults({
	ammo_cells = 0,
	ammo_nails = 0,
	ammo_rockets = 0,
	ammo_shells = 0,
	attack_finished = 0,
	attack_state = 0,
	cnt = 0,
	fixangle = 0,
	health = 0,
	inpain = 0,
	items = 0,
	lefty = 0,
	pain_finished = 0,
	pausetime = 0,
	search_time = 0,
	show_hostile = 0,
	spawnflags = 0,
	state = 0,
	takedamage = 0,
	waitmin = 0,
	movedir = vector.zero,
	view_ofs = vector.zero,
})

-- This is a single player game, for now.
deathmatch = 0
coop = 0

-- The last thing fired.
newmis = nil

-- How many monsters there are, and how many are dead.
total_monsters = 0
killed_monsters = 0

-- How far a monster moves on this step, kept for those that strafe.
movedist = 0

-- Set when the game ends.
intermission_running = 0
intermission_exittime = 0

-- The game's closing text, which is printed until there's a screen for it.
function finale(text)
	engine.print(text)
end

-- The entity that set off the current chain of events.
activator = nil

-- The world entity, set by worldspawn.
world = nil

-- Prints to the player, until text can be drawn.
function centerprint(_entity, message)
	engine.print(message)
end

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

-- What a `[frame, next]` function does before its code.
function animate(self, frame, think)
	self.frame = frame
	self.nextthink = time() + 0.1
	self.think = think
end

function SUB_AttackFinished(self, normal)
	self.cnt = 0 -- reset the refire counter
	if skill ~= 3 then
		self.attack_finished = time() + normal
	end
end

-- Runs `thinkst` again, for a nightmare monster that hasn't stopped firing.
function SUB_CheckRefire(self, thinkst)
	if skill ~= 3 or self.cnt == 1 or not visible(self, self.enemy) then
		return
	end
	self.cnt = 1
	self.think = thinkst
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
	-- An unset vector is zero in QuakeC. A secret door can link to a door, and
	-- then has no pos2.
	tdest = tdest or vector.zero
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
			sound(activator, CHAN_VOICE, "misc/talk.wav", 1, ATTN_NORM)
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
