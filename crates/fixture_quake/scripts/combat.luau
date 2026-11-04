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
-- Ported from combat.qc in v1.01.

-- Used by buttons and triggers to set the activator for target firing.
damage_attacker = nil

-- Whether the inflictor can directly damage the target. Used for explosions and
-- melee attacks.
function CanDamage(targ, inflictor)
	-- Brush models need special checking because their origin is 0 0 0.
	if targ.movetype == MOVETYPE_PUSH then
		traceline(inflictor.origin, (targ.absmin + targ.absmax) * 0.5, true, inflictor)
		return trace_fraction == 1 or trace_ent == targ
	end
	for _, offset in {
		vector.zero,
		vector.create(15, 15, 0),
		vector.create(-15, -15, 0),
		vector.create(-15, 15, 0),
		vector.create(15, -15, 0),
	} do
		traceline(inflictor.origin, targ.origin + offset, true, inflictor)
		if trace_fraction == 1 then
			return true
		end
	end
	return false
end

function Killed(targ, attacker)
	if targ.health < -99 then
		targ.health = -99
	end
	-- Doors, triggers and so on.
	if targ.movetype == MOVETYPE_PUSH or targ.movetype == MOVETYPE_NONE then
		targ.th_die(targ)
		return
	end
	targ.enemy = attacker
	if bit32.band(targ.flags, FL_MONSTER) ~= 0 then
		killed_monsters += 1
	end
	ClientObituary(targ, attacker)
	targ.takedamage = DAMAGE_NO
	targ.touch = SUB_Null
	monster_death_use(targ)
	if targ.th_die then
		targ.th_die(targ)
	end
end

-- The damage is coming from inflictor, but get mad at attacker. This should be
-- the only function that ever reduces health.
function T_Damage(targ, inflictor, attacker, damage)
	if (targ.takedamage or 0) == 0 then
		return
	end
	damage_attacker = attacker

	-- Check for quad damage on the attacker.
	if attacker and (attacker.super_damage_finished or 0) > time() then
		damage = damage * 4
	end

	-- Save damage based on the target's armor level.
	local armortype = targ.armortype or 0
	local armorvalue = targ.armorvalue or 0
	local save = math.ceil(armortype * damage)
	if save >= armorvalue then
		save = armorvalue
		targ.armortype = 0
		local armor = bit32.bor(IT_ARMOR1, IT_ARMOR2, IT_ARMOR3)
		targ.items = (targ.items or 0) - bit32.band(targ.items or 0, armor)
	end
	targ.armorvalue = armorvalue - save
	local take = math.ceil(damage - save)

	-- Add to the damage total for the player's screen.
	if bit32.band(targ.flags, FL_CLIENT) ~= 0 then
		targ.dmg_take = (targ.dmg_take or 0) + take
		targ.dmg_save = (targ.dmg_save or 0) + save
		targ.dmg_inflictor = inflictor
	end

	-- Figure momentum add.
	if inflictor ~= world and inflictor ~= nil and targ.movetype == MOVETYPE_WALK then
		local dir = normalize(targ.origin - (inflictor.absmin + inflictor.absmax) * 0.5)
		targ.velocity = targ.velocity + dir * (damage * 8)
	end

	-- Check for godmode or invincibility.
	if bit32.band(targ.flags, FL_GODMODE) ~= 0 then
		return
	end
	if (targ.invincible_finished or 0) >= time() then
		if (targ.invincible_sound or 0) < time() then
			sound(targ, CHAN_ITEM, "items/protect3.wav", 1, ATTN_NORM)
			targ.invincible_sound = time() + 2
		end
		return
	end

	targ.health = (targ.health or 0) - take
	if targ.health <= 0 then
		Killed(targ, attacker)
		return
	end

	-- React to the damage.
	if bit32.band(targ.flags, FL_MONSTER) ~= 0 and exists(attacker) then
		-- Get mad, unless it's the same class. Soldiers don't care.
		if targ ~= attacker and attacker ~= targ.enemy then
			if targ.classname ~= attacker.classname or targ.classname == "monster_army" then
				if ent(targ.enemy).classname == "player" then
					targ.oldenemy = targ.enemy
				end
				targ.enemy = attacker
				FoundTarget(targ)
			end
		end
	end

	if targ.th_pain then
		targ.th_pain(targ, attacker, take)
		-- Nightmare mode monsters don't go into pain frames often.
		if skill == 3 then
			targ.pain_finished = time() + 5
		end
	end
end

function T_RadiusDamage(inflictor, attacker, damage, ignore)
	for _, head in findradius(inflictor.origin, damage + 40) do
		if head ~= ignore and (head.takedamage or 0) ~= 0 then
			local org = head.origin + (head.mins + head.maxs) * 0.5
			local points = math.max(0.5 * vector.magnitude(inflictor.origin - org), 0)
			points = damage - points
			if head == attacker then
				points = points * 0.5
			end
			if points > 0 and CanDamage(head, inflictor) then
				if head.classname == "monster_shambler" then
					T_Damage(head, inflictor, attacker, points * 0.5)
				else
					T_Damage(head, inflictor, attacker, points)
				end
			end
		end
	end
end
