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
-- Ported from misc.qc.

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

local function switchable(self)
	if self.style >= 32 then
		self.use = light_use
		if bit32.band(self.spawnflags, START_OFF) ~= 0 then
			engine.lightstyle(self.style, "a")
		else
			engine.lightstyle(self.style, "m")
		end
	end
end

function light(self)
	if not has_text(self.targetname) then
		remove(self)
		return
	end
	switchable(self)
end

function light_fluoro(self)
	switchable(self)
	ambientsound(self.origin, "ambience/fl_hum1.wav", 0.5, ATTN_STATIC)
end

function light_fluorospark(self)
	if self.style == 0 then
		self.style = 10
	end
	ambientsound(self.origin, "ambience/buzz1.wav", 0.5, ATTN_STATIC)
end

local function FireAmbient(self)
	ambientsound(self.origin, "ambience/fire1.wav", 0.5, ATTN_STATIC)
end

function light_torch_small_walltorch(self)
	setmodel(self, "progs/flame.mdl")
	FireAmbient(self)
end

function light_flame_large_yellow(self)
	setmodel(self, "progs/flame2.mdl")
	self.frame = 1
	FireAmbient(self)
end

function light_flame_small_yellow(self)
	setmodel(self, "progs/flame2.mdl")
	FireAmbient(self)
end

light_flame_small_white = light_flame_small_yellow

local function ambient(sample, volume)
	return function(self)
		ambientsound(self.origin, sample, volume, ATTN_STATIC)
	end
end

ambient_suck_wind = ambient("ambience/suck1.wav", 1)
ambient_drone = ambient("ambience/drone6.wav", 0.5)
ambient_flouro_buzz = ambient("ambience/buzz1.wav", 1)
ambient_drip = ambient("ambience/drip1.wav", 0.5)
ambient_comp_hum = ambient("ambience/comp1.wav", 1)
ambient_thunder = ambient("ambience/thunder1.wav", 0.5)
ambient_light_buzz = ambient("ambience/fl_hum1.wav", 0.5)
ambient_swamp1 = ambient("ambience/swamp1.wav", 0.5)
ambient_swamp2 = ambient("ambience/swamp2.wav", 0.5)

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

function light_globe(self)
	setmodel(self, "progs/s_light.spr")
end

function fire_touch(self, other)
	T_Damage(other, self, self, 20)
	remove(self)
end

function fire_fly(self)
	local fireball = spawn()
	fireball.solid = SOLID_TRIGGER
	fireball.movetype = MOVETYPE_TOSS
	fireball.velocity =
		vector.create(random() * 100 - 50, random() * 100 - 50, self.speed + random() * 200)
	fireball.classname = "fireball"
	setmodel(fireball, "progs/lavaball.mdl")
	setsize(fireball, vector.zero, vector.zero)
	setorigin(fireball, self.origin)
	fireball.nextthink = time() + 5
	fireball.think = SUB_Remove
	fireball.touch = fire_touch

	self.nextthink = time() + random() * 5 + 3
	self.think = fire_fly
end

-- The original means to default the speed to 1000 here, but compares it
-- instead, so maps set their own.
function misc_fireball(self)
	self.classname = "fireball"
	self.nextthink = time() + random() * 5
	self.think = fire_fly
end

function barrel_explode(self)
	self.takedamage = DAMAGE_NO
	self.classname = "explo_box"
	T_RadiusDamage(self, self, 160, world)
	sound(self, CHAN_VOICE, "weapons/r_exp3.wav", 1, ATTN_NORM)
	particle(self.origin, vector.zero, 75, 255)
	setorigin(self, self.origin + vector.create(0, 0, 32))
	BecomeExplosion(self)
end

local function explobox(self, model, maxs)
	self.solid = SOLID_BBOX
	self.movetype = MOVETYPE_NONE
	setmodel(self, model)
	-- The original takes the size from the model.
	setsize(self, vector.create(1, 1, 1), maxs)
	self.health = 20
	self.th_die = barrel_explode
	self.takedamage = DAMAGE_AIM

	setorigin(self, self.origin + vector.create(0, 0, 2))
	local oldz = self.origin.z
	droptofloor(self)
	if oldz - self.origin.z > 250 then
		remove(self)
	end
end

function misc_explobox(self)
	explobox(self, "maps/b_explob.bsp", vector.create(31, 31, 63))
end

function misc_explobox2(self)
	explobox(self, "maps/b_exbox2.bsp", vector.create(31, 31, 31))
end

local SPAWNFLAG_SUPERSPIKE = 1
local SPAWNFLAG_LASER = 2

-- Returns what it fired.
function spikeshooter_use(self, _other)
	if bit32.band(self.spawnflags, SPAWNFLAG_LASER) ~= 0 then
		sound(self, CHAN_VOICE, "enforcer/enfire.wav", 1, ATTN_NORM)
		return LaunchLaser(self, self.origin, self.movedir)
	end
	sound(self, CHAN_VOICE, "weapons/spike2.wav", 1, ATTN_NORM)
	local spike = launch_spike(self, self.origin, self.movedir, 500)
	if bit32.band(self.spawnflags, SPAWNFLAG_SUPERSPIKE) ~= 0 then
		spike.touch = superspike_touch
	end
	return spike
end

function shooter_think(self)
	local fired = spikeshooter_use(self)
	self.nextthink = time() + self.wait
	fired.velocity = self.movedir * 500
end

function trap_spikeshooter(self)
	SetMovedir(self)
	self.use = spikeshooter_use
end

function trap_shooter(self)
	trap_spikeshooter(self)
	if self.wait == 0 then
		self.wait = 1
	end
	self.nextthink = self.nextthink + self.wait + self.ltime
	self.think = shooter_think
end

local function new_bubble(self, velocity, frame, count)
	local bubble = spawn()
	setmodel(bubble, "progs/s_bubble.spr")
	setorigin(bubble, self.origin)
	bubble.movetype = MOVETYPE_NOCLIP
	bubble.solid = SOLID_NOT
	bubble.velocity = velocity
	bubble.nextthink = time() + 0.5
	bubble.think = bubble_bob
	bubble.touch = bubble_remove
	bubble.classname = "bubble"
	bubble.frame = frame
	bubble.cnt = count
	setsize(bubble, vector.create(-8, -8, -8), vector.create(8, 8, 8))
end

function make_bubbles(self)
	new_bubble(self, vector.create(0, 0, 15), 0, 0)
	self.nextthink = time() + random() + 0.5
	self.think = make_bubbles
end

function air_bubbles(self)
	self.nextthink = time() + 1
	self.think = make_bubbles
end

-- Returns whether the bubble is gone.
function bubble_split(self)
	new_bubble(self, self.velocity, 1, 10)
	self.frame = 1
	self.cnt = 10
	if self.waterlevel ~= 3 then
		remove(self)
		return true
	end
	return false
end

function bubble_remove(self, other)
	if other.classname ~= self.classname then
		remove(self)
	end
end

function bubble_bob(self)
	self.cnt += 1
	if self.cnt == 4 and bubble_split(self) then
		return
	end
	if self.cnt == 20 then
		remove(self)
		return
	end

	local velocity = self.velocity
	local rnd1 = velocity.x + (-10 + random() * 20)
	local rnd2 = velocity.y + (-10 + random() * 20)
	local rnd3 = velocity.z + 10 + random() * 10

	if rnd1 > 10 then
		rnd1 = 5
	end
	if rnd1 < -10 then
		rnd1 = -5
	end
	if rnd2 > 10 then
		rnd2 = 5
	end
	if rnd2 < -10 then
		rnd2 = -5
	end
	if rnd3 < 10 then
		rnd3 = 15
	end
	if rnd3 > 30 then
		rnd3 = 25
	end

	self.velocity = vector.create(rnd1, rnd2, rnd3)
	self.nextthink = time() + 0.5
	self.think = bubble_bob
end

function viewthing(self)
	self.movetype = MOVETYPE_NONE
	self.solid = SOLID_NOT
	setmodel(self, "progs/player.mdl")
end

function noise_think(self)
	self.nextthink = time() + 0.5
	sound(self, 1, "enforcer/enfire.wav", 1, ATTN_NORM)
	sound(self, 2, "enforcer/enfstop.wav", 1, ATTN_NORM)
	sound(self, 3, "enforcer/sight1.wav", 1, ATTN_NORM)
	sound(self, 4, "enforcer/sight2.wav", 1, ATTN_NORM)
	sound(self, 5, "enforcer/sight3.wav", 1, ATTN_NORM)
	sound(self, 6, "enforcer/sight4.wav", 1, ATTN_NORM)
	sound(self, 7, "enforcer/pain1.wav", 1, ATTN_NORM)
end

function misc_noisemaker(self)
	self.nextthink = time() + 0.1 + random()
	self.think = noise_think
end
