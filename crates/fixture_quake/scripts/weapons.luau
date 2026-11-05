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
-- Ported from weapons.qc in v1.01.

function crandom()
	return 2 * (random() - 0.5)
end

function W_FireAxe(self)
	local source = self.origin + vector.create(0, 0, 16)
	traceline(source, source + v_forward * 64, false, self)
	if trace_fraction == 1 then
		return
	end
	local org = trace_endpos - v_forward * 4
	if (trace_ent.takedamage or 0) ~= 0 then
		trace_ent.axhitme = 1
		SpawnBlood(org, vector.zero, 20)
		T_Damage(trace_ent, self, self, 20)
	else
		sound(self, CHAN_WEAPON, "player/axhit2.wav", 1, ATTN_NORM)
		temp_entity(TE_GUNSHOT, org)
	end
end

function wall_velocity(self)
	local vel = normalize(self.velocity)
	vel = normalize(vel + v_up * (random() - 0.5) + v_right * (random() - 0.5))
	return (vel + trace_plane_normal * 2) * 200
end

function SpawnBlood(org, vel, damage)
	particle(org, vel * 0.1, 73, damage * 2)
end

function spawn_touchblood(self, damage)
	local vel = wall_velocity(self) * 0.2
	SpawnBlood(self.origin + vel * 0.01, vel, damage)
end

-- Collects multiple small damages into a single damage.
local multi_ent = nil
local multi_damage = 0

-- How close a target has to be to the line of fire to draw it, from `sv_aim`.
local AIM_LIMIT = 0.93

-- The direction to fire in, bent up or down to something near the line of
-- fire, from `PF_aim`.
function aim(self, _speed)
	-- Online, what's fired goes where it's pointed, so a client can draw it
	-- at once, as `sv_aim 2` does in QuakeWorld.
	if deathmatch ~= 0 or coop ~= 0 then
		return v_forward
	end
	local start = self.origin + vector.create(0, 0, 20)
	traceline(start, start + v_forward * 2048, false, self)
	if trace_ent.takedamage == DAMAGE_AIM then
		return v_forward
	end
	local bestdist = AIM_LIMIT
	local best = nil
	for _, check in find_all("takedamage", DAMAGE_AIM) do
		if check ~= self then
			local target = check.origin + (check.mins + check.maxs) * 0.5
			local dist = vector.dot(normalize(target - start), v_forward)
			if dist >= bestdist then
				traceline(start, target, false, self)
				if trace_ent == check then
					bestdist = dist
					best = check
				end
			end
		end
	end
	if best == nil then
		return v_forward
	end
	local dir = best.origin - self.origin
	local flat = v_forward * vector.dot(dir, v_forward)
	return normalize(with_z(flat, dir.z))
end

function SpawnMeatSpray(self, org, vel)
	local missile = spawn()
	missile.owner = self
	missile.movetype = MOVETYPE_BOUNCE
	missile.solid = SOLID_NOT
	makevectors(self.angles)
	missile.velocity = with_z(vel, vel.z + 250 + 50 * random())
	missile.avelocity = vector.create(3000, 1000, 2000)
	missile.nextthink = time() + 1
	missile.think = SUB_Remove
	setmodel(missile, "progs/zom_gib.mdl")
	setsize(missile, vector.zero, vector.zero)
	setorigin(missile, org)
end

function ClearMultiDamage()
	multi_ent = nil
	multi_damage = 0
end

function ApplyMultiDamage(self)
	if multi_ent then
		T_Damage(multi_ent, self, self, multi_damage)
	end
end

function AddMultiDamage(self, hit, damage)
	if hit == nil then
		return
	end
	if hit ~= multi_ent then
		ApplyMultiDamage(self)
		multi_damage = damage
		multi_ent = hit
	else
		multi_damage += damage
	end
end

function TraceAttack(self, damage, dir)
	local vel = normalize(dir + v_up * crandom() + v_right * crandom())
	vel = (vel + trace_plane_normal * 2) * 200
	local org = trace_endpos - dir * 4
	if (trace_ent.takedamage or 0) ~= 0 then
		SpawnBlood(org, vel * 0.2, damage)
		AddMultiDamage(self, trace_ent, damage)
	else
		temp_entity(TE_GUNSHOT, org)
	end
end

-- Used by the shotguns. Goes to the trouble of combining pellets into one
-- damage call.
function FireBullets(self, shotcount, dir, spread)
	makevectors(self.v_angle)
	local src = self.origin + v_forward * 10
	src = vector.create(src.x, src.y, self.absmin.z + self.size.z * 0.7)
	ClearMultiDamage()
	for _ = 1, shotcount do
		local direction = dir + v_right * (crandom() * spread.x) + v_up * (crandom() * spread.y)
		traceline(src, src + direction * 2048, false, self)
		if trace_fraction ~= 1 then
			TraceAttack(self, 4, direction)
		end
	end
	ApplyMultiDamage(self)
end

function W_FireShotgun(self)
	sound(self, CHAN_WEAPON, "weapons/guncock.wav", 1, ATTN_NORM)
	self.punchangle = vector.create(-2, 0, 0)
	self.ammo_shells -= 1
	self.currentammo = self.ammo_shells
	FireBullets(self, 6, aim(self, 100000), vector.create(0.04, 0.04, 0))
end

function W_FireSuperShotgun(self)
	if self.currentammo == 1 then
		W_FireShotgun(self)
		return
	end
	sound(self, CHAN_WEAPON, "weapons/shotgn2.wav", 1, ATTN_NORM)
	self.punchangle = vector.create(-4, 0, 0)
	self.ammo_shells -= 2
	self.currentammo = self.ammo_shells
	FireBullets(self, 14, aim(self, 100000), vector.create(0.14, 0.08, 0))
end

-- The six frames of an explosion's sprite.
local function s_explode(self, frame)
	self.frame = frame
	self.nextthink = time() + 0.1
	if frame == 5 then
		self.think = SUB_Remove
	else
		self.think = s_explode_next
	end
end

function s_explode_next(self)
	s_explode(self, self.frame + 1)
end

function s_explode1(self)
	s_explode(self, 0)
end

function BecomeExplosion(self)
	self.movetype = MOVETYPE_NONE
	self.velocity = vector.zero
	self.touch = SUB_Null
	setmodel(self, "progs/s_explod.spr")
	self.solid = SOLID_NOT
	s_explode(self, 0)
end

function T_MissileTouch(self, other)
	if other == self.owner then
		return
	end
	if pointcontents(self.origin) == CONTENT_SKY then
		remove(self)
		return
	end
	local damg = 100 + random() * 20
	if other and (other.health or 0) ~= 0 then
		if other.classname == "monster_shambler" then
			damg = damg * 0.5
		end
		T_Damage(other, self, self.owner, damg)
	end
	-- Don't do radius damage to the other, because all the damage was done in
	-- the impact.
	T_RadiusDamage(self, self.owner, 120, other)
	self.origin = self.origin - normalize(self.velocity) * 8
	temp_entity(TE_EXPLOSION, self.origin)
	BecomeExplosion(self)
end

-- Tells clients a path, so they can work it out for themselves.
function announce_launch(missile, owner, bounces, blasts)
	local id = rawget(missile, "_id")
	local velocity = missile.velocity
	engine.launch(id, rawget(owner, "_id"), missile.origin, velocity, time(), bounces, blasts)
end

-- The spread of a grenade from a shot's seed, the same as the client works it
-- out, or at random with none.
local function spread(self, salt)
	local seed = self.fire_seed
	if seed == nil then
		return crandom()
	end
	local hashed = ((seed + salt * 40503) * 2654435761) % 4294967296
	return hashed / 2147483648 - 1
end

function W_FireRocket(self)
	self.ammo_rockets -= 1
	self.currentammo = self.ammo_rockets
	sound(self, CHAN_WEAPON, "weapons/sgun1.wav", 1, ATTN_NORM)
	self.punchangle = vector.create(-2, 0, 0)
	local missile = spawn()
	missile.owner = self
	missile.movetype = MOVETYPE_FLYMISSILE
	missile.solid = SOLID_BBOX
	makevectors(self.v_angle)
	missile.velocity = aim(self, 1000) * 1000
	missile.angles = vectoangles(missile.velocity)
	missile.touch = T_MissileTouch
	missile.nextthink = time() + 5
	missile.think = SUB_Remove
	setmodel(missile, "progs/missile.mdl")
	setsize(missile, vector.zero, vector.zero)
	setorigin(missile, self.origin + v_forward * 8 + vector.create(0, 0, 16))
	announce_launch(missile, self, false, true)
end

function LightningDamage(self, p1, p2, from, damage)
	-- The original's side vector, which is lopsided, as it normalizes nothing.
	local f = p2 - p1
	f = vector.create(-f.y, -f.y, 0) * 16

	traceline(p1, p2, false, self)
	if (trace_ent.takedamage or 0) ~= 0 then
		particle(trace_endpos, vector.create(0, 0, 100), 225, damage * 4)
		T_Damage(trace_ent, from, from, damage)
	end
	local e1 = trace_ent
	traceline(p1 + f, p2 + f, false, self)
	if trace_ent ~= e1 and (trace_ent.takedamage or 0) ~= 0 then
		particle(trace_endpos, vector.create(0, 0, 100), 225, damage * 4)
		T_Damage(trace_ent, from, from, damage)
	end
	local e2 = trace_ent
	traceline(p1 - f, p2 - f, false, self)
	if trace_ent ~= e1 and trace_ent ~= e2 and (trace_ent.takedamage or 0) ~= 0 then
		particle(trace_endpos, vector.create(0, 0, 100), 225, damage * 4)
		T_Damage(trace_ent, from, from, damage)
	end
end

function W_FireLightning(self)
	if self.ammo_cells < 1 then
		self.weapon = W_BestWeapon(self)
		W_SetCurrentAmmo(self)
		return
	end
	-- Explode if under water.
	if self.waterlevel > 1 then
		T_RadiusDamage(self, self, 35 * self.ammo_cells, world)
		self.ammo_cells = 0
		W_SetCurrentAmmo(self)
		return
	end
	if (self.t_width or 0) < time() then
		sound(self, CHAN_WEAPON, "weapons/lhit.wav", 1, ATTN_NORM)
		self.t_width = time() + 0.6
	end
	self.punchangle = vector.create(-2, 0, 0)
	self.ammo_cells -= 1
	self.currentammo = self.ammo_cells
	local org = self.origin + vector.create(0, 0, 16)
	traceline(org, org + v_forward * 600, true, self)
	temp_entity(TE_LIGHTNING2, org, trace_endpos, self)
	LightningDamage(self, self.origin, trace_endpos + v_forward * 4, self, 30)
end

function GrenadeExplode(self)
	T_RadiusDamage(self, self.owner, 120, world)
	temp_entity(TE_EXPLOSION, self.origin)
	BecomeExplosion(self)
end

function GrenadeTouch(self, other)
	if other == self.owner then
		return
	end
	if other and other.takedamage == DAMAGE_AIM then
		GrenadeExplode(self)
		return
	end
	sound(self, CHAN_WEAPON, "weapons/bounce.wav", 1, ATTN_NORM)
	if self.velocity == vector.zero then
		self.avelocity = vector.zero
	end
end

function W_FireGrenade(self)
	self.ammo_rockets -= 1
	self.currentammo = self.ammo_rockets
	sound(self, CHAN_WEAPON, "weapons/grenade.wav", 1, ATTN_NORM)
	self.punchangle = vector.create(-2, 0, 0)
	local missile = spawn()
	missile.owner = self
	missile.movetype = MOVETYPE_BOUNCE
	missile.solid = SOLID_BBOX
	missile.classname = "grenade"
	makevectors(self.v_angle)
	if self.v_angle.x ~= 0 then
		missile.velocity = v_forward * 600
			+ v_up * 200
			+ v_right * (spread(self, 0) * 10)
			+ v_up * (spread(self, 1) * 10)
	else
		local velocity = aim(self, 10000) * 600
		missile.velocity = vector.create(velocity.x, velocity.y, 200)
	end
	missile.avelocity = vector.create(300, 300, 300)
	missile.angles = vectoangles(missile.velocity)
	missile.touch = GrenadeTouch
	missile.nextthink = time() + 2.5
	missile.think = GrenadeExplode
	setmodel(missile, "progs/grenade.mdl")
	setsize(missile, vector.zero, vector.zero)
	setorigin(missile, self.origin)
	announce_launch(missile, self, true, false)
end

-- Used for both the player and the ogre.
function launch_spike(self, org, dir)
	local spike = spawn()
	spike.owner = self
	spike.movetype = MOVETYPE_FLYMISSILE
	spike.solid = SOLID_BBOX
	spike.angles = vectoangles(dir)
	spike.touch = spike_touch
	spike.classname = "spike"
	spike.think = SUB_Remove
	spike.nextthink = time() + 6
	setmodel(spike, "progs/spike.mdl")
	setsize(spike, vector.zero, vector.zero)
	setorigin(spike, org)
	spike.velocity = dir * 1000
	newmis = spike
	announce_launch(spike, self, false, false)
	return spike
end

-- From enforcer.qc, because traps fire them too.
function Laser_Touch(self, other)
	if other == self.owner then
		return
	end
	if pointcontents(self.origin) == CONTENT_SKY then
		remove(self)
		return
	end
	sound(self, CHAN_WEAPON, "enforcer/enfstop.wav", 1, ATTN_STATIC)
	local org = self.origin - normalize(self.velocity) * 8
	if (other.health or 0) ~= 0 then
		SpawnBlood(org, self.velocity * 0.2, 15)
		T_Damage(other, self, self.owner, 15)
	else
		temp_entity(TE_GUNSHOT, org)
	end
	remove(self)
end

function LaunchLaser(self, org, vec)
	if self.classname == "monster_enforcer" then
		sound(self, CHAN_WEAPON, "enforcer/enfire.wav", 1, ATTN_NORM)
	end
	local laser = spawn()
	laser.owner = self
	laser.movetype = MOVETYPE_FLY
	laser.solid = SOLID_BBOX
	laser.effects = EF_DIMLIGHT
	setmodel(laser, "progs/laser.mdl")
	setsize(laser, vector.zero, vector.zero)
	setorigin(laser, org)
	laser.velocity = normalize(vec) * 600
	laser.angles = vectoangles(laser.velocity)
	laser.nextthink = time() + 5
	laser.think = SUB_Remove
	laser.touch = Laser_Touch
	newmis = laser
	return laser
end

function W_FireSuperSpikes(self)
	sound(self, CHAN_WEAPON, "weapons/spike2.wav", 1, ATTN_NORM)
	self.punchangle = vector.create(-2, 0, 0)
	self.attack_finished = time() + 0.2
	self.ammo_nails -= 2
	self.currentammo = self.ammo_nails
	local spike = launch_spike(self, self.origin + vector.create(0, 0, 16), aim(self, 1000))
	spike.touch = superspike_touch
	setmodel(spike, "progs/s_spike.mdl")
	setsize(spike, vector.zero, vector.zero)
end

function W_FireSpikes(self, ox)
	makevectors(self.v_angle)
	if self.ammo_nails >= 2 and self.weapon == IT_SUPER_NAILGUN then
		W_FireSuperSpikes(self)
		return
	end
	if self.ammo_nails < 1 then
		self.weapon = W_BestWeapon(self)
		W_SetCurrentAmmo(self)
		return
	end
	sound(self, CHAN_WEAPON, "weapons/rocket1i.wav", 1, ATTN_NORM)
	self.punchangle = vector.create(-2, 0, 0)
	self.attack_finished = time() + 0.2
	self.ammo_nails -= 1
	self.currentammo = self.ammo_nails
	local dir = aim(self, 1000)
	launch_spike(self, self.origin + vector.create(0, 0, 16) + v_right * ox, dir)
end

local function touch_spike(self, other, damage, effect)
	if other == self.owner then
		return
	end
	if other and other.solid == SOLID_TRIGGER then
		return
	end
	if pointcontents(self.origin) == CONTENT_SKY then
		remove(self)
		return
	end
	-- Hit something that bleeds.
	if other and (other.takedamage or 0) ~= 0 then
		spawn_touchblood(self, damage)
		T_Damage(other, self, self.owner, damage)
	else
		temp_entity(effect, self.origin)
	end
	remove(self)
end

function spike_touch(self, other)
	local effect = TE_SPIKE
	if self.classname == "wizspike" then
		effect = TE_WIZSPIKE
	elseif self.classname == "knightspike" then
		effect = TE_KNIGHTSPIKE
	end
	touch_spike(self, other, 9, effect)
end

function superspike_touch(self, other)
	touch_spike(self, other, 18, TE_SUPERSPIKE)
end

-- What each weapon shows and fires from.
local WEAPON_MODELS = {
	[IT_AXE] = { "progs/v_axe.mdl" },
	[IT_SHOTGUN] = { "progs/v_shot.mdl", "ammo_shells", IT_SHELLS },
	[IT_SUPER_SHOTGUN] = { "progs/v_shot2.mdl", "ammo_shells", IT_SHELLS },
	[IT_NAILGUN] = { "progs/v_nail.mdl", "ammo_nails", IT_NAILS },
	[IT_SUPER_NAILGUN] = { "progs/v_nail2.mdl", "ammo_nails", IT_NAILS },
	[IT_GRENADE_LAUNCHER] = { "progs/v_rock.mdl", "ammo_rockets", IT_ROCKETS },
	[IT_ROCKET_LAUNCHER] = { "progs/v_rock2.mdl", "ammo_rockets", IT_ROCKETS },
	[IT_LIGHTNING] = { "progs/v_light.mdl", "ammo_cells", IT_CELLS },
}

function W_SetCurrentAmmo(self)
	-- Get out of any weapon firing states.
	player_run(self)
	local ammo = bit32.bor(IT_SHELLS, IT_NAILS, IT_ROCKETS, IT_CELLS)
	self.items -= bit32.band(self.items, ammo)
	self.weaponframe = 0
	local weapon = WEAPON_MODELS[self.weapon]
	if weapon == nil then
		self.currentammo = 0
		self.weaponmodel = ""
		return
	end
	self.weaponmodel = weapon[1]
	self.currentammo = if weapon[2] then self[weapon[2]] else 0
	if weapon[3] then
		self.items = bit32.bor(self.items, weapon[3])
	end
end

function W_BestWeapon(self)
	local it = self.items
	local has = function(item)
		return bit32.band(it, item) ~= 0
	end
	if self.ammo_cells >= 1 and has(IT_LIGHTNING) then
		return IT_LIGHTNING
	elseif self.ammo_nails >= 2 and has(IT_SUPER_NAILGUN) then
		return IT_SUPER_NAILGUN
	elseif self.ammo_shells >= 2 and has(IT_SUPER_SHOTGUN) then
		return IT_SUPER_SHOTGUN
	elseif self.ammo_nails >= 1 and has(IT_NAILGUN) then
		return IT_NAILGUN
	elseif self.ammo_shells >= 1 and has(IT_SHOTGUN) then
		return IT_SHOTGUN
	end
	return IT_AXE
end

function W_CheckNoAmmo(self)
	if self.currentammo > 0 or self.weapon == IT_AXE then
		return true
	end
	self.weapon = W_BestWeapon(self)
	W_SetCurrentAmmo(self)
	return false
end

-- A tick, in seconds.
local TICK_SECONDS = 1 / 72

-- Sets when a weapon can fire next. With the button held, it counts from when
-- it was due, not the tick that noticed, so rates don't round up to whole
-- ticks (proposal 0050).
function refire(self, seconds)
	local now = time()
	local due = self.attack_finished or 0
	local held = due <= now and now - due < TICK_SECONDS
	self.attack_finished = (if held then due else now) + seconds
end

-- An attack can start now.
function W_Attack(self)
	if not W_CheckNoAmmo(self) then
		return
	end
	makevectors(self.v_angle)
	self.show_hostile = time() + 1

	local weapon = self.weapon
	if weapon == IT_AXE then
		sound(self, CHAN_WEAPON, "weapons/ax1.wav", 1, ATTN_NORM)
		local r = random()
		if r < 0.25 then
			player_axe(self, 1, 119)
		elseif r < 0.5 then
			player_axe(self, 5, 125)
		elseif r < 0.75 then
			player_axe(self, 1, 131)
		else
			player_axe(self, 5, 137)
		end
		refire(self, 0.5)
	elseif weapon == IT_SHOTGUN then
		player_shot(self)
		W_FireShotgun(self)
		refire(self, 0.5)
	elseif weapon == IT_SUPER_SHOTGUN then
		player_shot(self)
		W_FireSuperShotgun(self)
		refire(self, 0.7)
	elseif weapon == IT_NAILGUN or weapon == IT_SUPER_NAILGUN then
		player_nail1(self)
	elseif weapon == IT_GRENADE_LAUNCHER then
		player_rocket(self)
		W_FireGrenade(self)
		refire(self, 0.6)
	elseif weapon == IT_ROCKET_LAUNCHER then
		player_rocket(self)
		W_FireRocket(self)
		refire(self, 0.8)
	elseif weapon == IT_LIGHTNING then
		player_light1(self)
		refire(self, 0.1)
		sound(self, CHAN_AUTO, "weapons/lstart.wav", 1, ATTN_NORM)
	end
end

-- The weapon for each impulse, and the ammo it needs.
local IMPULSE_WEAPONS = {
	{ IT_AXE },
	{ IT_SHOTGUN, "ammo_shells", 1 },
	{ IT_SUPER_SHOTGUN, "ammo_shells", 2 },
	{ IT_NAILGUN, "ammo_nails", 1 },
	{ IT_SUPER_NAILGUN, "ammo_nails", 2 },
	{ IT_GRENADE_LAUNCHER, "ammo_rockets", 1 },
	{ IT_ROCKET_LAUNCHER, "ammo_rockets", 1 },
	{ IT_LIGHTNING, "ammo_cells", 1 },
}

function W_ChangeWeapon(self)
	local choice = IMPULSE_WEAPONS[self.impulse]
	self.impulse = 0
	if bit32.band(self.items, choice[1]) == 0 then
		sprint(self, "no weapon.")
		return
	end
	if choice[2] and self[choice[2]] < choice[3] then
		sprint(self, "not enough ammo.")
		return
	end
	self.weapon = choice[1]
	W_SetCurrentAmmo(self)
end

function CheatCommand(self)
	if deathmatch ~= 0 or coop ~= 0 then
		return
	end
	self.ammo_rockets = 100
	self.ammo_nails = 200
	self.ammo_shells = 100
	self.ammo_cells = 200
	self.items = bit32.bor(
		self.items,
		IT_AXE,
		IT_SHOTGUN,
		IT_SUPER_SHOTGUN,
		IT_NAILGUN,
		IT_SUPER_NAILGUN,
		IT_GRENADE_LAUNCHER,
		IT_ROCKET_LAUNCHER,
		IT_LIGHTNING,
		IT_KEY1,
		IT_KEY2
	)
	self.weapon = IT_ROCKET_LAUNCHER
	self.impulse = 0
	W_SetCurrentAmmo(self)
end

-- Goes to the next weapon with ammo.
function CycleWeaponCommand(self)
	self.impulse = 0
	local position = 1
	for index, choice in IMPULSE_WEAPONS do
		if choice[1] == self.weapon then
			position = index
		end
	end
	for step = 1, #IMPULSE_WEAPONS do
		local choice = IMPULSE_WEAPONS[(position + step - 1) % #IMPULSE_WEAPONS + 1]
		local enough = choice[2] == nil or self[choice[2]] >= choice[3]
		if bit32.band(self.items, choice[1]) ~= 0 and enough then
			self.weapon = choice[1]
			W_SetCurrentAmmo(self)
			return
		end
	end
end

function QuadCheat(self)
	if deathmatch ~= 0 or coop ~= 0 then
		return
	end
	self.super_time = 1
	self.super_damage_finished = time() + 30
	self.items = bit32.bor(self.items, IT_QUAD)
end

function ImpulseCommands(self)
	local impulse = self.impulse
	if impulse >= 1 and impulse <= 8 then
		W_ChangeWeapon(self)
	elseif impulse == 9 then
		CheatCommand(self)
	elseif impulse == 10 then
		CycleWeaponCommand(self)
	elseif impulse == 11 and deathmatch == 0 and coop == 0 then
		serverflags = serverflags * 2 + 1
	elseif impulse == 255 then
		QuadCheat(self)
	end
	self.impulse = 0
end

-- Called every frame, so impulses are handled as soon as they can be.
function W_WeaponFrame(self)
	if time() < (self.attack_finished or 0) then
		return
	end
	ImpulseCommands(self)
	if self.button0 then
		SuperDamageSound(self)
		W_Attack(self)
	end
end

function SuperDamageSound(self)
	if (self.super_damage_finished or 0) > time() and (self.super_sound or 0) < time() then
		self.super_sound = time() + 1
		sound(self, CHAN_BODY, "items/damage3.wav", 1, ATTN_NORM)
	end
end

-- Instant weapons hit what the player saw, not where it's since moved to.
local function aimed_at_what_was_seen(fire)
	return function(self, ...)
		engine.rewind(rawget(self, "_id"))
		fire(self, ...)
		engine.end_rewind()
	end
end

W_FireAxe = aimed_at_what_was_seen(W_FireAxe)
W_FireShotgun = aimed_at_what_was_seen(W_FireShotgun)
W_FireSuperShotgun = aimed_at_what_was_seen(W_FireSuperShotgun)
W_FireLightning = aimed_at_what_was_seen(W_FireLightning)
