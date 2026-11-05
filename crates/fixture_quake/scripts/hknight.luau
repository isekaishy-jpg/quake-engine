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
-- Ported from hknight.qc.

function hknight_shot(self, offset)
	local offang = vector.zero
	local vec = vector.zero
	local org
	offang = vectoangles(ent(self.enemy).origin - self.origin)
	offang = with_y(offang, offang.y + offset * 6)
	makevectors(offang)
	org = self.origin + self.mins + self.size * 0.5 + v_forward * 20
	-- set missile speed
	vec = normalize(v_forward)
	vec = with_z(vec, 0 - vec.z + (random() - 0.5) * 0.1)
	launch_spike(self, org, vec, 300)
	newmis.classname = "knightspike"
	setmodel(newmis, "progs/k_spike.mdl")
	setsize(newmis, VEC_ORIGIN, VEC_ORIGIN)
	sound(self, CHAN_WEAPON, "hknight/attack1.wav", 1, ATTN_NORM)
end

function CheckForCharge(self)
	-- check for mad charge
	if not enemy_vis then
		return
	end
	if time() < self.attack_finished then
		return
	end
	if math.abs(self.origin.z - ent(self.enemy).origin.z) > 20 then
		return
	end
	if vector.magnitude(self.origin - ent(self.enemy).origin) < 80 then
		return
	end
	SUB_AttackFinished(self, 2)
	hknight_char_a1(self)
end

function CheckContinueCharge(self)
	if time() > self.attack_finished then
		SUB_AttackFinished(self, 3)
		hknight_run1(self)
		return
		-- done charging
	end
	if random() > 0.5 then
		sound(self, CHAN_WEAPON, "knight/sword2.wav", 1, ATTN_NORM)
	else
		sound(self, CHAN_WEAPON, "knight/sword1.wav", 1, ATTN_NORM)
	end
end

function hknight_stand1(self)
	animate(self, 0, hknight_stand2)
	ai_stand(self)
end

function hknight_stand2(self)
	animate(self, 1, hknight_stand3)
	ai_stand(self)
end

function hknight_stand3(self)
	animate(self, 2, hknight_stand4)
	ai_stand(self)
end

function hknight_stand4(self)
	animate(self, 3, hknight_stand5)
	ai_stand(self)
end

function hknight_stand5(self)
	animate(self, 4, hknight_stand6)
	ai_stand(self)
end

function hknight_stand6(self)
	animate(self, 5, hknight_stand7)
	ai_stand(self)
end

function hknight_stand7(self)
	animate(self, 6, hknight_stand8)
	ai_stand(self)
end

function hknight_stand8(self)
	animate(self, 7, hknight_stand9)
	ai_stand(self)
end

function hknight_stand9(self)
	animate(self, 8, hknight_stand1)
	ai_stand(self)
end

function hknight_walk1(self)
	animate(self, 9, hknight_walk2)
	hk_idle_sound(self)
	ai_walk(self, 2)
end

function hknight_walk2(self)
	animate(self, 10, hknight_walk3)
	ai_walk(self, 5)
end

function hknight_walk3(self)
	animate(self, 11, hknight_walk4)
	ai_walk(self, 5)
end

function hknight_walk4(self)
	animate(self, 12, hknight_walk5)
	ai_walk(self, 4)
end

function hknight_walk5(self)
	animate(self, 13, hknight_walk6)
	ai_walk(self, 4)
end

function hknight_walk6(self)
	animate(self, 14, hknight_walk7)
	ai_walk(self, 2)
end

function hknight_walk7(self)
	animate(self, 15, hknight_walk8)
	ai_walk(self, 2)
end

function hknight_walk8(self)
	animate(self, 16, hknight_walk9)
	ai_walk(self, 3)
end

function hknight_walk9(self)
	animate(self, 17, hknight_walk10)
	ai_walk(self, 3)
end

function hknight_walk10(self)
	animate(self, 18, hknight_walk11)
	ai_walk(self, 4)
end

function hknight_walk11(self)
	animate(self, 19, hknight_walk12)
	ai_walk(self, 3)
end

function hknight_walk12(self)
	animate(self, 20, hknight_walk13)
	ai_walk(self, 4)
end

function hknight_walk13(self)
	animate(self, 21, hknight_walk14)
	ai_walk(self, 6)
end

function hknight_walk14(self)
	animate(self, 22, hknight_walk15)
	ai_walk(self, 2)
end

function hknight_walk15(self)
	animate(self, 23, hknight_walk16)
	ai_walk(self, 2)
end

function hknight_walk16(self)
	animate(self, 24, hknight_walk17)
	ai_walk(self, 4)
end

function hknight_walk17(self)
	animate(self, 25, hknight_walk18)
	ai_walk(self, 3)
end

function hknight_walk18(self)
	animate(self, 26, hknight_walk19)
	ai_walk(self, 3)
end

function hknight_walk19(self)
	animate(self, 27, hknight_walk20)
	ai_walk(self, 3)
end

function hknight_walk20(self)
	animate(self, 28, hknight_walk1)
	ai_walk(self, 2)
end

function hknight_run1(self)
	animate(self, 29, hknight_run2)
	hk_idle_sound(self)
	ai_run(self, 20)
	CheckForCharge(self)
end

function hknight_run2(self)
	animate(self, 30, hknight_run3)
	ai_run(self, 25)
end

function hknight_run3(self)
	animate(self, 31, hknight_run4)
	ai_run(self, 18)
end

function hknight_run4(self)
	animate(self, 32, hknight_run5)
	ai_run(self, 16)
end

function hknight_run5(self)
	animate(self, 33, hknight_run6)
	ai_run(self, 14)
end

function hknight_run6(self)
	animate(self, 34, hknight_run7)
	ai_run(self, 25)
end

function hknight_run7(self)
	animate(self, 35, hknight_run8)
	ai_run(self, 21)
end

function hknight_run8(self)
	animate(self, 36, hknight_run1)
	ai_run(self, 13)
end

function hknight_pain1(self)
	animate(self, 37, hknight_pain2)
	sound(self, CHAN_VOICE, "hknight/pain1.wav", 1, ATTN_NORM)
end

function hknight_pain2(self)
	animate(self, 38, hknight_pain3)
end

function hknight_pain3(self)
	animate(self, 39, hknight_pain4)
end

function hknight_pain4(self)
	animate(self, 40, hknight_pain5)
end

function hknight_pain5(self)
	animate(self, 41, hknight_run1)
end

function hknight_die1(self)
	animate(self, 42, hknight_die2)
	ai_forward(self, 10)
end

function hknight_die2(self)
	animate(self, 43, hknight_die3)
	ai_forward(self, 8)
end

function hknight_die3(self)
	animate(self, 44, hknight_die4)
	self.solid = SOLID_NOT
	ai_forward(self, 7)
end

function hknight_die4(self)
	animate(self, 45, hknight_die5)
end

function hknight_die5(self)
	animate(self, 46, hknight_die6)
end

function hknight_die6(self)
	animate(self, 47, hknight_die7)
end

function hknight_die7(self)
	animate(self, 48, hknight_die8)
end

function hknight_die8(self)
	animate(self, 49, hknight_die9)
	ai_forward(self, 10)
end

function hknight_die9(self)
	animate(self, 50, hknight_die10)
	ai_forward(self, 11)
end

function hknight_die10(self)
	animate(self, 51, hknight_die11)
end

function hknight_die11(self)
	animate(self, 52, hknight_die12)
end

function hknight_die12(self)
	animate(self, 53, hknight_die12)
end

function hknight_dieb1(self)
	animate(self, 54, hknight_dieb2)
end

function hknight_dieb2(self)
	animate(self, 55, hknight_dieb3)
end

function hknight_dieb3(self)
	animate(self, 56, hknight_dieb4)
	self.solid = SOLID_NOT
end

function hknight_dieb4(self)
	animate(self, 57, hknight_dieb5)
end

function hknight_dieb5(self)
	animate(self, 58, hknight_dieb6)
end

function hknight_dieb6(self)
	animate(self, 59, hknight_dieb7)
end

function hknight_dieb7(self)
	animate(self, 60, hknight_dieb8)
end

function hknight_dieb8(self)
	animate(self, 61, hknight_dieb9)
end

function hknight_dieb9(self)
	animate(self, 62, hknight_dieb9)
end

function hknight_die(self)
	-- check for gib
	if self.health < -40 then
		sound(self, CHAN_VOICE, "player/udeath.wav", 1, ATTN_NORM)
		ThrowHead(self, "progs/h_hellkn.mdl", self.health)
		ThrowGib(self, "progs/gib1.mdl", self.health)
		ThrowGib(self, "progs/gib2.mdl", self.health)
		ThrowGib(self, "progs/gib3.mdl", self.health)
		return
	end
	sound(self, CHAN_VOICE, "hknight/death1.wav", 1, ATTN_NORM)
	if random() > 0.5 then
		hknight_die1(self)
	else
		hknight_dieb1(self)
	end
end

function hknight_magica1(self)
	animate(self, 79, hknight_magica2)
	ai_face(self)
end

function hknight_magica2(self)
	animate(self, 80, hknight_magica3)
	ai_face(self)
end

function hknight_magica3(self)
	animate(self, 81, hknight_magica4)
	ai_face(self)
end

function hknight_magica4(self)
	animate(self, 82, hknight_magica5)
	ai_face(self)
end

function hknight_magica5(self)
	animate(self, 83, hknight_magica6)
	ai_face(self)
end

function hknight_magica6(self)
	animate(self, 84, hknight_magica7)
	ai_face(self)
end

function hknight_magica7(self)
	animate(self, 85, hknight_magica8)
	hknight_shot(self, -2)
end

function hknight_magica8(self)
	animate(self, 86, hknight_magica9)
	hknight_shot(self, -1)
end

function hknight_magica9(self)
	animate(self, 87, hknight_magica10)
	hknight_shot(self, 0)
end

function hknight_magica10(self)
	animate(self, 88, hknight_magica11)
	hknight_shot(self, 1)
end

function hknight_magica11(self)
	animate(self, 89, hknight_magica12)
	hknight_shot(self, 2)
end

function hknight_magica12(self)
	animate(self, 90, hknight_magica13)
	hknight_shot(self, 3)
end

function hknight_magica13(self)
	animate(self, 91, hknight_magica14)
	ai_face(self)
end

function hknight_magica14(self)
	animate(self, 92, hknight_run1)
	ai_face(self)
end

function hknight_magicb1(self)
	animate(self, 93, hknight_magicb2)
	ai_face(self)
end

function hknight_magicb2(self)
	animate(self, 94, hknight_magicb3)
	ai_face(self)
end

function hknight_magicb3(self)
	animate(self, 95, hknight_magicb4)
	ai_face(self)
end

function hknight_magicb4(self)
	animate(self, 96, hknight_magicb5)
	ai_face(self)
end

function hknight_magicb5(self)
	animate(self, 97, hknight_magicb6)
	ai_face(self)
end

function hknight_magicb6(self)
	animate(self, 98, hknight_magicb7)
	ai_face(self)
end

function hknight_magicb7(self)
	animate(self, 99, hknight_magicb8)
	hknight_shot(self, -2)
end

function hknight_magicb8(self)
	animate(self, 100, hknight_magicb9)
	hknight_shot(self, -1)
end

function hknight_magicb9(self)
	animate(self, 101, hknight_magicb10)
	hknight_shot(self, 0)
end

function hknight_magicb10(self)
	animate(self, 102, hknight_magicb11)
	hknight_shot(self, 1)
end

function hknight_magicb11(self)
	animate(self, 103, hknight_magicb12)
	hknight_shot(self, 2)
end

function hknight_magicb12(self)
	animate(self, 104, hknight_magicb13)
	hknight_shot(self, 3)
end

function hknight_magicb13(self)
	animate(self, 105, hknight_run1)
	ai_face(self)
end

function hknight_magicc1(self)
	animate(self, 155, hknight_magicc2)
	ai_face(self)
end

function hknight_magicc2(self)
	animate(self, 156, hknight_magicc3)
	ai_face(self)
end

function hknight_magicc3(self)
	animate(self, 157, hknight_magicc4)
	ai_face(self)
end

function hknight_magicc4(self)
	animate(self, 158, hknight_magicc5)
	ai_face(self)
end

function hknight_magicc5(self)
	animate(self, 159, hknight_magicc6)
	ai_face(self)
end

function hknight_magicc6(self)
	animate(self, 160, hknight_magicc7)
	hknight_shot(self, -2)
end

function hknight_magicc7(self)
	animate(self, 161, hknight_magicc8)
	hknight_shot(self, -1)
end

function hknight_magicc8(self)
	animate(self, 162, hknight_magicc9)
	hknight_shot(self, 0)
end

function hknight_magicc9(self)
	animate(self, 163, hknight_magicc10)
	hknight_shot(self, 1)
end

function hknight_magicc10(self)
	animate(self, 164, hknight_magicc11)
	hknight_shot(self, 2)
end

function hknight_magicc11(self)
	animate(self, 165, hknight_run1)
	hknight_shot(self, 3)
end

function hknight_char_a1(self)
	animate(self, 63, hknight_char_a2)
	ai_charge(self, 20)
end

function hknight_char_a2(self)
	animate(self, 64, hknight_char_a3)
	ai_charge(self, 25)
end

function hknight_char_a3(self)
	animate(self, 65, hknight_char_a4)
	ai_charge(self, 18)
end

function hknight_char_a4(self)
	animate(self, 66, hknight_char_a5)
	ai_charge(self, 16)
end

function hknight_char_a5(self)
	animate(self, 67, hknight_char_a6)
	ai_charge(self, 14)
end

function hknight_char_a6(self)
	animate(self, 68, hknight_char_a7)
	ai_charge(self, 20)
	ai_melee(self)
end

function hknight_char_a7(self)
	animate(self, 69, hknight_char_a8)
	ai_charge(self, 21)
	ai_melee(self)
end

function hknight_char_a8(self)
	animate(self, 70, hknight_char_a9)
	ai_charge(self, 13)
	ai_melee(self)
end

function hknight_char_a9(self)
	animate(self, 71, hknight_char_a10)
	ai_charge(self, 20)
	ai_melee(self)
end

function hknight_char_a10(self)
	animate(self, 72, hknight_char_a11)
	ai_charge(self, 20)
	ai_melee(self)
end

function hknight_char_a11(self)
	animate(self, 73, hknight_char_a12)
	ai_charge(self, 18)
	ai_melee(self)
end

function hknight_char_a12(self)
	animate(self, 74, hknight_char_a13)
	ai_charge(self, 16)
end

function hknight_char_a13(self)
	animate(self, 75, hknight_char_a14)
	ai_charge(self, 14)
end

function hknight_char_a14(self)
	animate(self, 76, hknight_char_a15)
	ai_charge(self, 25)
end

function hknight_char_a15(self)
	animate(self, 77, hknight_char_a16)
	ai_charge(self, 21)
end

function hknight_char_a16(self)
	animate(self, 78, hknight_run1)
	ai_charge(self, 13)
end

function hknight_char_b1(self)
	animate(self, 106, hknight_char_b2)
	CheckContinueCharge(self)
	ai_charge(self, 23)
	ai_melee(self)
end

function hknight_char_b2(self)
	animate(self, 107, hknight_char_b3)
	ai_charge(self, 17)
	ai_melee(self)
end

function hknight_char_b3(self)
	animate(self, 108, hknight_char_b4)
	ai_charge(self, 12)
	ai_melee(self)
end

function hknight_char_b4(self)
	animate(self, 109, hknight_char_b5)
	ai_charge(self, 22)
	ai_melee(self)
end

function hknight_char_b5(self)
	animate(self, 110, hknight_char_b6)
	ai_charge(self, 18)
	ai_melee(self)
end

function hknight_char_b6(self)
	animate(self, 111, hknight_char_b1)
	ai_charge(self, 8)
	ai_melee(self)
end

function hknight_slice1(self)
	animate(self, 112, hknight_slice2)
	ai_charge(self, 9)
end

function hknight_slice2(self)
	animate(self, 113, hknight_slice3)
	ai_charge(self, 6)
end

function hknight_slice3(self)
	animate(self, 114, hknight_slice4)
	ai_charge(self, 13)
end

function hknight_slice4(self)
	animate(self, 115, hknight_slice5)
	ai_charge(self, 4)
end

function hknight_slice5(self)
	animate(self, 116, hknight_slice6)
	ai_charge(self, 7)
	ai_melee(self)
end

function hknight_slice6(self)
	animate(self, 117, hknight_slice7)
	ai_charge(self, 15)
	ai_melee(self)
end

function hknight_slice7(self)
	animate(self, 118, hknight_slice8)
	ai_charge(self, 8)
	ai_melee(self)
end

function hknight_slice8(self)
	animate(self, 119, hknight_slice9)
	ai_charge(self, 2)
	ai_melee(self)
end

function hknight_slice9(self)
	animate(self, 120, hknight_slice10)
	ai_melee(self)
end

function hknight_slice10(self)
	animate(self, 121, hknight_run1)
	ai_charge(self, 3)
end

function hknight_smash1(self)
	animate(self, 122, hknight_smash2)
	ai_charge(self, 1)
end

function hknight_smash2(self)
	animate(self, 123, hknight_smash3)
	ai_charge(self, 13)
end

function hknight_smash3(self)
	animate(self, 124, hknight_smash4)
	ai_charge(self, 9)
end

function hknight_smash4(self)
	animate(self, 125, hknight_smash5)
	ai_charge(self, 11)
end

function hknight_smash5(self)
	animate(self, 126, hknight_smash6)
	ai_charge(self, 10)
	ai_melee(self)
end

function hknight_smash6(self)
	animate(self, 127, hknight_smash7)
	ai_charge(self, 7)
	ai_melee(self)
end

function hknight_smash7(self)
	animate(self, 128, hknight_smash8)
	ai_charge(self, 12)
	ai_melee(self)
end

function hknight_smash8(self)
	animate(self, 129, hknight_smash9)
	ai_charge(self, 2)
	ai_melee(self)
end

function hknight_smash9(self)
	animate(self, 130, hknight_smash10)
	ai_charge(self, 3)
	ai_melee(self)
end

function hknight_smash10(self)
	animate(self, 131, hknight_smash11)
	ai_charge(self, 0)
end

function hknight_smash11(self)
	animate(self, 132, hknight_run1)
	ai_charge(self, 0)
end

function hknight_watk1(self)
	animate(self, 133, hknight_watk2)
	ai_charge(self, 2)
end

function hknight_watk2(self)
	animate(self, 134, hknight_watk3)
	ai_charge(self, 0)
end

function hknight_watk3(self)
	animate(self, 135, hknight_watk4)
	ai_charge(self, 0)
end

function hknight_watk4(self)
	animate(self, 136, hknight_watk5)
	ai_melee(self)
end

function hknight_watk5(self)
	animate(self, 137, hknight_watk6)
	ai_melee(self)
end

function hknight_watk6(self)
	animate(self, 138, hknight_watk7)
	ai_melee(self)
end

function hknight_watk7(self)
	animate(self, 139, hknight_watk8)
	ai_charge(self, 1)
end

function hknight_watk8(self)
	animate(self, 140, hknight_watk9)
	ai_charge(self, 4)
end

function hknight_watk9(self)
	animate(self, 141, hknight_watk10)
	ai_charge(self, 5)
end

function hknight_watk10(self)
	animate(self, 142, hknight_watk11)
	ai_charge(self, 3)
	ai_melee(self)
end

function hknight_watk11(self)
	animate(self, 143, hknight_watk12)
	ai_charge(self, 2)
	ai_melee(self)
end

function hknight_watk12(self)
	animate(self, 144, hknight_watk13)
	ai_charge(self, 2)
	ai_melee(self)
end

function hknight_watk13(self)
	animate(self, 145, hknight_watk14)
	ai_charge(self, 0)
end

function hknight_watk14(self)
	animate(self, 146, hknight_watk15)
	ai_charge(self, 0)
end

function hknight_watk15(self)
	animate(self, 147, hknight_watk16)
	ai_charge(self, 0)
end

function hknight_watk16(self)
	animate(self, 148, hknight_watk17)
	ai_charge(self, 1)
end

function hknight_watk17(self)
	animate(self, 149, hknight_watk18)
	ai_charge(self, 1)
	ai_melee(self)
end

function hknight_watk18(self)
	animate(self, 150, hknight_watk19)
	ai_charge(self, 3)
	ai_melee(self)
end

function hknight_watk19(self)
	animate(self, 151, hknight_watk20)
	ai_charge(self, 4)
	ai_melee(self)
end

function hknight_watk20(self)
	animate(self, 152, hknight_watk21)
	ai_charge(self, 6)
end

function hknight_watk21(self)
	animate(self, 153, hknight_watk22)
	ai_charge(self, 7)
end

function hknight_watk22(self)
	animate(self, 154, hknight_run1)
	ai_charge(self, 3)
end

function hk_idle_sound(self)
	if random() < 0.2 then
		sound(self, CHAN_VOICE, "hknight/idle.wav", 1, ATTN_NORM)
	end
end

function hknight_pain(self, attacker, damage)
	if self.pain_finished > time() then
		return
	end
	sound(self, CHAN_VOICE, "hknight/pain1.wav", 1, ATTN_NORM)
	if time() - self.pain_finished > 5 then
		-- allways go into pain frame if it has been a while
		hknight_pain1(self)
		self.pain_finished = time() + 1
		return
	end
	if random() * 30 > damage then
		return
	end
	self.pain_finished = time() + 1
	hknight_pain1(self)
end

hknight_type = 0

function hknight_melee(self)
	hknight_type = hknight_type + 1
	sound(self, CHAN_WEAPON, "hknight/slash1.wav", 1, ATTN_NORM)
	if hknight_type == 1 then
		hknight_slice1(self)
	elseif hknight_type == 2 then
		hknight_smash1(self)
	elseif hknight_type == 3 then
		hknight_watk1(self)
		hknight_type = 0
	end
end

function monster_hell_knight(self)
	if deathmatch ~= 0 then
		remove(self)
		return
	end
	-- used by C code, so don't sound2
	self.solid = SOLID_SLIDEBOX
	self.movetype = MOVETYPE_STEP
	setmodel(self, "progs/hknight.mdl")
	setsize(self, vector.create(-16, -16, -24), vector.create(16, 16, 40))
	self.health = 250
	self.th_stand = hknight_stand1
	self.th_walk = hknight_walk1
	self.th_run = hknight_run1
	self.th_melee = hknight_melee
	self.th_missile = hknight_magicc1
	self.th_pain = hknight_pain
	self.th_die = hknight_die
	walkmonster_start(self)
end
