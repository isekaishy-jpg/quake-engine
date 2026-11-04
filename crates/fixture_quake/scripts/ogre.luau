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
-- Ported from ogre.qc.

function OgreGrenadeExplode(self)
	T_RadiusDamage(self, self.owner, 40, world)
	sound(self, CHAN_VOICE, "weapons/r_exp3.wav", 1, ATTN_NORM)
	temp_entity(TE_EXPLOSION, self.origin)
	self.velocity = vector.zero
	self.touch = SUB_Null
	setmodel(self, "progs/s_explod.spr")
	self.solid = SOLID_NOT
	s_explode1(self)
end

function OgreGrenadeTouch(self, other)
	if other == self.owner then
		return
	end
	if other.takedamage == DAMAGE_AIM then
		OgreGrenadeExplode(self)
		return
	end
	sound(self, CHAN_VOICE, "weapons/bounce.wav", 1, ATTN_NORM)
	-- bounce sound
	if self.velocity == vector.zero then
		self.avelocity = vector.zero
	end
end

function OgreFireGrenade(self)
	local missile
	self.effects = bit32.bor(self.effects, EF_MUZZLEFLASH)
	sound(self, CHAN_WEAPON, "weapons/grenade.wav", 1, ATTN_NORM)
	missile = spawn()
	missile.owner = self
	missile.movetype = MOVETYPE_BOUNCE
	missile.solid = SOLID_BBOX
	-- set missile speed
	makevectors(self.angles)
	missile.velocity = normalize(ent(self.enemy).origin - self.origin)
	missile.velocity = missile.velocity * 600
	missile.velocity = with_z(missile.velocity, 200)
	missile.avelocity = vector.create(300, 300, 300)
	missile.angles = vectoangles(missile.velocity)
	missile.touch = OgreGrenadeTouch
	-- set missile duration
	missile.nextthink = time() + 2.5
	missile.think = OgreGrenadeExplode
	setmodel(missile, "progs/grenade.mdl")
	setsize(missile, vector.zero, vector.zero)
	setorigin(missile, self.origin)
end

function chainsaw(self, side)
	local delta, ldmg
	if not exists(self.enemy) then
		return
	end
	if not CanDamage(self.enemy, self) then
		return
	end
	ai_charge(self, 10)
	delta = ent(self.enemy).origin - self.origin
	if vector.magnitude(delta) > 100 then
		return
	end
	ldmg = (random() + random() + random()) * 4
	T_Damage(self.enemy, self, self, ldmg)
	if side ~= 0 then
		makevectors(self.angles)
		if side == 1 then
			SpawnMeatSpray(self, self.origin + v_forward * 16, crandom() * 100 * v_right)
		else
			SpawnMeatSpray(self, self.origin + v_forward * 16, side * v_right)
		end
	end
end

function ogre_stand1(self)
	animate(self, 0, ogre_stand2)
	ai_stand(self)
end

function ogre_stand2(self)
	animate(self, 1, ogre_stand3)
	ai_stand(self)
end

function ogre_stand3(self)
	animate(self, 2, ogre_stand4)
	ai_stand(self)
end

function ogre_stand4(self)
	animate(self, 3, ogre_stand5)
	ai_stand(self)
end

function ogre_stand5(self)
	animate(self, 4, ogre_stand6)
	if random() < 0.2 then
		sound(self, CHAN_VOICE, "ogre/ogidle.wav", 1, ATTN_IDLE)
	end
	ai_stand(self)
end

function ogre_stand6(self)
	animate(self, 5, ogre_stand7)
	ai_stand(self)
end

function ogre_stand7(self)
	animate(self, 6, ogre_stand8)
	ai_stand(self)
end

function ogre_stand8(self)
	animate(self, 7, ogre_stand9)
	ai_stand(self)
end

function ogre_stand9(self)
	animate(self, 8, ogre_stand1)
	ai_stand(self)
end

function ogre_walk1(self)
	animate(self, 9, ogre_walk2)
	ai_walk(self, 3)
end

function ogre_walk2(self)
	animate(self, 10, ogre_walk3)
	ai_walk(self, 2)
end

function ogre_walk3(self)
	animate(self, 11, ogre_walk4)
	ai_walk(self, 2)
	if random() < 0.2 then
		sound(self, CHAN_VOICE, "ogre/ogidle.wav", 1, ATTN_IDLE)
	end
end

function ogre_walk4(self)
	animate(self, 12, ogre_walk5)
	ai_walk(self, 2)
end

function ogre_walk5(self)
	animate(self, 13, ogre_walk6)
	ai_walk(self, 2)
end

function ogre_walk6(self)
	animate(self, 14, ogre_walk7)
	ai_walk(self, 5)
	if random() < 0.1 then
		sound(self, CHAN_VOICE, "ogre/ogdrag.wav", 1, ATTN_IDLE)
	end
end

function ogre_walk7(self)
	animate(self, 15, ogre_walk8)
	ai_walk(self, 3)
end

function ogre_walk8(self)
	animate(self, 16, ogre_walk9)
	ai_walk(self, 2)
end

function ogre_walk9(self)
	animate(self, 17, ogre_walk10)
	ai_walk(self, 3)
end

function ogre_walk10(self)
	animate(self, 18, ogre_walk11)
	ai_walk(self, 1)
end

function ogre_walk11(self)
	animate(self, 19, ogre_walk12)
	ai_walk(self, 2)
end

function ogre_walk12(self)
	animate(self, 20, ogre_walk13)
	ai_walk(self, 3)
end

function ogre_walk13(self)
	animate(self, 21, ogre_walk14)
	ai_walk(self, 3)
end

function ogre_walk14(self)
	animate(self, 22, ogre_walk15)
	ai_walk(self, 3)
end

function ogre_walk15(self)
	animate(self, 23, ogre_walk16)
	ai_walk(self, 3)
end

function ogre_walk16(self)
	animate(self, 24, ogre_walk1)
	ai_walk(self, 4)
end

function ogre_run1(self)
	animate(self, 25, ogre_run2)
	ai_run(self, 9)
	if random() < 0.2 then
		sound(self, CHAN_VOICE, "ogre/ogidle2.wav", 1, ATTN_IDLE)
	end
end

function ogre_run2(self)
	animate(self, 26, ogre_run3)
	ai_run(self, 12)
end

function ogre_run3(self)
	animate(self, 27, ogre_run4)
	ai_run(self, 8)
end

function ogre_run4(self)
	animate(self, 28, ogre_run5)
	ai_run(self, 22)
end

function ogre_run5(self)
	animate(self, 29, ogre_run6)
	ai_run(self, 16)
end

function ogre_run6(self)
	animate(self, 30, ogre_run7)
	ai_run(self, 4)
end

function ogre_run7(self)
	animate(self, 31, ogre_run8)
	ai_run(self, 13)
end

function ogre_run8(self)
	animate(self, 32, ogre_run1)
	ai_run(self, 24)
end

function ogre_swing1(self)
	animate(self, 33, ogre_swing2)
	ai_charge(self, 11)
	sound(self, CHAN_WEAPON, "ogre/ogsawatk.wav", 1, ATTN_NORM)
end

function ogre_swing2(self)
	animate(self, 34, ogre_swing3)
	ai_charge(self, 1)
end

function ogre_swing3(self)
	animate(self, 35, ogre_swing4)
	ai_charge(self, 4)
end

function ogre_swing4(self)
	animate(self, 36, ogre_swing5)
	ai_charge(self, 13)
end

function ogre_swing5(self)
	animate(self, 37, ogre_swing6)
	ai_charge(self, 9)
	chainsaw(self, 0)
	self.angles = with_y(self.angles, self.angles.y + random() * 25)
end

function ogre_swing6(self)
	animate(self, 38, ogre_swing7)
	chainsaw(self, 200)
	self.angles = with_y(self.angles, self.angles.y + random() * 25)
end

function ogre_swing7(self)
	animate(self, 39, ogre_swing8)
	chainsaw(self, 0)
	self.angles = with_y(self.angles, self.angles.y + random() * 25)
end

function ogre_swing8(self)
	animate(self, 40, ogre_swing9)
	chainsaw(self, 0)
	self.angles = with_y(self.angles, self.angles.y + random() * 25)
end

function ogre_swing9(self)
	animate(self, 41, ogre_swing10)
	chainsaw(self, 0)
	self.angles = with_y(self.angles, self.angles.y + random() * 25)
end

function ogre_swing10(self)
	animate(self, 42, ogre_swing11)
	chainsaw(self, -200)
	self.angles = with_y(self.angles, self.angles.y + random() * 25)
end

function ogre_swing11(self)
	animate(self, 43, ogre_swing12)
	chainsaw(self, 0)
	self.angles = with_y(self.angles, self.angles.y + random() * 25)
end

function ogre_swing12(self)
	animate(self, 44, ogre_swing13)
	ai_charge(self, 3)
end

function ogre_swing13(self)
	animate(self, 45, ogre_swing14)
	ai_charge(self, 8)
end

function ogre_swing14(self)
	animate(self, 46, ogre_run1)
	ai_charge(self, 9)
end

function ogre_smash1(self)
	animate(self, 47, ogre_smash2)
	ai_charge(self, 6)
	sound(self, CHAN_WEAPON, "ogre/ogsawatk.wav", 1, ATTN_NORM)
end

function ogre_smash2(self)
	animate(self, 48, ogre_smash3)
	ai_charge(self, 0)
end

function ogre_smash3(self)
	animate(self, 49, ogre_smash4)
	ai_charge(self, 0)
end

function ogre_smash4(self)
	animate(self, 50, ogre_smash5)
	ai_charge(self, 1)
end

function ogre_smash5(self)
	animate(self, 51, ogre_smash6)
	ai_charge(self, 4)
end

function ogre_smash6(self)
	animate(self, 52, ogre_smash7)
	ai_charge(self, 4)
	chainsaw(self, 0)
end

function ogre_smash7(self)
	animate(self, 53, ogre_smash8)
	ai_charge(self, 4)
	chainsaw(self, 0)
end

function ogre_smash8(self)
	animate(self, 54, ogre_smash9)
	ai_charge(self, 10)
	chainsaw(self, 0)
end

function ogre_smash9(self)
	animate(self, 55, ogre_smash10)
	ai_charge(self, 13)
	chainsaw(self, 0)
end

function ogre_smash10(self)
	animate(self, 56, ogre_smash11)
	chainsaw(self, 1)
end

function ogre_smash11(self)
	animate(self, 57, ogre_smash12)
	ai_charge(self, 2)
	chainsaw(self, 0)
	self.nextthink = self.nextthink + random() * 0.2
end

-- slight variation
function ogre_smash12(self)
	animate(self, 58, ogre_smash13)
	ai_charge(self, 0)
end

function ogre_smash13(self)
	animate(self, 59, ogre_smash14)
	ai_charge(self, 4)
end

function ogre_smash14(self)
	animate(self, 60, ogre_run1)
	ai_charge(self, 12)
end

function ogre_nail1(self)
	animate(self, 61, ogre_nail2)
	ai_face(self)
end

function ogre_nail2(self)
	animate(self, 62, ogre_nail3)
	ai_face(self)
end

function ogre_nail3(self)
	animate(self, 62, ogre_nail4)
	ai_face(self)
end

function ogre_nail4(self)
	animate(self, 63, ogre_nail5)
	ai_face(self)
	OgreFireGrenade(self)
end

function ogre_nail5(self)
	animate(self, 64, ogre_nail6)
	ai_face(self)
end

function ogre_nail6(self)
	animate(self, 65, ogre_nail7)
	ai_face(self)
end

function ogre_nail7(self)
	animate(self, 66, ogre_run1)
	ai_face(self)
end

function ogre_pain1(self)
	animate(self, 67, ogre_pain2)
end

function ogre_pain2(self)
	animate(self, 68, ogre_pain3)
end

function ogre_pain3(self)
	animate(self, 69, ogre_pain4)
end

function ogre_pain4(self)
	animate(self, 70, ogre_pain5)
end

function ogre_pain5(self)
	animate(self, 71, ogre_run1)
end

function ogre_painb1(self)
	animate(self, 72, ogre_painb2)
end

function ogre_painb2(self)
	animate(self, 73, ogre_painb3)
end

function ogre_painb3(self)
	animate(self, 74, ogre_run1)
end

function ogre_painc1(self)
	animate(self, 75, ogre_painc2)
end

function ogre_painc2(self)
	animate(self, 76, ogre_painc3)
end

function ogre_painc3(self)
	animate(self, 77, ogre_painc4)
end

function ogre_painc4(self)
	animate(self, 78, ogre_painc5)
end

function ogre_painc5(self)
	animate(self, 79, ogre_painc6)
end

function ogre_painc6(self)
	animate(self, 80, ogre_run1)
end

function ogre_paind1(self)
	animate(self, 81, ogre_paind2)
end

function ogre_paind2(self)
	animate(self, 82, ogre_paind3)
	ai_pain(self, 10)
end

function ogre_paind3(self)
	animate(self, 83, ogre_paind4)
	ai_pain(self, 9)
end

function ogre_paind4(self)
	animate(self, 84, ogre_paind5)
	ai_pain(self, 4)
end

function ogre_paind5(self)
	animate(self, 85, ogre_paind6)
end

function ogre_paind6(self)
	animate(self, 86, ogre_paind7)
end

function ogre_paind7(self)
	animate(self, 87, ogre_paind8)
end

function ogre_paind8(self)
	animate(self, 88, ogre_paind9)
end

function ogre_paind9(self)
	animate(self, 89, ogre_paind10)
end

function ogre_paind10(self)
	animate(self, 90, ogre_paind11)
end

function ogre_paind11(self)
	animate(self, 91, ogre_paind12)
end

function ogre_paind12(self)
	animate(self, 92, ogre_paind13)
end

function ogre_paind13(self)
	animate(self, 93, ogre_paind14)
end

function ogre_paind14(self)
	animate(self, 94, ogre_paind15)
end

function ogre_paind15(self)
	animate(self, 95, ogre_paind16)
end

function ogre_paind16(self)
	animate(self, 96, ogre_run1)
end

function ogre_paine1(self)
	animate(self, 97, ogre_paine2)
end

function ogre_paine2(self)
	animate(self, 98, ogre_paine3)
	ai_pain(self, 10)
end

function ogre_paine3(self)
	animate(self, 99, ogre_paine4)
	ai_pain(self, 9)
end

function ogre_paine4(self)
	animate(self, 100, ogre_paine5)
	ai_pain(self, 4)
end

function ogre_paine5(self)
	animate(self, 101, ogre_paine6)
end

function ogre_paine6(self)
	animate(self, 102, ogre_paine7)
end

function ogre_paine7(self)
	animate(self, 103, ogre_paine8)
end

function ogre_paine8(self)
	animate(self, 104, ogre_paine9)
end

function ogre_paine9(self)
	animate(self, 105, ogre_paine10)
end

function ogre_paine10(self)
	animate(self, 106, ogre_paine11)
end

function ogre_paine11(self)
	animate(self, 107, ogre_paine12)
end

function ogre_paine12(self)
	animate(self, 108, ogre_paine13)
end

function ogre_paine13(self)
	animate(self, 109, ogre_paine14)
end

function ogre_paine14(self)
	animate(self, 110, ogre_paine15)
end

function ogre_paine15(self)
	animate(self, 111, ogre_run1)
end

function ogre_pain(self, attacker, damage)
	local r
	-- don't make multiple pain sounds right after each other
	if self.pain_finished > time() then
		return
	end
	sound(self, CHAN_VOICE, "ogre/ogpain1.wav", 1, ATTN_NORM)
	r = random()
	if r < 0.25 then
		ogre_pain1(self)
		self.pain_finished = time() + 1
	elseif r < 0.5 then
		ogre_painb1(self)
		self.pain_finished = time() + 1
	elseif r < 0.75 then
		ogre_painc1(self)
		self.pain_finished = time() + 1
	elseif r < 0.88 then
		ogre_paind1(self)
		self.pain_finished = time() + 2
	else
		ogre_paine1(self)
		self.pain_finished = time() + 2
	end
end

function ogre_die1(self)
	animate(self, 112, ogre_die2)
end

function ogre_die2(self)
	animate(self, 113, ogre_die3)
end

function ogre_die3(self)
	animate(self, 114, ogre_die4)
	self.solid = SOLID_NOT
	self.ammo_rockets = 2
	DropBackpack(self)
end

function ogre_die4(self)
	animate(self, 115, ogre_die5)
end

function ogre_die5(self)
	animate(self, 116, ogre_die6)
end

function ogre_die6(self)
	animate(self, 117, ogre_die7)
end

function ogre_die7(self)
	animate(self, 118, ogre_die8)
end

function ogre_die8(self)
	animate(self, 119, ogre_die9)
end

function ogre_die9(self)
	animate(self, 120, ogre_die10)
end

function ogre_die10(self)
	animate(self, 121, ogre_die11)
end

function ogre_die11(self)
	animate(self, 122, ogre_die12)
end

function ogre_die12(self)
	animate(self, 123, ogre_die13)
end

function ogre_die13(self)
	animate(self, 124, ogre_die14)
end

function ogre_die14(self)
	animate(self, 125, ogre_die14)
end

function ogre_bdie1(self)
	animate(self, 126, ogre_bdie2)
end

function ogre_bdie2(self)
	animate(self, 127, ogre_bdie3)
	ai_forward(self, 5)
end

function ogre_bdie3(self)
	animate(self, 128, ogre_bdie4)
	self.solid = SOLID_NOT
	self.ammo_rockets = 2
	DropBackpack(self)
end

function ogre_bdie4(self)
	animate(self, 129, ogre_bdie5)
	ai_forward(self, 1)
end

function ogre_bdie5(self)
	animate(self, 130, ogre_bdie6)
	ai_forward(self, 3)
end

function ogre_bdie6(self)
	animate(self, 131, ogre_bdie7)
	ai_forward(self, 7)
end

function ogre_bdie7(self)
	animate(self, 132, ogre_bdie8)
	ai_forward(self, 25)
end

function ogre_bdie8(self)
	animate(self, 133, ogre_bdie9)
end

function ogre_bdie9(self)
	animate(self, 134, ogre_bdie10)
end

function ogre_bdie10(self)
	animate(self, 135, ogre_bdie10)
end

function ogre_die(self)
	-- check for gib
	if self.health < -80 then
		sound(self, CHAN_VOICE, "player/udeath.wav", 1, ATTN_NORM)
		ThrowHead(self, "progs/h_ogre.mdl", self.health)
		ThrowGib(self, "progs/gib3.mdl", self.health)
		ThrowGib(self, "progs/gib3.mdl", self.health)
		ThrowGib(self, "progs/gib3.mdl", self.health)
		return
	end
	sound(self, CHAN_VOICE, "ogre/ogdth.wav", 1, ATTN_NORM)
	if random() < 0.5 then
		ogre_die1(self)
	else
		ogre_bdie1(self)
	end
end

function ogre_melee(self)
	if random() > 0.5 then
		ogre_smash1(self)
	else
		ogre_swing1(self)
	end
end

function monster_ogre(self)
	if deathmatch ~= 0 then
		remove(self)
		return
	end
	self.solid = SOLID_SLIDEBOX
	self.movetype = MOVETYPE_STEP
	setmodel(self, "progs/ogre.mdl")
	setsize(self, VEC_HULL2_MIN, VEC_HULL2_MAX)
	self.health = 200
	self.th_stand = ogre_stand1
	self.th_walk = ogre_walk1
	self.th_run = ogre_run1
	self.th_die = ogre_die
	self.th_melee = ogre_melee
	self.th_missile = ogre_nail1
	self.th_pain = ogre_pain
	walkmonster_start(self)
end

function monster_ogre_marksman(self)
	monster_ogre(self)
end
