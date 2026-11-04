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
-- Ported from player.qc and the spawning, powerup and level parts of client.qc
-- in v1.01. The player's own model isn't drawn in first person, so only its
-- weapon frames are kept.

-- What carries over between levels.
parms = {}

function SetNewParms()
	parms = { bit32.bor(IT_SHOTGUN, IT_AXE), 100, 0, 25, 0, 0, 0, IT_SHOTGUN, 0 }
end

function SetChangeParms(self)
	local dropped = bit32.bor(IT_KEY1, IT_KEY2, IT_INVISIBILITY, IT_INVULNERABILITY, IT_SUIT, IT_QUAD)
	local items = self.items - bit32.band(self.items, dropped)
	local health = math.clamp(self.health, 50, 100)
	parms = {
		items,
		health,
		self.armorvalue or 0,
		math.max(self.ammo_shells, 25),
		self.ammo_nails,
		self.ammo_rockets,
		self.ammo_cells,
		self.weapon,
		(self.armortype or 0) * 100,
	}
end

function DecodeLevelParms(self)
	if serverflags ~= 0 and world and world.model == "maps/start.bsp" then
		-- Take everything away on starting a new episode.
		SetNewParms()
	end
	self.items = parms[1]
	self.health = parms[2]
	self.armorvalue = parms[3]
	self.ammo_shells = parms[4]
	self.ammo_nails = parms[5]
	self.ammo_rockets = parms[6]
	self.ammo_cells = parms[7]
	self.weapon = parms[8]
	self.armortype = parms[9] * 0.01
end

-- Gets the player ready. The engine has already put them at the spawn point.
function PutClientInServer(self)
	self.classname = "player"
	self.takedamage = DAMAGE_AIM
	self.view_ofs = vector.create(0, 0, 22)
	self.flags = bit32.bor(self.flags, FL_CLIENT)
	self.solid = SOLID_SLIDEBOX
	self.movetype = MOVETYPE_WALK
	self.show_hostile = 0
	self.max_health = 100
	self.flags = FL_CLIENT
	self.air_finished = time() + 12
	self.dmg = 2
	self.super_damage_finished = 0
	self.radsuit_finished = 0
	self.invisible_finished = 0
	self.invincible_finished = 0
	self.effects = 0
	self.invincible_time = 0
	DecodeLevelParms(self)
	W_SetCurrentAmmo(self)
	self.attack_finished = time()
	self.th_pain = player_pain
	self.th_die = PlayerDie
	self.deadflag = DEAD_NO
	self.netname = "player"
	setsize(self, vector.create(-16, -16, -24), vector.create(16, 16, 32))
	-- Returning with a rune lands somewhere else, from `SelectSpawnPoint`.
	if serverflags ~= 0 then
		local spot = find_entity(world, "classname", "info_player_start2")
		if exists(spot) then
			setorigin(self, spot.origin + vector.create(0, 0, 1))
			engine.set_view(spot.angles.y, 0)
		end
	end
end

-- Stands the weapon down.
function player_run(self)
	self.weaponframe = 0
	self.think = nil
	self.nextthink = 0
end

-- Runs `frames` of the weapon, a tenth of a second each, then stands it down.
-- `fire` is the step that strikes, if any.
local function weapon_frames(self, frames, step, fire)
	self.weaponframe = frames[step]
	if step == fire then
		W_FireAxe(self)
	end
	self.nextthink = time() + 0.1
	self.weapon_steps = frames
	self.weapon_step = step
	self.weapon_strike = fire
	self.think = weapon_frames_next
end

function weapon_frames_next(self)
	if self.weapon_step == #self.weapon_steps then
		player_run(self)
	else
		weapon_frames(self, self.weapon_steps, self.weapon_step + 1, self.weapon_strike)
	end
end

local function muzzle_flash(self)
	self.effects = bit32.bor(self.effects, EF_MUZZLEFLASH)
end

function player_shot(self)
	muzzle_flash(self)
	weapon_frames(self, { 1, 2, 3, 4, 5, 6 }, 1, nil)
end

function player_rocket(self)
	muzzle_flash(self)
	weapon_frames(self, { 1, 2, 3, 4, 5, 6 }, 1, nil)
end

-- The axe has two swings, starting on frame 1 or 5.
function player_axe(self, first)
	weapon_frames(self, { first, first + 1, first + 2, first + 3 }, 1, 3)
end

-- Fires for as long as the button is down, a tenth of a second a shot.
local function held_fire(self, wrap, fire, next_think)
	muzzle_flash(self)
	self.nextthink = time() + 0.1
	self.think = next_think
	if not self.button0 then
		player_run(self)
		return
	end
	self.weaponframe += 1
	if self.weaponframe == wrap then
		self.weaponframe = 1
	end
	SuperDamageSound(self)
	fire(self)
	self.attack_finished = time() + 0.2
end

function player_nail1(self)
	held_fire(self, 9, function(me)
		W_FireSpikes(me, 4)
	end, player_nail2)
end

function player_nail2(self)
	held_fire(self, 9, function(me)
		W_FireSpikes(me, -4)
	end, player_nail1)
end

function player_light1(self)
	held_fire(self, 5, W_FireLightning, player_light1)
end

local function PainSound(self)
	if self.health < 0 then
		return
	end
	-- Drowning.
	if self.watertype == CONTENT_WATER and self.waterlevel == 3 then
		local sample = if random() > 0.5 then "player/drown1.wav" else "player/drown2.wav"
		sound(self, CHAN_VOICE, sample, 1, ATTN_NORM)
		return
	end
	-- Slime and lava.
	if self.watertype == CONTENT_SLIME or self.watertype == CONTENT_LAVA then
		local sample = if random() > 0.5 then "player/lburn1.wav" else "player/lburn2.wav"
		sound(self, CHAN_VOICE, sample, 1, ATTN_NORM)
		return
	end
	if (self.pain_finished or 0) > time() then
		self.axhitme = 0
		return
	end
	self.pain_finished = time() + 0.5
	if (self.axhitme or 0) == 1 then
		self.axhitme = 0
		sound(self, CHAN_VOICE, "player/axhit1.wav", 1, ATTN_NORM)
		return
	end
	local number = math.floor(random() * 6) + 1
	sound(self, CHAN_VOICE, "player/pain" .. tostring(number) .. ".wav", 1, ATTN_NORM)
end

function player_pain(self, _attacker, _damage)
	PainSound(self)
end

local function DeathSound(self)
	if self.waterlevel == 3 then
		sound(self, CHAN_VOICE, "player/h2odeath.wav", 1, ATTN_NONE)
		return
	end
	local number = math.floor(random() * 5) + 1
	sound(self, CHAN_VOICE, "player/death" .. tostring(number) .. ".wav", 1, ATTN_NONE)
end

function VelocityForDamage(dm)
	local v = vector.create(100 * crandom(), 100 * crandom(), 200 + 100 * random())
	if dm > -50 then
		return v * 0.7
	elseif dm > -200 then
		return v * 2
	end
	return v * 10
end

function ThrowGib(self, gibname, dm)
	local new = spawn()
	new.origin = self.origin
	setmodel(new, gibname)
	setsize(new, vector.zero, vector.zero)
	new.velocity = VelocityForDamage(dm)
	new.movetype = MOVETYPE_BOUNCE
	new.solid = SOLID_NOT
	new.avelocity = vector.create(random() * 600, random() * 600, random() * 600)
	new.think = SUB_Remove
	new.ltime = time()
	new.nextthink = time() + 10 + random() * 10
	new.frame = 0
	new.flags = 0
end

function ThrowHead(self, gibname, dm)
	setmodel(self, gibname)
	self.frame = 0
	self.nextthink = -1
	self.movetype = MOVETYPE_BOUNCE
	self.takedamage = DAMAGE_NO
	self.solid = SOLID_NOT
	self.view_ofs = vector.create(0, 0, 8)
	setsize(self, vector.create(-16, -16, 0), vector.create(16, 16, 56))
	self.velocity = VelocityForDamage(dm)
	self.origin = with_z(self.origin, self.origin.z - 24)
	self.flags -= bit32.band(self.flags, FL_ONGROUND)
	self.avelocity = crandom() * vector.create(0, 600, 0)
end

function PlayerDie(self)
	self.items -= bit32.band(self.items, IT_INVISIBILITY)
	self.invisible_finished = 0
	self.invincible_finished = 0
	self.super_damage_finished = 0
	self.radsuit_finished = 0
	self.weaponmodel = ""
	self.deadflag = DEAD_DYING
	self.solid = SOLID_NOT
	self.dead = true
	if self.health < -40 then
		ThrowGib(self, "progs/gib1.mdl", self.health)
		ThrowGib(self, "progs/gib2.mdl", self.health)
		ThrowGib(self, "progs/gib3.mdl", self.health)
		local sample = if random() < 0.5 then "player/gib.wav" else "player/udeath.wav"
		sound(self, CHAN_VOICE, sample, 1, ATTN_NONE)
	else
		DeathSound(self)
	end
	self.nextthink = time() + 1
	self.think = PlayerDead
end

function PlayerDead(self)
	self.deadflag = DEAD_DEAD
end

function PlayerDeathThink(self)
	-- Wait for all buttons released.
	if self.deadflag == DEAD_DEAD then
		if self.button2 or self.button0 then
			return
		end
		self.deadflag = DEAD_RESPAWNABLE
		return
	end
	-- Wait for any button down.
	if self.deadflag == DEAD_RESPAWNABLE and (self.button2 or self.button0) then
		respawn(self)
	end
end

-- One powerup's warnings and end. `finished`, `timer` and `item` name its fields.
local function check_powerup(self, finished, timer, item, warning, sample)
	local ends = self[finished] or 0
	if ends == 0 then
		return
	end
	-- Sound and screen flash when it starts to run out.
	if ends < time() + 3 then
		if self[timer] == 1 then
			sprint(self, warning)
			bonus_flash(self)
			sound(self, CHAN_AUTO, sample, 1, ATTN_NORM)
			self[timer] = time() + 1
		end
		if self[timer] < time() then
			self[timer] = time() + 1
			bonus_flash(self)
		end
	end
	if ends < time() then
		self.items -= bit32.band(self.items, item)
		self[finished] = 0
		self[timer] = 0
	end
end

function CheckPowerups(self)
	if self.health <= 0 then
		return
	end
	-- From `R_DrawViewModel`: the invisible can't see their own gun.
	self.weaponhidden = bit32.band(self.items, IT_INVISIBILITY) ~= 0
	if (self.invisible_finished or 0) ~= 0 and (self.invisible_sound or 0) < time() then
		sound(self, CHAN_AUTO, "items/inv3.wav", 0.5, ATTN_IDLE)
		self.invisible_sound = time() + random() * 3 + 1
	end
	check_powerup(
		self,
		"invisible_finished",
		"invisible_time",
		IT_INVISIBILITY,
		"Ring of Shadows magic is fading",
		"items/inv2.wav"
	)
	check_powerup(
		self,
		"invincible_finished",
		"invincible_time",
		IT_INVULNERABILITY,
		"Protection is almost burned out",
		"items/protect2.wav"
	)
	check_powerup(
		self,
		"super_damage_finished",
		"super_time",
		IT_QUAD,
		"Quad Damage is wearing off",
		"items/damage2.wav"
	)
	if (self.radsuit_finished or 0) ~= 0 then
		self.air_finished = time() + 12
	end
	check_powerup(
		self,
		"radsuit_finished",
		"rad_time",
		IT_SUIT,
		"Air supply in Biosuit expiring",
		"items/suit2.wav"
	)
	local glowing = (self.invincible_finished or 0) > time()
		or (self.super_damage_finished or 0) > time()
	if glowing then
		self.effects = bit32.bor(self.effects, EF_DIMLIGHT)
	else
		self.effects -= bit32.band(self.effects, EF_DIMLIGHT)
	end
end
