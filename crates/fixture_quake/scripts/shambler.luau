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
-- Ported from shambler.qc.

function sham_stand1(self)
	animate(self, 0, sham_stand2)
	ai_stand(self)
end

function sham_stand2(self)
	animate(self, 1, sham_stand3)
	ai_stand(self)
end

function sham_stand3(self)
	animate(self, 2, sham_stand4)
	ai_stand(self)
end

function sham_stand4(self)
	animate(self, 3, sham_stand5)
	ai_stand(self)
end

function sham_stand5(self)
	animate(self, 4, sham_stand6)
	ai_stand(self)
end

function sham_stand6(self)
	animate(self, 5, sham_stand7)
	ai_stand(self)
end

function sham_stand7(self)
	animate(self, 6, sham_stand8)
	ai_stand(self)
end

function sham_stand8(self)
	animate(self, 7, sham_stand9)
	ai_stand(self)
end

function sham_stand9(self)
	animate(self, 8, sham_stand10)
	ai_stand(self)
end

function sham_stand10(self)
	animate(self, 9, sham_stand11)
	ai_stand(self)
end

function sham_stand11(self)
	animate(self, 10, sham_stand12)
	ai_stand(self)
end

function sham_stand12(self)
	animate(self, 11, sham_stand13)
	ai_stand(self)
end

function sham_stand13(self)
	animate(self, 12, sham_stand14)
	ai_stand(self)
end

function sham_stand14(self)
	animate(self, 13, sham_stand15)
	ai_stand(self)
end

function sham_stand15(self)
	animate(self, 14, sham_stand16)
	ai_stand(self)
end

function sham_stand16(self)
	animate(self, 15, sham_stand17)
	ai_stand(self)
end

function sham_stand17(self)
	animate(self, 16, sham_stand1)
	ai_stand(self)
end

function sham_walk1(self)
	animate(self, 17, sham_walk2)
	ai_walk(self, 10)
end

function sham_walk2(self)
	animate(self, 18, sham_walk3)
	ai_walk(self, 9)
end

function sham_walk3(self)
	animate(self, 19, sham_walk4)
	ai_walk(self, 9)
end

function sham_walk4(self)
	animate(self, 20, sham_walk5)
	ai_walk(self, 5)
end

function sham_walk5(self)
	animate(self, 21, sham_walk6)
	ai_walk(self, 6)
end

function sham_walk6(self)
	animate(self, 22, sham_walk7)
	ai_walk(self, 12)
end

function sham_walk7(self)
	animate(self, 23, sham_walk8)
	ai_walk(self, 8)
end

function sham_walk8(self)
	animate(self, 24, sham_walk9)
	ai_walk(self, 3)
end

function sham_walk9(self)
	animate(self, 25, sham_walk10)
	ai_walk(self, 13)
end

function sham_walk10(self)
	animate(self, 26, sham_walk11)
	ai_walk(self, 9)
end

function sham_walk11(self)
	animate(self, 27, sham_walk12)
	ai_walk(self, 7)
end

function sham_walk12(self)
	animate(self, 28, sham_walk1)
	ai_walk(self, 7)
	if random() > 0.8 then
		sound(self, CHAN_VOICE, "shambler/sidle.wav", 1, ATTN_IDLE)
	end
end

function sham_run1(self)
	animate(self, 29, sham_run2)
	ai_run(self, 20)
end

function sham_run2(self)
	animate(self, 30, sham_run3)
	ai_run(self, 24)
end

function sham_run3(self)
	animate(self, 31, sham_run4)
	ai_run(self, 20)
end

function sham_run4(self)
	animate(self, 32, sham_run5)
	ai_run(self, 20)
end

function sham_run5(self)
	animate(self, 33, sham_run6)
	ai_run(self, 24)
end

function sham_run6(self)
	animate(self, 34, sham_run1)
	ai_run(self, 20)
	if random() > 0.8 then
		sound(self, CHAN_VOICE, "shambler/sidle.wav", 1, ATTN_IDLE)
	end
end

function sham_smash1(self)
	animate(self, 35, sham_smash2)
	sound(self, CHAN_VOICE, "shambler/melee1.wav", 1, ATTN_NORM)
	ai_charge(self, 2)
end

function sham_smash2(self)
	animate(self, 36, sham_smash3)
	ai_charge(self, 6)
end

function sham_smash3(self)
	animate(self, 37, sham_smash4)
	ai_charge(self, 6)
end

function sham_smash4(self)
	animate(self, 38, sham_smash5)
	ai_charge(self, 5)
end

function sham_smash5(self)
	animate(self, 39, sham_smash6)
	ai_charge(self, 4)
end

function sham_smash6(self)
	animate(self, 40, sham_smash7)
	ai_charge(self, 1)
end

function sham_smash7(self)
	animate(self, 41, sham_smash8)
	ai_charge(self, 0)
end

function sham_smash8(self)
	animate(self, 42, sham_smash9)
	ai_charge(self, 0)
end

function sham_smash9(self)
	animate(self, 43, sham_smash10)
	ai_charge(self, 0)
end

function sham_smash10(self)
	local delta, ldmg
	animate(self, 44, sham_smash11)
	if not exists(self.enemy) then
		return
	end
	ai_charge(self, 0)
	delta = ent(self.enemy).origin - self.origin
	if vector.magnitude(delta) > 100 then
		return
	end
	if not CanDamage(self.enemy, self) then
		return
	end
	ldmg = (random() + random() + random()) * 40
	T_Damage(self.enemy, self, self, ldmg)
	sound(self, CHAN_VOICE, "shambler/smack.wav", 1, ATTN_NORM)
	SpawnMeatSpray(self, self.origin + v_forward * 16, crandom() * 100 * v_right)
	SpawnMeatSpray(self, self.origin + v_forward * 16, crandom() * 100 * v_right)
end

function sham_smash11(self)
	animate(self, 45, sham_smash12)
	ai_charge(self, 5)
end

function sham_smash12(self)
	animate(self, 46, sham_run1)
	ai_charge(self, 4)
end

function ShamClaw(self, side)
	local delta, ldmg
	if not exists(self.enemy) then
		return
	end
	ai_charge(self, 10)
	delta = ent(self.enemy).origin - self.origin
	if vector.magnitude(delta) > 100 then
		return
	end
	ldmg = (random() + random() + random()) * 20
	T_Damage(self.enemy, self, self, ldmg)
	sound(self, CHAN_VOICE, "shambler/smack.wav", 1, ATTN_NORM)
	if side ~= 0 then
		makevectors(self.angles)
		SpawnMeatSpray(self, self.origin + v_forward * 16, side * v_right)
	end
end

function sham_swingl1(self)
	animate(self, 56, sham_swingl2)
	sound(self, CHAN_VOICE, "shambler/melee2.wav", 1, ATTN_NORM)
	ai_charge(self, 5)
end

function sham_swingl2(self)
	animate(self, 57, sham_swingl3)
	ai_charge(self, 3)
end

function sham_swingl3(self)
	animate(self, 58, sham_swingl4)
	ai_charge(self, 7)
end

function sham_swingl4(self)
	animate(self, 59, sham_swingl5)
	ai_charge(self, 3)
end

function sham_swingl5(self)
	animate(self, 60, sham_swingl6)
	ai_charge(self, 7)
end

function sham_swingl6(self)
	animate(self, 61, sham_swingl7)
	ai_charge(self, 9)
end

function sham_swingl7(self)
	animate(self, 62, sham_swingl8)
	ai_charge(self, 5)
	ShamClaw(self, 250)
end

function sham_swingl8(self)
	animate(self, 63, sham_swingl9)
	ai_charge(self, 4)
end

function sham_swingl9(self)
	animate(self, 64, sham_run1)
	ai_charge(self, 8)
	if random() < 0.5 then
		self.think = sham_swingr1
	end
end

function sham_swingr1(self)
	animate(self, 47, sham_swingr2)
	sound(self, CHAN_VOICE, "shambler/melee1.wav", 1, ATTN_NORM)
	ai_charge(self, 1)
end

function sham_swingr2(self)
	animate(self, 48, sham_swingr3)
	ai_charge(self, 8)
end

function sham_swingr3(self)
	animate(self, 49, sham_swingr4)
	ai_charge(self, 14)
end

function sham_swingr4(self)
	animate(self, 50, sham_swingr5)
	ai_charge(self, 7)
end

function sham_swingr5(self)
	animate(self, 51, sham_swingr6)
	ai_charge(self, 3)
end

function sham_swingr6(self)
	animate(self, 52, sham_swingr7)
	ai_charge(self, 6)
end

function sham_swingr7(self)
	animate(self, 53, sham_swingr8)
	ai_charge(self, 6)
	ShamClaw(self, -250)
end

function sham_swingr8(self)
	animate(self, 54, sham_swingr9)
	ai_charge(self, 3)
end

function sham_swingr9(self)
	animate(self, 55, sham_run1)
	ai_charge(self, 1)
	ai_charge(self, 10)
	if random() < 0.5 then
		self.think = sham_swingl1
	end
end

function sham_melee(self)
	local chance
	chance = random()
	if chance > 0.6 or self.health == 600 then
		sham_smash1(self)
	elseif chance > 0.3 then
		sham_swingr1(self)
	else
		sham_swingl1(self)
	end
end

function CastLightning(self)
	local org, dir
	self.effects = bit32.bor(self.effects, EF_MUZZLEFLASH)
	ai_face(self)
	org = self.origin + vector.create(0, 0, 40)
	dir = ent(self.enemy).origin + vector.create(0, 0, 16) - org
	dir = normalize(dir)
	traceline(org, self.origin + dir * 600, true, self)
	temp_entity(TE_LIGHTNING1, org, trace_endpos, self)
	LightningDamage(self, org, trace_endpos, self, 10)
end

function sham_magic1(self)
	animate(self, 65, sham_magic2)
	ai_face(self)
	sound(self, CHAN_WEAPON, "shambler/sattck1.wav", 1, ATTN_NORM)
end

function sham_magic2(self)
	animate(self, 66, sham_magic3)
	ai_face(self)
end

function sham_magic3(self)
	local o
	animate(self, 67, sham_magic4)
	ai_face(self)
	self.nextthink = self.nextthink + 0.2
	self.effects = bit32.bor(self.effects, EF_MUZZLEFLASH)
	ai_face(self)
	self.owner = spawn()
	o = self.owner
	setmodel(o, "progs/s_light.mdl")
	setorigin(o, self.origin)
	o.angles = self.angles
	o.nextthink = time() + 0.7
	o.think = SUB_Remove
end

function sham_magic4(self)
	animate(self, 68, sham_magic5)
	self.effects = bit32.bor(self.effects, EF_MUZZLEFLASH)
	ent(self.owner).frame = 1
end

function sham_magic5(self)
	animate(self, 69, sham_magic6)
	self.effects = bit32.bor(self.effects, EF_MUZZLEFLASH)
	ent(self.owner).frame = 2
end

function sham_magic6(self)
	animate(self, 70, sham_magic9)
	remove(self.owner)
	CastLightning(self)
	sound(self, CHAN_WEAPON, "shambler/sboom.wav", 1, ATTN_NORM)
end

function sham_magic9(self)
	animate(self, 73, sham_magic10)
	CastLightning(self)
end

function sham_magic10(self)
	animate(self, 74, sham_magic11)
	CastLightning(self)
end

function sham_magic11(self)
	animate(self, 75, sham_magic12)
	if skill == 3 then
		CastLightning(self)
	end
end

function sham_magic12(self)
	animate(self, 76, sham_run1)
end

function sham_pain1(self)
	animate(self, 77, sham_pain2)
end

function sham_pain2(self)
	animate(self, 78, sham_pain3)
end

function sham_pain3(self)
	animate(self, 79, sham_pain4)
end

function sham_pain4(self)
	animate(self, 80, sham_pain5)
end

function sham_pain5(self)
	animate(self, 81, sham_pain6)
end

function sham_pain6(self)
	animate(self, 82, sham_run1)
end

function sham_pain(self, attacker, damage)
	sound(self, CHAN_VOICE, "shambler/shurt2.wav", 1, ATTN_NORM)
	if self.health <= 0 then
		return
	end
	if random() * 400 > damage then
		return
	end
	if self.pain_finished > time() then
		return
	end
	self.pain_finished = time() + 2
	sham_pain1(self)
end

function sham_death1(self)
	animate(self, 83, sham_death2)
end

function sham_death2(self)
	animate(self, 84, sham_death3)
end

function sham_death3(self)
	animate(self, 85, sham_death4)
	self.solid = SOLID_NOT
end

function sham_death4(self)
	animate(self, 86, sham_death5)
end

function sham_death5(self)
	animate(self, 87, sham_death6)
end

function sham_death6(self)
	animate(self, 88, sham_death7)
end

function sham_death7(self)
	animate(self, 89, sham_death8)
end

function sham_death8(self)
	animate(self, 90, sham_death9)
end

function sham_death9(self)
	animate(self, 91, sham_death10)
end

function sham_death10(self)
	animate(self, 92, sham_death11)
end

function sham_death11(self)
	animate(self, 93, sham_death11)
end

function sham_die(self)
	-- check for gib
	if self.health < -60 then
		sound(self, CHAN_VOICE, "player/udeath.wav", 1, ATTN_NORM)
		ThrowHead(self, "progs/h_shams.mdl", self.health)
		ThrowGib(self, "progs/gib1.mdl", self.health)
		ThrowGib(self, "progs/gib2.mdl", self.health)
		ThrowGib(self, "progs/gib3.mdl", self.health)
		return
	end
	sound(self, CHAN_VOICE, "shambler/sdeath.wav", 1, ATTN_NORM)
	sham_death1(self)
end

function monster_shambler(self)
	if deathmatch ~= 0 then
		remove(self)
		return
	end
	self.solid = SOLID_SLIDEBOX
	self.movetype = MOVETYPE_STEP
	setmodel(self, "progs/shambler.mdl")
	setsize(self, VEC_HULL2_MIN, VEC_HULL2_MAX)
	self.health = 600
	self.th_stand = sham_stand1
	self.th_walk = sham_walk1
	self.th_run = sham_run1
	self.th_die = sham_die
	self.th_melee = sham_melee
	self.th_missile = sham_magic1
	self.th_pain = sham_pain
	walkmonster_start(self)
end
