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
-- Ported from knight.qc.

-- frame runc1 runc2 runc3 runc4 runc5 runc6
-- frame attack1 attack2 attack3 attack4 attack5 attack6 attack7
-- frame attack8 attack9 attack10 attack11
function knight_stand1(self)
	animate(self, 0, knight_stand2)
	ai_stand(self)
end

function knight_stand2(self)
	animate(self, 1, knight_stand3)
	ai_stand(self)
end

function knight_stand3(self)
	animate(self, 2, knight_stand4)
	ai_stand(self)
end

function knight_stand4(self)
	animate(self, 3, knight_stand5)
	ai_stand(self)
end

function knight_stand5(self)
	animate(self, 4, knight_stand6)
	ai_stand(self)
end

function knight_stand6(self)
	animate(self, 5, knight_stand7)
	ai_stand(self)
end

function knight_stand7(self)
	animate(self, 6, knight_stand8)
	ai_stand(self)
end

function knight_stand8(self)
	animate(self, 7, knight_stand9)
	ai_stand(self)
end

function knight_stand9(self)
	animate(self, 8, knight_stand1)
	ai_stand(self)
end

function knight_walk1(self)
	animate(self, 53, knight_walk2)
	if random() < 0.2 then
		sound(self, CHAN_VOICE, "knight/idle.wav", 1, ATTN_IDLE)
	end
	ai_walk(self, 3)
end

function knight_walk2(self)
	animate(self, 54, knight_walk3)
	ai_walk(self, 2)
end

function knight_walk3(self)
	animate(self, 55, knight_walk4)
	ai_walk(self, 3)
end

function knight_walk4(self)
	animate(self, 56, knight_walk5)
	ai_walk(self, 4)
end

function knight_walk5(self)
	animate(self, 57, knight_walk6)
	ai_walk(self, 3)
end

function knight_walk6(self)
	animate(self, 58, knight_walk7)
	ai_walk(self, 3)
end

function knight_walk7(self)
	animate(self, 59, knight_walk8)
	ai_walk(self, 3)
end

function knight_walk8(self)
	animate(self, 60, knight_walk9)
	ai_walk(self, 4)
end

function knight_walk9(self)
	animate(self, 61, knight_walk10)
	ai_walk(self, 3)
end

function knight_walk10(self)
	animate(self, 62, knight_walk11)
	ai_walk(self, 3)
end

function knight_walk11(self)
	animate(self, 63, knight_walk12)
	ai_walk(self, 2)
end

function knight_walk12(self)
	animate(self, 64, knight_walk13)
	ai_walk(self, 3)
end

function knight_walk13(self)
	animate(self, 65, knight_walk14)
	ai_walk(self, 4)
end

function knight_walk14(self)
	animate(self, 66, knight_walk1)
	ai_walk(self, 3)
end

function knight_run1(self)
	animate(self, 9, knight_run2)
	if random() < 0.2 then
		sound(self, CHAN_VOICE, "knight/idle.wav", 1, ATTN_IDLE)
	end
	ai_run(self, 16)
end

function knight_run2(self)
	animate(self, 10, knight_run3)
	ai_run(self, 20)
end

function knight_run3(self)
	animate(self, 11, knight_run4)
	ai_run(self, 13)
end

function knight_run4(self)
	animate(self, 12, knight_run5)
	ai_run(self, 7)
end

function knight_run5(self)
	animate(self, 13, knight_run6)
	ai_run(self, 16)
end

function knight_run6(self)
	animate(self, 14, knight_run7)
	ai_run(self, 20)
end

function knight_run7(self)
	animate(self, 15, knight_run8)
	ai_run(self, 14)
end

function knight_run8(self)
	animate(self, 16, knight_run1)
	ai_run(self, 6)
end

function knight_runatk1(self)
	animate(self, 17, knight_runatk2)
	if random() > 0.5 then
		sound(self, CHAN_WEAPON, "knight/sword2.wav", 1, ATTN_NORM)
	else
		sound(self, CHAN_WEAPON, "knight/sword1.wav", 1, ATTN_NORM)
	end
	ai_charge(self, 20)
end

function knight_runatk2(self)
	animate(self, 18, knight_runatk3)
	ai_charge_side(self)
end

function knight_runatk3(self)
	animate(self, 19, knight_runatk4)
	ai_charge_side(self)
end

function knight_runatk4(self)
	animate(self, 20, knight_runatk5)
	ai_charge_side(self)
end

function knight_runatk5(self)
	animate(self, 21, knight_runatk6)
	ai_melee_side(self)
end

function knight_runatk6(self)
	animate(self, 22, knight_runatk7)
	ai_melee_side(self)
end

function knight_runatk7(self)
	animate(self, 23, knight_runatk8)
	ai_melee_side(self)
end

function knight_runatk8(self)
	animate(self, 24, knight_runatk9)
	ai_melee_side(self)
end

function knight_runatk9(self)
	animate(self, 25, knight_runatk10)
	ai_melee_side(self)
end

function knight_runatk10(self)
	animate(self, 26, knight_runatk11)
	ai_charge_side(self)
end

function knight_runatk11(self)
	animate(self, 27, knight_run1)
	ai_charge(self, 10)
end

function knight_atk1(self)
	animate(self, 43, knight_atk2)
	sound(self, CHAN_WEAPON, "knight/sword1.wav", 1, ATTN_NORM)
	ai_charge(self, 0)
end

function knight_atk2(self)
	animate(self, 44, knight_atk3)
	ai_charge(self, 7)
end

function knight_atk3(self)
	animate(self, 45, knight_atk4)
	ai_charge(self, 4)
end

function knight_atk4(self)
	animate(self, 46, knight_atk5)
	ai_charge(self, 0)
end

function knight_atk5(self)
	animate(self, 47, knight_atk6)
	ai_charge(self, 3)
end

function knight_atk6(self)
	animate(self, 48, knight_atk7)
	ai_charge(self, 4)
	ai_melee(self)
end

function knight_atk7(self)
	animate(self, 49, knight_atk8)
	ai_charge(self, 1)
	ai_melee(self)
end

function knight_atk8(self)
	animate(self, 50, knight_atk9)
	ai_charge(self, 3)
	ai_melee(self)
end

function knight_atk9(self)
	animate(self, 51, knight_atk10)
	ai_charge(self, 1)
end

function knight_atk10(self)
	animate(self, 52, knight_run1)
	ai_charge(self, 5)
end

function knight_pain1(self)
	animate(self, 28, knight_pain2)
end

function knight_pain2(self)
	animate(self, 29, knight_pain3)
end

function knight_pain3(self)
	animate(self, 30, knight_run1)
end

function knight_painb1(self)
	animate(self, 31, knight_painb2)
	ai_painforward(self, 0)
end

function knight_painb2(self)
	animate(self, 32, knight_painb3)
	ai_painforward(self, 3)
end

function knight_painb3(self)
	animate(self, 33, knight_painb4)
end

function knight_painb4(self)
	animate(self, 34, knight_painb5)
end

function knight_painb5(self)
	animate(self, 35, knight_painb6)
	ai_painforward(self, 2)
end

function knight_painb6(self)
	animate(self, 36, knight_painb7)
	ai_painforward(self, 4)
end

function knight_painb7(self)
	animate(self, 37, knight_painb8)
	ai_painforward(self, 2)
end

function knight_painb8(self)
	animate(self, 38, knight_painb9)
	ai_painforward(self, 5)
end

function knight_painb9(self)
	animate(self, 39, knight_painb10)
	ai_painforward(self, 5)
end

function knight_painb10(self)
	animate(self, 40, knight_painb11)
	ai_painforward(self, 0)
end

function knight_painb11(self)
	animate(self, 41, knight_run1)
end

function knight_pain(self, attacker, damage)
	local r
	if self.pain_finished > time() then
		return
	end
	r = random()
	sound(self, CHAN_VOICE, "knight/khurt.wav", 1, ATTN_NORM)
	if r < 0.85 then
		knight_pain1(self)
		self.pain_finished = time() + 1
	else
		knight_painb1(self)
		self.pain_finished = time() + 1
	end
end

function knight_bow1(self)
	animate(self, 67, knight_bow2)
	ai_turn(self)
end

function knight_bow2(self)
	animate(self, 68, knight_bow3)
	ai_turn(self)
end

function knight_bow3(self)
	animate(self, 69, knight_bow4)
	ai_turn(self)
end

function knight_bow4(self)
	animate(self, 70, knight_bow5)
	ai_turn(self)
end

function knight_bow5(self)
	animate(self, 71, knight_bow5)
	ai_turn(self)
end

function knight_bow6(self)
	animate(self, 70, knight_bow7)
	ai_turn(self)
end

function knight_bow7(self)
	animate(self, 69, knight_bow8)
	ai_turn(self)
end

function knight_bow8(self)
	animate(self, 68, knight_bow9)
	ai_turn(self)
end

function knight_bow9(self)
	animate(self, 67, knight_bow10)
	ai_turn(self)
end

function knight_bow10(self)
	animate(self, 53, knight_walk1)
	ai_turn(self)
end

function knight_die1(self)
	animate(self, 76, knight_die2)
end

function knight_die2(self)
	animate(self, 77, knight_die3)
end

function knight_die3(self)
	animate(self, 78, knight_die4)
	self.solid = SOLID_NOT
end

function knight_die4(self)
	animate(self, 79, knight_die5)
end

function knight_die5(self)
	animate(self, 80, knight_die6)
end

function knight_die6(self)
	animate(self, 81, knight_die7)
end

function knight_die7(self)
	animate(self, 82, knight_die8)
end

function knight_die8(self)
	animate(self, 83, knight_die9)
end

function knight_die9(self)
	animate(self, 84, knight_die10)
end

function knight_die10(self)
	animate(self, 85, knight_die10)
end

function knight_dieb1(self)
	animate(self, 86, knight_dieb2)
end

function knight_dieb2(self)
	animate(self, 87, knight_dieb3)
end

function knight_dieb3(self)
	animate(self, 88, knight_dieb4)
	self.solid = SOLID_NOT
end

function knight_dieb4(self)
	animate(self, 89, knight_dieb5)
end

function knight_dieb5(self)
	animate(self, 90, knight_dieb6)
end

function knight_dieb6(self)
	animate(self, 91, knight_dieb7)
end

function knight_dieb7(self)
	animate(self, 92, knight_dieb8)
end

function knight_dieb8(self)
	animate(self, 93, knight_dieb9)
end

function knight_dieb9(self)
	animate(self, 94, knight_dieb10)
end

function knight_dieb10(self)
	animate(self, 95, knight_dieb11)
end

function knight_dieb11(self)
	animate(self, 96, knight_dieb11)
end

function knight_die(self)
	-- check for gib
	if self.health < -40 then
		sound(self, CHAN_VOICE, "player/udeath.wav", 1, ATTN_NORM)
		ThrowHead(self, "progs/h_knight.mdl", self.health)
		ThrowGib(self, "progs/gib1.mdl", self.health)
		ThrowGib(self, "progs/gib2.mdl", self.health)
		ThrowGib(self, "progs/gib3.mdl", self.health)
		return
	end
	sound(self, CHAN_VOICE, "knight/kdeath.wav", 1, ATTN_NORM)
	if random() < 0.5 then
		knight_die1(self)
	else
		knight_dieb1(self)
	end
end

function monster_knight(self)
	if deathmatch ~= 0 then
		remove(self)
		return
	end
	self.solid = SOLID_SLIDEBOX
	self.movetype = MOVETYPE_STEP
	setmodel(self, "progs/knight.mdl")
	setsize(self, vector.create(-16, -16, -24), vector.create(16, 16, 40))
	self.health = 75
	self.th_stand = knight_stand1
	self.th_walk = knight_walk1
	self.th_run = knight_run1
	self.th_melee = knight_atk1
	self.th_pain = knight_pain
	self.th_die = knight_die
	walkmonster_start(self)
end
