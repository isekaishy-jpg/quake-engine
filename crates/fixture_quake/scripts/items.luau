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
-- Ported from items.qc in v1.01, for single player. Deathmatch's respawning
-- waits for online play.

-- Plants the object on the floor.
function PlaceItem(self)
	self.mdl = self.model
	self.flags = FL_ITEM
	self.solid = SOLID_TRIGGER
	self.movetype = MOVETYPE_TOSS
	self.velocity = vector.zero
	self.origin = self.origin + vector.create(0, 0, 6)
	if not droptofloor(self) then
		remove(self)
	end
end

-- Items start after other solids.
function StartItem(self)
	self.nextthink = time() + 0.2
	self.think = PlaceItem
end

-- Brings an item back, from `SUB_regen`.
function SUB_regen(self)
	self.model = self.mdl
	self.solid = SOLID_TRIGGER
	sound(self, CHAN_VOICE, "items/itembk2.wav", 1, ATTN_NORM)
	setorigin(self, self.origin)
end

-- Takes an item out of play, and fires what it targets. In deathmatch 1 it comes
-- back after `seconds`, if it does, and with `always` in every deathmatch.
local function taken(self, other, seconds, always)
	self.model = ""
	self.solid = SOLID_NOT
	if seconds and (deathmatch == 1 or (always and deathmatch ~= 0)) then
		self.nextthink = time() + seconds
		self.think = SUB_regen
	end
	activator = other
	SUB_UseTargets(self)
end

-- From `RankForWeapon`.
local function RankForWeapon(weapon)
	local ranks = {
		[IT_LIGHTNING] = 1,
		[IT_ROCKET_LAUNCHER] = 2,
		[IT_SUPER_NAILGUN] = 3,
		[IT_GRENADE_LAUNCHER] = 4,
		[IT_SUPER_SHOTGUN] = 5,
		[IT_NAILGUN] = 6,
	}
	return ranks[weapon] or 7
end

-- Adds health to an entity, up to its limit unless `ignore` is set.
function T_Heal(e, healamount, ignore)
	if e.health <= 0 then
		return false
	end
	if not ignore and e.health >= e.max_health then
		return false
	end
	e.health += math.ceil(healamount)
	if not ignore and e.health >= e.max_health then
		e.health = e.max_health
	end
	if e.health > 250 then
		e.health = 250
	end
	return true
end

local H_ROTTEN = 1
local H_MEGA = 2

function item_megahealth_rot(self)
	local other = self.owner
	if other.health > other.max_health then
		other.health -= 1
		self.nextthink = time() + 1
		return
	end
	other.items -= bit32.band(other.items, IT_SUPERHEALTH)
	if deathmatch == 1 then
		self.nextthink = time() + 20
		self.think = SUB_regen
	end
end

function health_touch(self, other)
	if other.classname ~= "player" then
		return
	end
	if self.healtype == 2 then
		if other.health >= 250 or not T_Heal(other, self.healamount, true) then
			return
		end
	elseif not T_Heal(other, self.healamount, false) then
		return
	end
	sprint(other, "You receive " .. tostring(self.healamount) .. " health")
	sound(other, CHAN_ITEM, self.noise, 1, ATTN_NORM)
	bonus_flash(other)
	self.model = ""
	self.solid = SOLID_NOT
	if self.healtype == 2 then
		other.items = bit32.bor(other.items, IT_SUPERHEALTH)
		self.nextthink = time() + 5
		self.think = item_megahealth_rot
		self.owner = other
	elseif deathmatch ~= 2 then
		-- Deathmatch 2 is the silly old rules.
		if deathmatch ~= 0 then
			self.nextthink = time() + 20
		end
		self.think = SUB_regen
	end
	activator = other
	SUB_UseTargets(self)
end

function item_health(self)
	self.touch = health_touch
	if bit32.band(self.spawnflags, H_ROTTEN) ~= 0 then
		setmodel(self, "maps/b_bh10.bsp")
		self.noise = "items/r_item1.wav"
		self.healamount = 15
		self.healtype = 0
	elseif bit32.band(self.spawnflags, H_MEGA) ~= 0 then
		setmodel(self, "maps/b_bh100.bsp")
		self.noise = "items/r_item2.wav"
		self.healamount = 100
		self.healtype = 2
	else
		setmodel(self, "maps/b_bh25.bsp")
		self.noise = "items/health1.wav"
		self.healamount = 25
		self.healtype = 1
	end
	setsize(self, vector.zero, vector.create(32, 32, 56))
	StartItem(self)
end

local ARMORS = {
	item_armor1 = { 0.3, 100, IT_ARMOR1 },
	item_armor2 = { 0.6, 150, IT_ARMOR2 },
	item_armorInv = { 0.8, 200, IT_ARMOR3 },
}

function armor_touch(self, other)
	if other.health <= 0 or other.classname ~= "player" then
		return
	end
	local armor = ARMORS[self.classname]
	if (other.armortype or 0) * (other.armorvalue or 0) >= armor[1] * armor[2] then
		return
	end
	other.armortype = armor[1]
	other.armorvalue = armor[2]
	local worn = bit32.bor(IT_ARMOR1, IT_ARMOR2, IT_ARMOR3)
	other.items = other.items - bit32.band(other.items, worn) + armor[3]
	sprint(other, "You got armor")
	sound(other, CHAN_ITEM, "items/armor1.wav", 1, ATTN_NORM)
	bonus_flash(other)
	taken(self, other, 20)
end

local function armor(skin)
	return function(self)
		self.touch = armor_touch
		setmodel(self, "progs/armor.mdl")
		self.skin = skin
		setsize(self, vector.create(-16, -16, 0), vector.create(16, 16, 56))
		StartItem(self)
	end
end

item_armor1 = armor(0)
item_armor2 = armor(1)
item_armorInv = armor(2)

function bound_other_ammo(other)
	other.ammo_shells = math.min(other.ammo_shells, 100)
	other.ammo_nails = math.min(other.ammo_nails, 200)
	other.ammo_rockets = math.min(other.ammo_rockets, 100)
	other.ammo_cells = math.min(other.ammo_cells, 100)
end

-- What each weapon on the floor gives.
local WEAPONS = {
	weapon_nailgun = { IT_NAILGUN, "ammo_nails", 30 },
	weapon_supernailgun = { IT_SUPER_NAILGUN, "ammo_nails", 30 },
	weapon_supershotgun = { IT_SUPER_SHOTGUN, "ammo_shells", 5 },
	weapon_rocketlauncher = { IT_ROCKET_LAUNCHER, "ammo_rockets", 5 },
	weapon_grenadelauncher = { IT_GRENADE_LAUNCHER, "ammo_rockets", 5 },
	weapon_lightning = { IT_LIGHTNING, "ammo_cells", 15 },
}

function weapon_touch(self, other)
	if bit32.band(other.flags, FL_CLIENT) == 0 then
		return
	end
	local weapon = WEAPONS[self.classname]
	if weapon == nil then
		objerror("weapon_touch: unknown classname")
	end
	local leave = deathmatch == 2 or coop ~= 0
	if leave and bit32.band(other.items, weapon[1]) ~= 0 then
		return
	end
	other[weapon[2]] += weapon[3]
	sprint(other, "You got the " .. self.netname)
	sound(other, CHAN_ITEM, "weapons/pkup.wav", 1, ATTN_NORM)
	bonus_flash(other)
	bound_other_ammo(other)
	other.items = bit32.bor(other.items, weapon[1])
	if deathmatch == 0 or RankForWeapon(weapon[1]) < RankForWeapon(other.weapon) then
		other.weapon = weapon[1]
	end
	W_SetCurrentAmmo(other)
	if not leave then
		taken(self, other, 30)
	end
end

local function floor_weapon(model, name)
	return function(self)
		setmodel(self, model)
		self.netname = name
		self.touch = weapon_touch
		setsize(self, vector.create(-16, -16, 0), vector.create(16, 16, 56))
		StartItem(self)
	end
end

weapon_supershotgun = floor_weapon("progs/g_shot.mdl", "Double-barrelled Shotgun")
weapon_nailgun = floor_weapon("progs/g_nail.mdl", "nailgun")
weapon_supernailgun = floor_weapon("progs/g_nail2.mdl", "Super Nailgun")
weapon_grenadelauncher = floor_weapon("progs/g_rock.mdl", "Grenade Launcher")
weapon_rocketlauncher = floor_weapon("progs/g_rock2.mdl", "Rocket Launcher")
weapon_lightning = floor_weapon("progs/g_light.mdl", "Thunderbolt")

-- The ammo each pickup holds, and how much of it a player can carry.
local AMMO = {
	{ "ammo_shells", 100 },
	{ "ammo_nails", 200 },
	{ "ammo_rockets", 100 },
	{ "ammo_cells", 200 },
}

function ammo_touch(self, other)
	if other.classname ~= "player" or other.health <= 0 then
		return
	end
	-- If the player was using their best weapon, change up to the new one.
	local best = W_BestWeapon(other)
	local ammo = AMMO[self.ammo_kind]
	if other[ammo[1]] >= ammo[2] then
		return
	end
	other[ammo[1]] += self.aflag
	bound_other_ammo(other)
	sprint(other, "You got the " .. self.netname)
	sound(other, CHAN_ITEM, "weapons/lock4.wav", 1, ATTN_NORM)
	bonus_flash(other)
	if other.weapon == best then
		other.weapon = W_BestWeapon(other)
	end
	W_SetCurrentAmmo(other)
	taken(self, other, 30)
end

local WEAPON_BIG2 = 1

local function ammo_box(kind, name, small, big, small_count, big_count)
	return function(self)
		self.touch = ammo_touch
		if bit32.band(self.spawnflags, WEAPON_BIG2) ~= 0 then
			setmodel(self, big)
			self.aflag = big_count
		else
			setmodel(self, small)
			self.aflag = small_count
		end
		self.ammo_kind = kind
		self.netname = name
		setsize(self, vector.zero, vector.create(32, 32, 56))
		StartItem(self)
	end
end

item_shells = ammo_box(1, "shells", "maps/b_shell0.bsp", "maps/b_shell1.bsp", 20, 40)
item_spikes = ammo_box(2, "nails", "maps/b_nail0.bsp", "maps/b_nail1.bsp", 25, 50)
item_rockets = ammo_box(3, "rockets", "maps/b_rock0.bsp", "maps/b_rock1.bsp", 5, 10)
item_cells = ammo_box(4, "cells", "maps/b_batt0.bsp", "maps/b_batt1.bsp", 6, 12)

local WEAPON_SHOTGUN = 1
local WEAPON_ROCKET = 2
local WEAPON_SPIKES = 4
local WEAPON_BIG = 8

-- An old form of ammo that some maps still use.
function item_weapon(self)
	self.touch = ammo_touch
	local big = bit32.band(self.spawnflags, WEAPON_BIG) ~= 0
	local function set(kind, name, small, large, small_count, big_count)
		setmodel(self, if big then large else small)
		self.aflag = if big then big_count else small_count
		self.ammo_kind = kind
		self.netname = name
	end
	if bit32.band(self.spawnflags, WEAPON_SHOTGUN) ~= 0 then
		set(1, "shells", "maps/b_shell0.bsp", "maps/b_shell1.bsp", 20, 40)
	end
	if bit32.band(self.spawnflags, WEAPON_SPIKES) ~= 0 then
		set(2, "spikes", "maps/b_nail0.bsp", "maps/b_nail1.bsp", 20, 40)
	end
	if bit32.band(self.spawnflags, WEAPON_ROCKET) ~= 0 then
		set(3, "rockets", "maps/b_rock0.bsp", "maps/b_rock1.bsp", 5, 10)
	end
	setsize(self, vector.zero, vector.create(32, 32, 56))
	StartItem(self)
end

function key_touch(self, other)
	if other.classname ~= "player" or other.health <= 0 then
		return
	end
	if bit32.band(other.items, self.items) ~= 0 then
		return
	end
	sprint(other, "You got the " .. self.netname)
	sound(other, CHAN_ITEM, self.noise, 1, ATTN_NORM)
	bonus_flash(other)
	other.items = bit32.bor(other.items, self.items)
	if coop ~= 0 then
		-- Everyone needs the key.
		activator = other
		SUB_UseTargets(self)
	else
		taken(self, other)
	end
end

local KEY_SOUNDS = { [0] = "misc/medkey.wav", "misc/runekey.wav", "misc/basekey.wav" }

local function key(item, kind, models, names)
	return function(self)
		local worldtype = world and world.worldtype or 0
		setmodel(self, models[worldtype + 1])
		self.netname = names[worldtype + 1]
		self.noise = KEY_SOUNDS[worldtype]
		self.touch = key_touch
		self.items = item
		setsize(self, vector.create(-16, -16, -24), vector.create(16, 16, 32))
		StartItem(self)
	end
end

item_key1 = key(IT_KEY1, "silver", {
	"progs/w_s_key.mdl",
	"progs/m_s_key.mdl",
	"progs/b_s_key.mdl",
}, { "silver key", "silver runekey", "silver keycard" })

item_key2 = key(IT_KEY2, "gold", {
	"progs/w_g_key.mdl",
	"progs/m_g_key.mdl",
	"progs/b_g_key.mdl",
}, { "gold key", "gold runekey", "gold keycard" })

function sigil_touch(self, other)
	if other.classname ~= "player" or other.health <= 0 then
		return
	end
	centerprint(other, "You got the rune!")
	sound(other, CHAN_ITEM, self.noise, 1, ATTN_NORM)
	bonus_flash(other)
	serverflags = bit32.bor(serverflags, bit32.band(self.spawnflags, 15))
	self.classname = ""
	taken(self, other)
end

-- End of level sigil: pick it up to end the episode.
function item_sigil(self)
	if self.spawnflags == 0 then
		objerror("no spawnflags")
	end
	self.noise = "misc/runekey.wav"
	local models = {
		[1] = "progs/end1.mdl",
		[2] = "progs/end2.mdl",
		[4] = "progs/end3.mdl",
		[8] = "progs/end4.mdl",
	}
	for bit, model in models do
		if bit32.band(self.spawnflags, bit) ~= 0 then
			setmodel(self, model)
		end
	end
	self.touch = sigil_touch
	setsize(self, vector.create(-16, -16, -24), vector.create(16, 16, 32))
	StartItem(self)
end

function powerup_touch(self, other)
	if other.classname ~= "player" or other.health <= 0 then
		return
	end
	sprint(other, "You got the " .. self.netname)
	sound(other, CHAN_VOICE, self.noise, 1, ATTN_NORM)
	bonus_flash(other)
	other.items = bit32.bor(other.items, self.items)
	local class = self.classname
	if class == "item_artifact_envirosuit" then
		other.rad_time = 1
		other.radsuit_finished = time() + 30
	elseif class == "item_artifact_invulnerability" then
		other.invincible_time = 1
		other.invincible_finished = time() + 30
	elseif class == "item_artifact_invisibility" then
		other.invisible_time = 1
		other.invisible_finished = time() + 30
	elseif class == "item_artifact_super_damage" then
		other.super_time = 1
		other.super_damage_finished = time() + 30
	end
	local long = class == "item_artifact_invulnerability" or class == "item_artifact_invisibility"
	taken(self, other, if long then 300 else 60, true)
end

local function powerup(sample, model, name, item)
	return function(self)
		self.touch = powerup_touch
		self.noise = sample
		setmodel(self, model)
		self.netname = name
		self.items = item
		setsize(self, vector.create(-16, -16, -24), vector.create(16, 16, 32))
		StartItem(self)
	end
end

item_artifact_invulnerability = powerup(
	"items/protect.wav",
	"progs/invulner.mdl",
	"Pentagram of Protection",
	IT_INVULNERABILITY
)
item_artifact_envirosuit = powerup("items/suit.wav", "progs/suit.mdl", "Biosuit", IT_SUIT)
item_artifact_invisibility =
	powerup("items/inv1.wav", "progs/invisibl.mdl", "Ring of Shadows", IT_INVISIBILITY)
item_artifact_super_damage =
	powerup("items/damage.wav", "progs/quaddama.mdl", "Quad Damage", IT_QUAD)

function BackpackTouch(self, other)
	if other.classname ~= "player" or other.health <= 0 then
		return
	end
	-- If the player was using their best weapon, change up to the new one if better.
	local best = W_BestWeapon(other)

	other.ammo_shells += self.ammo_shells
	other.ammo_nails += self.ammo_nails
	other.ammo_rockets += self.ammo_rockets
	other.ammo_cells += self.ammo_cells
	other.items = bit32.bor(other.items, self.items)
	bound_other_ammo(other)

	local got = "You get "
	if self.ammo_shells ~= 0 then
		got ..= self.ammo_shells .. " shells  "
	end
	if self.ammo_nails ~= 0 then
		got ..= self.ammo_nails .. " nails "
	end
	if self.ammo_rockets ~= 0 then
		got ..= self.ammo_rockets .. " rockets  "
	end
	if self.ammo_cells ~= 0 then
		got ..= self.ammo_cells .. " cells  "
	end
	sprint(other, got)
	sound(other, CHAN_ITEM, "weapons/lock4.wav", 1, ATTN_NORM)
	bonus_flash(other)

	if other.weapon == best then
		other.weapon = W_BestWeapon(other)
	end
	remove(self)
	W_SetCurrentAmmo(other)
end

function DropBackpack(self)
	if self.ammo_shells + self.ammo_nails + self.ammo_rockets + self.ammo_cells == 0 then
		return
	end
	local item = spawn()
	item.origin = self.origin - vector.create(0, 0, 24)
	item.items = self.weapon or 0
	item.ammo_shells = self.ammo_shells
	item.ammo_nails = self.ammo_nails
	item.ammo_rockets = self.ammo_rockets
	item.ammo_cells = self.ammo_cells
	item.velocity = vector.create(-100 + random() * 200, -100 + random() * 200, 300)
	item.flags = FL_ITEM
	item.solid = SOLID_TRIGGER
	item.movetype = MOVETYPE_TOSS
	setmodel(item, "progs/backpack.mdl")
	setsize(item, vector.create(-16, -16, 0), vector.create(16, 16, 56))
	item.touch = BackpackTouch
	item.nextthink = time() + 120
	item.think = SUB_Remove
end
