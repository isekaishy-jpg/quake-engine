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
-- Ported from fish.qc.

function f_stand1(self)
	animate(self, 39, f_stand2)
	ai_stand(self)
end

function f_stand2(self)
	animate(self, 40, f_stand3)
	ai_stand(self)
end

function f_stand3(self)
	animate(self, 41, f_stand4)
	ai_stand(self)
end

function f_stand4(self)
	animate(self, 42, f_stand5)
	ai_stand(self)
end

function f_stand5(self)
	animate(self, 43, f_stand6)
	ai_stand(self)
end

function f_stand6(self)
	animate(self, 44, f_stand7)
	ai_stand(self)
end

function f_stand7(self)
	animate(self, 45, f_stand8)
	ai_stand(self)
end

function f_stand8(self)
	animate(self, 46, f_stand9)
	ai_stand(self)
end

function f_stand9(self)
	animate(self, 47, f_stand10)
	ai_stand(self)
end

function f_stand10(self)
	animate(self, 48, f_stand11)
	ai_stand(self)
end

function f_stand11(self)
	animate(self, 49, f_stand12)
	ai_stand(self)
end

function f_stand12(self)
	animate(self, 50, f_stand13)
	ai_stand(self)
end

function f_stand13(self)
	animate(self, 51, f_stand14)
	ai_stand(self)
end

function f_stand14(self)
	animate(self, 52, f_stand15)
	ai_stand(self)
end

function f_stand15(self)
	animate(self, 53, f_stand16)
	ai_stand(self)
end

function f_stand16(self)
	animate(self, 54, f_stand17)
	ai_stand(self)
end

function f_stand17(self)
	animate(self, 55, f_stand18)
	ai_stand(self)
end

function f_stand18(self)
	animate(self, 56, f_stand1)
	ai_stand(self)
end

function f_walk1(self)
	animate(self, 39, f_walk2)
	ai_walk(self, 8)
end

function f_walk2(self)
	animate(self, 40, f_walk3)
	ai_walk(self, 8)
end

function f_walk3(self)
	animate(self, 41, f_walk4)
	ai_walk(self, 8)
end

function f_walk4(self)
	animate(self, 42, f_walk5)
	ai_walk(self, 8)
end

function f_walk5(self)
	animate(self, 43, f_walk6)
	ai_walk(self, 8)
end

function f_walk6(self)
	animate(self, 44, f_walk7)
	ai_walk(self, 8)
end

function f_walk7(self)
	animate(self, 45, f_walk8)
	ai_walk(self, 8)
end

function f_walk8(self)
	animate(self, 46, f_walk9)
	ai_walk(self, 8)
end

function f_walk9(self)
	animate(self, 47, f_walk10)
	ai_walk(self, 8)
end

function f_walk10(self)
	animate(self, 48, f_walk11)
	ai_walk(self, 8)
end

function f_walk11(self)
	animate(self, 49, f_walk12)
	ai_walk(self, 8)
end

function f_walk12(self)
	animate(self, 50, f_walk13)
	ai_walk(self, 8)
end

function f_walk13(self)
	animate(self, 51, f_walk14)
	ai_walk(self, 8)
end

function f_walk14(self)
	animate(self, 52, f_walk15)
	ai_walk(self, 8)
end

function f_walk15(self)
	animate(self, 53, f_walk16)
	ai_walk(self, 8)
end

function f_walk16(self)
	animate(self, 54, f_walk17)
	ai_walk(self, 8)
end

function f_walk17(self)
	animate(self, 55, f_walk18)
	ai_walk(self, 8)
end

function f_walk18(self)
	animate(self, 56, f_walk1)
	ai_walk(self, 8)
end

function f_run1(self)
	animate(self, 39, f_run2)
	ai_run(self, 12)
	if random() < 0.5 then
		sound(self, CHAN_VOICE, "fish/idle.wav", 1, ATTN_NORM)
	end
end

function f_run2(self)
	animate(self, 41, f_run3)
	ai_run(self, 12)
end

function f_run3(self)
	animate(self, 43, f_run4)
	ai_run(self, 12)
end

function f_run4(self)
	animate(self, 45, f_run5)
	ai_run(self, 12)
end

function f_run5(self)
	animate(self, 47, f_run6)
	ai_run(self, 12)
end

function f_run6(self)
	animate(self, 49, f_run7)
	ai_run(self, 12)
end

function f_run7(self)
	animate(self, 51, f_run8)
	ai_run(self, 12)
end

function f_run8(self)
	animate(self, 53, f_run9)
	ai_run(self, 12)
end

function f_run9(self)
	animate(self, 55, f_run1)
	ai_run(self, 12)
end

function fish_melee(self)
	local delta, ldmg
	if not exists(self.enemy) then
		return
	end
	delta = ent(self.enemy).origin - self.origin
	if vector.magnitude(delta) > 60 then
		return
	end
	sound(self, CHAN_VOICE, "fish/bite.wav", 1, ATTN_NORM)
	ldmg = (random() + random()) * 3
	T_Damage(self.enemy, self, self, ldmg)
end

function f_attack1(self)
	animate(self, 0, f_attack2)
	ai_charge(self, 10)
end

function f_attack2(self)
	animate(self, 1, f_attack3)
	ai_charge(self, 10)
end

function f_attack3(self)
	animate(self, 2, f_attack4)
	fish_melee(self)
end

function f_attack4(self)
	animate(self, 3, f_attack5)
	ai_charge(self, 10)
end

function f_attack5(self)
	animate(self, 4, f_attack6)
	ai_charge(self, 10)
end

function f_attack6(self)
	animate(self, 5, f_attack7)
	ai_charge(self, 10)
end

function f_attack7(self)
	animate(self, 6, f_attack8)
	ai_charge(self, 10)
end

function f_attack8(self)
	animate(self, 7, f_attack9)
	ai_charge(self, 10)
end

function f_attack9(self)
	animate(self, 8, f_attack10)
	fish_melee(self)
end

function f_attack10(self)
	animate(self, 9, f_attack11)
	ai_charge(self, 10)
end

function f_attack11(self)
	animate(self, 10, f_attack12)
	ai_charge(self, 10)
end

function f_attack12(self)
	animate(self, 11, f_attack13)
	ai_charge(self, 10)
end

function f_attack13(self)
	animate(self, 12, f_attack14)
	ai_charge(self, 10)
end

function f_attack14(self)
	animate(self, 13, f_attack15)
	ai_charge(self, 10)
end

function f_attack15(self)
	animate(self, 14, f_attack16)
	fish_melee(self)
end

function f_attack16(self)
	animate(self, 15, f_attack17)
	ai_charge(self, 10)
end

function f_attack17(self)
	animate(self, 16, f_attack18)
	ai_charge(self, 10)
end

function f_attack18(self)
	animate(self, 17, f_run1)
	ai_charge(self, 10)
end

function f_death1(self)
	animate(self, 18, f_death2)
	sound(self, CHAN_VOICE, "fish/death.wav", 1, ATTN_NORM)
end

function f_death2(self)
	animate(self, 19, f_death3)
end

function f_death3(self)
	animate(self, 20, f_death4)
end

function f_death4(self)
	animate(self, 21, f_death5)
end

function f_death5(self)
	animate(self, 22, f_death6)
end

function f_death6(self)
	animate(self, 23, f_death7)
end

function f_death7(self)
	animate(self, 24, f_death8)
end

function f_death8(self)
	animate(self, 25, f_death9)
end

function f_death9(self)
	animate(self, 26, f_death10)
end

function f_death10(self)
	animate(self, 27, f_death11)
end

function f_death11(self)
	animate(self, 28, f_death12)
end

function f_death12(self)
	animate(self, 29, f_death13)
end

function f_death13(self)
	animate(self, 30, f_death14)
end

function f_death14(self)
	animate(self, 31, f_death15)
end

function f_death15(self)
	animate(self, 32, f_death16)
end

function f_death16(self)
	animate(self, 33, f_death17)
end

function f_death17(self)
	animate(self, 34, f_death18)
end

function f_death18(self)
	animate(self, 35, f_death19)
end

function f_death19(self)
	animate(self, 36, f_death20)
end

function f_death20(self)
	animate(self, 37, f_death21)
end

function f_death21(self)
	animate(self, 38, f_death21)
	self.solid = SOLID_NOT
end

function f_pain1(self)
	animate(self, 57, f_pain2)
end

function f_pain2(self)
	animate(self, 58, f_pain3)
	ai_pain(self, 6)
end

function f_pain3(self)
	animate(self, 59, f_pain4)
	ai_pain(self, 6)
end

function f_pain4(self)
	animate(self, 60, f_pain5)
	ai_pain(self, 6)
end

function f_pain5(self)
	animate(self, 61, f_pain6)
	ai_pain(self, 6)
end

function f_pain6(self)
	animate(self, 62, f_pain7)
	ai_pain(self, 6)
end

function f_pain7(self)
	animate(self, 63, f_pain8)
	ai_pain(self, 6)
end

function f_pain8(self)
	animate(self, 64, f_pain9)
	ai_pain(self, 6)
end

function f_pain9(self)
	animate(self, 65, f_run1)
	ai_pain(self, 6)
end

function fish_pain(self, attacker, damage)
	-- fish allways do pain frames
	f_pain1(self)
end

function monster_fish(self)
	if deathmatch ~= 0 then
		remove(self)
		return
	end
	self.solid = SOLID_SLIDEBOX
	self.movetype = MOVETYPE_STEP
	setmodel(self, "progs/fish.mdl")
	setsize(self, vector.create(-16, -16, -24), vector.create(16, 16, 24))
	self.health = 25
	self.th_stand = f_stand1
	self.th_walk = f_walk1
	self.th_run = f_run1
	self.th_die = f_death1
	self.th_pain = fish_pain
	self.th_melee = f_attack1
	swimmonster_start(self)
end
