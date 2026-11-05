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
-- Ported from enforcer.qc.

function enforcer_fire(self)
	local org
	self.effects = bit32.bor(self.effects, EF_MUZZLEFLASH)
	makevectors(self.angles)
	org = self.origin + v_forward * 30 + v_right * 8.5 + vector.create(0, 0, 16)
	LaunchLaser(self, org, ent(self.enemy).origin - self.origin)
end

function enf_stand1(self)
	animate(self, 0, enf_stand2)
	ai_stand(self)
end

function enf_stand2(self)
	animate(self, 1, enf_stand3)
	ai_stand(self)
end

function enf_stand3(self)
	animate(self, 2, enf_stand4)
	ai_stand(self)
end

function enf_stand4(self)
	animate(self, 3, enf_stand5)
	ai_stand(self)
end

function enf_stand5(self)
	animate(self, 4, enf_stand6)
	ai_stand(self)
end

function enf_stand6(self)
	animate(self, 5, enf_stand7)
	ai_stand(self)
end

function enf_stand7(self)
	animate(self, 6, enf_stand1)
	ai_stand(self)
end

function enf_walk1(self)
	animate(self, 7, enf_walk2)
	if random() < 0.2 then
		sound(self, CHAN_VOICE, "enforcer/idle1.wav", 1, ATTN_IDLE)
	end
	ai_walk(self, 2)
end

function enf_walk2(self)
	animate(self, 8, enf_walk3)
	ai_walk(self, 4)
end

function enf_walk3(self)
	animate(self, 9, enf_walk4)
	ai_walk(self, 4)
end

function enf_walk4(self)
	animate(self, 10, enf_walk5)
	ai_walk(self, 3)
end

function enf_walk5(self)
	animate(self, 11, enf_walk6)
	ai_walk(self, 1)
end

function enf_walk6(self)
	animate(self, 12, enf_walk7)
	ai_walk(self, 2)
end

function enf_walk7(self)
	animate(self, 13, enf_walk8)
	ai_walk(self, 2)
end

function enf_walk8(self)
	animate(self, 14, enf_walk9)
	ai_walk(self, 1)
end

function enf_walk9(self)
	animate(self, 15, enf_walk10)
	ai_walk(self, 2)
end

function enf_walk10(self)
	animate(self, 16, enf_walk11)
	ai_walk(self, 4)
end

function enf_walk11(self)
	animate(self, 17, enf_walk12)
	ai_walk(self, 4)
end

function enf_walk12(self)
	animate(self, 18, enf_walk13)
	ai_walk(self, 1)
end

function enf_walk13(self)
	animate(self, 19, enf_walk14)
	ai_walk(self, 2)
end

function enf_walk14(self)
	animate(self, 20, enf_walk15)
	ai_walk(self, 3)
end

function enf_walk15(self)
	animate(self, 21, enf_walk16)
	ai_walk(self, 4)
end

function enf_walk16(self)
	animate(self, 22, enf_walk1)
	ai_walk(self, 2)
end

function enf_run1(self)
	animate(self, 23, enf_run2)
	if random() < 0.2 then
		sound(self, CHAN_VOICE, "enforcer/idle1.wav", 1, ATTN_IDLE)
	end
	ai_run(self, 18)
end

function enf_run2(self)
	animate(self, 24, enf_run3)
	ai_run(self, 14)
end

function enf_run3(self)
	animate(self, 25, enf_run4)
	ai_run(self, 7)
end

function enf_run4(self)
	animate(self, 26, enf_run5)
	ai_run(self, 12)
end

function enf_run5(self)
	animate(self, 27, enf_run6)
	ai_run(self, 14)
end

function enf_run6(self)
	animate(self, 28, enf_run7)
	ai_run(self, 14)
end

function enf_run7(self)
	animate(self, 29, enf_run8)
	ai_run(self, 7)
end

function enf_run8(self)
	animate(self, 30, enf_run1)
	ai_run(self, 11)
end

function enf_atk1(self)
	animate(self, 31, enf_atk2)
	ai_face(self)
end

function enf_atk2(self)
	animate(self, 32, enf_atk3)
	ai_face(self)
end

function enf_atk3(self)
	animate(self, 33, enf_atk4)
	ai_face(self)
end

function enf_atk4(self)
	animate(self, 34, enf_atk5)
	ai_face(self)
end

function enf_atk5(self)
	animate(self, 35, enf_atk6)
	ai_face(self)
end

function enf_atk6(self)
	animate(self, 36, enf_atk7)
	enforcer_fire(self)
end

function enf_atk7(self)
	animate(self, 37, enf_atk8)
	ai_face(self)
end

function enf_atk8(self)
	animate(self, 38, enf_atk9)
	ai_face(self)
end

function enf_atk9(self)
	animate(self, 35, enf_atk10)
	ai_face(self)
end

function enf_atk10(self)
	animate(self, 36, enf_atk11)
	enforcer_fire(self)
end

function enf_atk11(self)
	animate(self, 37, enf_atk12)
	ai_face(self)
end

function enf_atk12(self)
	animate(self, 38, enf_atk13)
	ai_face(self)
end

function enf_atk13(self)
	animate(self, 39, enf_atk14)
	ai_face(self)
end

function enf_atk14(self)
	animate(self, 40, enf_run1)
	ai_face(self)
	SUB_CheckRefire(self, enf_atk1)
end

function enf_paina1(self)
	animate(self, 66, enf_paina2)
end

function enf_paina2(self)
	animate(self, 67, enf_paina3)
end

function enf_paina3(self)
	animate(self, 68, enf_paina4)
end

function enf_paina4(self)
	animate(self, 69, enf_run1)
end

function enf_painb1(self)
	animate(self, 70, enf_painb2)
end

function enf_painb2(self)
	animate(self, 71, enf_painb3)
end

function enf_painb3(self)
	animate(self, 72, enf_painb4)
end

function enf_painb4(self)
	animate(self, 73, enf_painb5)
end

function enf_painb5(self)
	animate(self, 74, enf_run1)
end

function enf_painc1(self)
	animate(self, 75, enf_painc2)
end

function enf_painc2(self)
	animate(self, 76, enf_painc3)
end

function enf_painc3(self)
	animate(self, 77, enf_painc4)
end

function enf_painc4(self)
	animate(self, 78, enf_painc5)
end

function enf_painc5(self)
	animate(self, 79, enf_painc6)
end

function enf_painc6(self)
	animate(self, 80, enf_painc7)
end

function enf_painc7(self)
	animate(self, 81, enf_painc8)
end

function enf_painc8(self)
	animate(self, 82, enf_run1)
end

function enf_paind1(self)
	animate(self, 83, enf_paind2)
end

function enf_paind2(self)
	animate(self, 84, enf_paind3)
end

function enf_paind3(self)
	animate(self, 85, enf_paind4)
end

function enf_paind4(self)
	animate(self, 86, enf_paind5)
	ai_painforward(self, 2)
end

function enf_paind5(self)
	animate(self, 87, enf_paind6)
	ai_painforward(self, 1)
end

function enf_paind6(self)
	animate(self, 88, enf_paind7)
end

function enf_paind7(self)
	animate(self, 89, enf_paind8)
end

function enf_paind8(self)
	animate(self, 90, enf_paind9)
end

function enf_paind9(self)
	animate(self, 91, enf_paind10)
end

function enf_paind10(self)
	animate(self, 92, enf_paind11)
end

function enf_paind11(self)
	animate(self, 93, enf_paind12)
	ai_painforward(self, 1)
end

function enf_paind12(self)
	animate(self, 94, enf_paind13)
	ai_painforward(self, 1)
end

function enf_paind13(self)
	animate(self, 95, enf_paind14)
	ai_painforward(self, 1)
end

function enf_paind14(self)
	animate(self, 96, enf_paind15)
end

function enf_paind15(self)
	animate(self, 97, enf_paind16)
end

function enf_paind16(self)
	animate(self, 98, enf_paind17)
	ai_pain(self, 1)
end

function enf_paind17(self)
	animate(self, 99, enf_paind18)
	ai_pain(self, 1)
end

function enf_paind18(self)
	animate(self, 100, enf_paind19)
end

function enf_paind19(self)
	animate(self, 101, enf_run1)
end

function enf_pain(self, attacker, damage)
	local r
	r = random()
	if self.pain_finished > time() then
		return
	end
	if r < 0.5 then
		sound(self, CHAN_VOICE, "enforcer/pain1.wav", 1, ATTN_NORM)
	else
		sound(self, CHAN_VOICE, "enforcer/pain2.wav", 1, ATTN_NORM)
	end
	if r < 0.2 then
		self.pain_finished = time() + 1
		enf_paina1(self)
	elseif r < 0.4 then
		self.pain_finished = time() + 1
		enf_painb1(self)
	elseif r < 0.7 then
		self.pain_finished = time() + 1
		enf_painc1(self)
	else
		self.pain_finished = time() + 2
		enf_paind1(self)
	end
end

function enf_die1(self)
	animate(self, 41, enf_die2)
end

function enf_die2(self)
	animate(self, 42, enf_die3)
end

function enf_die3(self)
	animate(self, 43, enf_die4)
	self.solid = SOLID_NOT
	self.ammo_cells = 5
	DropBackpack(self)
end

function enf_die4(self)
	animate(self, 44, enf_die5)
	ai_forward(self, 14)
end

function enf_die5(self)
	animate(self, 45, enf_die6)
	ai_forward(self, 2)
end

function enf_die6(self)
	animate(self, 46, enf_die7)
end

function enf_die7(self)
	animate(self, 47, enf_die8)
end

function enf_die8(self)
	animate(self, 48, enf_die9)
end

function enf_die9(self)
	animate(self, 49, enf_die10)
	ai_forward(self, 3)
end

function enf_die10(self)
	animate(self, 50, enf_die11)
	ai_forward(self, 5)
end

function enf_die11(self)
	animate(self, 51, enf_die12)
	ai_forward(self, 5)
end

function enf_die12(self)
	animate(self, 52, enf_die13)
	ai_forward(self, 5)
end

function enf_die13(self)
	animate(self, 53, enf_die14)
end

function enf_die14(self)
	animate(self, 54, enf_die14)
end

function enf_fdie1(self)
	animate(self, 55, enf_fdie2)
end

function enf_fdie2(self)
	animate(self, 56, enf_fdie3)
end

function enf_fdie3(self)
	animate(self, 57, enf_fdie4)
	self.solid = SOLID_NOT
	self.ammo_cells = 5
	DropBackpack(self)
end

function enf_fdie4(self)
	animate(self, 58, enf_fdie5)
end

function enf_fdie5(self)
	animate(self, 59, enf_fdie6)
end

function enf_fdie6(self)
	animate(self, 60, enf_fdie7)
end

function enf_fdie7(self)
	animate(self, 61, enf_fdie8)
end

function enf_fdie8(self)
	animate(self, 62, enf_fdie9)
end

function enf_fdie9(self)
	animate(self, 63, enf_fdie10)
end

function enf_fdie10(self)
	animate(self, 64, enf_fdie11)
end

function enf_fdie11(self)
	animate(self, 65, enf_fdie11)
end

function enf_die(self)
	-- check for gib
	if self.health < -35 then
		sound(self, CHAN_VOICE, "player/udeath.wav", 1, ATTN_NORM)
		ThrowHead(self, "progs/h_mega.mdl", self.health)
		ThrowGib(self, "progs/gib1.mdl", self.health)
		ThrowGib(self, "progs/gib2.mdl", self.health)
		ThrowGib(self, "progs/gib3.mdl", self.health)
		return
	end
	sound(self, CHAN_VOICE, "enforcer/death1.wav", 1, ATTN_NORM)
	if random() > 0.5 then
		enf_die1(self)
	else
		enf_fdie1(self)
	end
end

function monster_enforcer(self)
	if deathmatch ~= 0 then
		remove(self)
		return
	end
	self.solid = SOLID_SLIDEBOX
	self.movetype = MOVETYPE_STEP
	setmodel(self, "progs/enforcer.mdl")
	setsize(self, vector.create(-16, -16, -24), vector.create(16, 16, 40))
	self.health = 80
	self.th_stand = enf_stand1
	self.th_walk = enf_walk1
	self.th_run = enf_run1
	self.th_pain = enf_pain
	self.th_die = enf_die
	self.th_missile = enf_atk1
	walkmonster_start(self)
end
