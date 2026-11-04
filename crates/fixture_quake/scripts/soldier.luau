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
-- Ported from soldier.qc.

function army_stand1(self)
	animate(self, 0, army_stand2)
	ai_stand(self)
end

function army_stand2(self)
	animate(self, 1, army_stand3)
	ai_stand(self)
end

function army_stand3(self)
	animate(self, 2, army_stand4)
	ai_stand(self)
end

function army_stand4(self)
	animate(self, 3, army_stand5)
	ai_stand(self)
end

function army_stand5(self)
	animate(self, 4, army_stand6)
	ai_stand(self)
end

function army_stand6(self)
	animate(self, 5, army_stand7)
	ai_stand(self)
end

function army_stand7(self)
	animate(self, 6, army_stand8)
	ai_stand(self)
end

function army_stand8(self)
	animate(self, 7, army_stand1)
	ai_stand(self)
end

function army_walk1(self)
	animate(self, 90, army_walk2)
	if random() < 0.2 then
		sound(self, CHAN_VOICE, "soldier/idle.wav", 1, ATTN_IDLE)
	end
	ai_walk(self, 1)
end

function army_walk2(self)
	animate(self, 91, army_walk3)
	ai_walk(self, 1)
end

function army_walk3(self)
	animate(self, 92, army_walk4)
	ai_walk(self, 1)
end

function army_walk4(self)
	animate(self, 93, army_walk5)
	ai_walk(self, 1)
end

function army_walk5(self)
	animate(self, 94, army_walk6)
	ai_walk(self, 2)
end

function army_walk6(self)
	animate(self, 95, army_walk7)
	ai_walk(self, 3)
end

function army_walk7(self)
	animate(self, 96, army_walk8)
	ai_walk(self, 4)
end

function army_walk8(self)
	animate(self, 97, army_walk9)
	ai_walk(self, 4)
end

function army_walk9(self)
	animate(self, 98, army_walk10)
	ai_walk(self, 2)
end

function army_walk10(self)
	animate(self, 99, army_walk11)
	ai_walk(self, 2)
end

function army_walk11(self)
	animate(self, 100, army_walk12)
	ai_walk(self, 2)
end

function army_walk12(self)
	animate(self, 101, army_walk13)
	ai_walk(self, 1)
end

function army_walk13(self)
	animate(self, 102, army_walk14)
	ai_walk(self, 0)
end

function army_walk14(self)
	animate(self, 103, army_walk15)
	ai_walk(self, 1)
end

function army_walk15(self)
	animate(self, 104, army_walk16)
	ai_walk(self, 1)
end

function army_walk16(self)
	animate(self, 105, army_walk17)
	ai_walk(self, 1)
end

function army_walk17(self)
	animate(self, 106, army_walk18)
	ai_walk(self, 3)
end

function army_walk18(self)
	animate(self, 107, army_walk19)
	ai_walk(self, 3)
end

function army_walk19(self)
	animate(self, 108, army_walk20)
	ai_walk(self, 3)
end

function army_walk20(self)
	animate(self, 109, army_walk21)
	ai_walk(self, 3)
end

function army_walk21(self)
	animate(self, 110, army_walk22)
	ai_walk(self, 2)
end

function army_walk22(self)
	animate(self, 111, army_walk23)
	ai_walk(self, 1)
end

function army_walk23(self)
	animate(self, 112, army_walk24)
	ai_walk(self, 1)
end

function army_walk24(self)
	animate(self, 113, army_walk1)
	ai_walk(self, 1)
end

function army_run1(self)
	animate(self, 73, army_run2)
	if random() < 0.2 then
		sound(self, CHAN_VOICE, "soldier/idle.wav", 1, ATTN_IDLE)
	end
	ai_run(self, 11)
end

function army_run2(self)
	animate(self, 74, army_run3)
	ai_run(self, 15)
end

function army_run3(self)
	animate(self, 75, army_run4)
	ai_run(self, 10)
end

function army_run4(self)
	animate(self, 76, army_run5)
	ai_run(self, 10)
end

function army_run5(self)
	animate(self, 77, army_run6)
	ai_run(self, 8)
end

function army_run6(self)
	animate(self, 78, army_run7)
	ai_run(self, 15)
end

function army_run7(self)
	animate(self, 79, army_run8)
	ai_run(self, 10)
end

function army_run8(self)
	animate(self, 80, army_run1)
	ai_run(self, 8)
end

function army_atk1(self)
	animate(self, 81, army_atk2)
	ai_face(self)
end

function army_atk2(self)
	animate(self, 82, army_atk3)
	ai_face(self)
end

function army_atk3(self)
	animate(self, 83, army_atk4)
	ai_face(self)
end

function army_atk4(self)
	animate(self, 84, army_atk5)
	ai_face(self)
end

function army_atk5(self)
	animate(self, 85, army_atk6)
	ai_face(self)
	army_fire(self)
	self.effects = bit32.bor(self.effects, EF_MUZZLEFLASH)
end

function army_atk6(self)
	animate(self, 86, army_atk7)
	ai_face(self)
end

function army_atk7(self)
	animate(self, 87, army_atk8)
	ai_face(self)
	SUB_CheckRefire(army_atk1)
end

function army_atk8(self)
	animate(self, 88, army_atk9)
	ai_face(self)
end

function army_atk9(self)
	animate(self, 89, army_run1)
	ai_face(self)
end

function army_pain1(self)
	animate(self, 40, army_pain2)
end

function army_pain2(self)
	animate(self, 41, army_pain3)
end

function army_pain3(self)
	animate(self, 42, army_pain4)
end

function army_pain4(self)
	animate(self, 43, army_pain5)
end

function army_pain5(self)
	animate(self, 44, army_pain6)
end

function army_pain6(self)
	animate(self, 45, army_run1)
	ai_pain(self, 1)
end

function army_painb1(self)
	animate(self, 46, army_painb2)
end

function army_painb2(self)
	animate(self, 47, army_painb3)
	ai_painforward(self, 13)
end

function army_painb3(self)
	animate(self, 48, army_painb4)
	ai_painforward(self, 9)
end

function army_painb4(self)
	animate(self, 49, army_painb5)
end

function army_painb5(self)
	animate(self, 50, army_painb6)
end

function army_painb6(self)
	animate(self, 51, army_painb7)
end

function army_painb7(self)
	animate(self, 52, army_painb8)
end

function army_painb8(self)
	animate(self, 53, army_painb9)
end

function army_painb9(self)
	animate(self, 54, army_painb10)
end

function army_painb10(self)
	animate(self, 55, army_painb11)
end

function army_painb11(self)
	animate(self, 56, army_painb12)
end

function army_painb12(self)
	animate(self, 57, army_painb13)
	ai_pain(self, 2)
end

function army_painb13(self)
	animate(self, 58, army_painb14)
end

function army_painb14(self)
	animate(self, 59, army_run1)
end

function army_painc1(self)
	animate(self, 60, army_painc2)
end

function army_painc2(self)
	animate(self, 61, army_painc3)
	ai_pain(self, 1)
end

function army_painc3(self)
	animate(self, 62, army_painc4)
end

function army_painc4(self)
	animate(self, 63, army_painc5)
end

function army_painc5(self)
	animate(self, 64, army_painc6)
	ai_painforward(self, 1)
end

function army_painc6(self)
	animate(self, 65, army_painc7)
	ai_painforward(self, 1)
end

function army_painc7(self)
	animate(self, 66, army_painc8)
end

function army_painc8(self)
	animate(self, 67, army_painc9)
	ai_pain(self, 1)
end

function army_painc9(self)
	animate(self, 68, army_painc10)
	ai_painforward(self, 4)
end

function army_painc10(self)
	animate(self, 69, army_painc11)
	ai_painforward(self, 3)
end

function army_painc11(self)
	animate(self, 70, army_painc12)
	ai_painforward(self, 6)
end

function army_painc12(self)
	animate(self, 71, army_painc13)
	ai_painforward(self, 8)
end

function army_painc13(self)
	animate(self, 72, army_run1)
end

function army_pain(self, attacker, damage)
	local r
	if self.pain_finished > time() then
		return
	end
	r = random()
	if r < 0.2 then
		self.pain_finished = time() + 0.6
		army_pain1(self)
		sound(self, CHAN_VOICE, "soldier/pain1.wav", 1, ATTN_NORM)
	elseif r < 0.6 then
		self.pain_finished = time() + 1.1
		army_painb1(self)
		sound(self, CHAN_VOICE, "soldier/pain2.wav", 1, ATTN_NORM)
	else
		self.pain_finished = time() + 1.1
		army_painc1(self)
		sound(self, CHAN_VOICE, "soldier/pain2.wav", 1, ATTN_NORM)
	end
end

function army_fire(self)
	local dir, en
	ai_face(self)
	sound(self, CHAN_WEAPON, "soldier/sattck1.wav", 1, ATTN_NORM)
	-- fire somewhat behind the player, so a dodging player is harder to hit
	en = self.enemy
	dir = en.origin - en.velocity * 0.2
	dir = normalize(dir - self.origin)
	FireBullets(self, 4, dir, vector.create(0.1, 0.1, 0))
end

function army_die1(self)
	animate(self, 8, army_die2)
end

function army_die2(self)
	animate(self, 9, army_die3)
end

function army_die3(self)
	animate(self, 10, army_die4)
	self.solid = SOLID_NOT
	self.ammo_shells = 5
	DropBackpack(self)
end

function army_die4(self)
	animate(self, 11, army_die5)
end

function army_die5(self)
	animate(self, 12, army_die6)
end

function army_die6(self)
	animate(self, 13, army_die7)
end

function army_die7(self)
	animate(self, 14, army_die8)
end

function army_die8(self)
	animate(self, 15, army_die9)
end

function army_die9(self)
	animate(self, 16, army_die10)
end

function army_die10(self)
	animate(self, 17, army_die10)
end

function army_cdie1(self)
	animate(self, 18, army_cdie2)
end

function army_cdie2(self)
	animate(self, 19, army_cdie3)
	ai_back(self, 5)
end

function army_cdie3(self)
	animate(self, 20, army_cdie4)
	self.solid = SOLID_NOT
	self.ammo_shells = 5
	DropBackpack(self)
	ai_back(self, 4)
end

function army_cdie4(self)
	animate(self, 21, army_cdie5)
	ai_back(self, 13)
end

function army_cdie5(self)
	animate(self, 22, army_cdie6)
	ai_back(self, 3)
end

function army_cdie6(self)
	animate(self, 23, army_cdie7)
	ai_back(self, 4)
end

function army_cdie7(self)
	animate(self, 24, army_cdie8)
end

function army_cdie8(self)
	animate(self, 25, army_cdie9)
end

function army_cdie9(self)
	animate(self, 26, army_cdie10)
end

function army_cdie10(self)
	animate(self, 27, army_cdie11)
end

function army_cdie11(self)
	animate(self, 28, army_cdie11)
end

function army_die(self)
	-- check for gib
	if self.health < -35 then
		sound(self, CHAN_VOICE, "player/udeath.wav", 1, ATTN_NORM)
		ThrowHead(self, "progs/h_guard.mdl", self.health)
		ThrowGib(self, "progs/gib1.mdl", self.health)
		ThrowGib(self, "progs/gib2.mdl", self.health)
		ThrowGib(self, "progs/gib3.mdl", self.health)
		return
	end
	sound(self, CHAN_VOICE, "soldier/death1.wav", 1, ATTN_NORM)
	if random() < 0.5 then
		army_die1(self)
	else
		army_cdie1(self)
	end
end

function monster_army(self)
	if deathmatch ~= 0 then
		remove(self)
		return
	end
	-- gib death
	self.solid = SOLID_SLIDEBOX
	self.movetype = MOVETYPE_STEP
	setmodel(self, "progs/soldier.mdl")
	setsize(self, vector.create(-16, -16, -24), vector.create(16, 16, 40))
	self.health = 30
	self.th_stand = army_stand1
	self.th_walk = army_walk1
	self.th_run = army_run1
	self.th_missile = army_atk1
	self.th_pain = army_pain
	self.th_die = army_die
	walkmonster_start(self)
end
