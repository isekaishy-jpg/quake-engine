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
-- Ported from tarbaby.qc.

function tbaby_stand1(self)
	animate(self, 0, tbaby_stand1)
	ai_stand(self)
end

function tbaby_hang1(self)
	animate(self, 0, tbaby_hang1)
	ai_stand(self)
end

function tbaby_walk1(self)
	animate(self, 0, tbaby_walk2)
	ai_turn(self)
end

function tbaby_walk2(self)
	animate(self, 1, tbaby_walk3)
	ai_turn(self)
end

function tbaby_walk3(self)
	animate(self, 2, tbaby_walk4)
	ai_turn(self)
end

function tbaby_walk4(self)
	animate(self, 3, tbaby_walk5)
	ai_turn(self)
end

function tbaby_walk5(self)
	animate(self, 4, tbaby_walk6)
	ai_turn(self)
end

function tbaby_walk6(self)
	animate(self, 5, tbaby_walk7)
	ai_turn(self)
end

function tbaby_walk7(self)
	animate(self, 6, tbaby_walk8)
	ai_turn(self)
end

function tbaby_walk8(self)
	animate(self, 7, tbaby_walk9)
	ai_turn(self)
end

function tbaby_walk9(self)
	animate(self, 8, tbaby_walk10)
	ai_turn(self)
end

function tbaby_walk10(self)
	animate(self, 9, tbaby_walk11)
	ai_turn(self)
end

function tbaby_walk11(self)
	animate(self, 10, tbaby_walk12)
	ai_walk(self, 2)
end

function tbaby_walk12(self)
	animate(self, 11, tbaby_walk13)
	ai_walk(self, 2)
end

function tbaby_walk13(self)
	animate(self, 12, tbaby_walk14)
	ai_walk(self, 2)
end

function tbaby_walk14(self)
	animate(self, 13, tbaby_walk15)
	ai_walk(self, 2)
end

function tbaby_walk15(self)
	animate(self, 14, tbaby_walk16)
	ai_walk(self, 2)
end

function tbaby_walk16(self)
	animate(self, 15, tbaby_walk17)
	ai_walk(self, 2)
end

function tbaby_walk17(self)
	animate(self, 16, tbaby_walk18)
	ai_walk(self, 2)
end

function tbaby_walk18(self)
	animate(self, 17, tbaby_walk19)
	ai_walk(self, 2)
end

function tbaby_walk19(self)
	animate(self, 18, tbaby_walk20)
	ai_walk(self, 2)
end

function tbaby_walk20(self)
	animate(self, 19, tbaby_walk21)
	ai_walk(self, 2)
end

function tbaby_walk21(self)
	animate(self, 20, tbaby_walk22)
	ai_walk(self, 2)
end

function tbaby_walk22(self)
	animate(self, 21, tbaby_walk23)
	ai_walk(self, 2)
end

function tbaby_walk23(self)
	animate(self, 22, tbaby_walk24)
	ai_walk(self, 2)
end

function tbaby_walk24(self)
	animate(self, 23, tbaby_walk25)
	ai_walk(self, 2)
end

function tbaby_walk25(self)
	animate(self, 24, tbaby_walk1)
	ai_walk(self, 2)
end

function tbaby_run1(self)
	animate(self, 25, tbaby_run2)
	ai_face(self)
end

function tbaby_run2(self)
	animate(self, 26, tbaby_run3)
	ai_face(self)
end

function tbaby_run3(self)
	animate(self, 27, tbaby_run4)
	ai_face(self)
end

function tbaby_run4(self)
	animate(self, 28, tbaby_run5)
	ai_face(self)
end

function tbaby_run5(self)
	animate(self, 29, tbaby_run6)
	ai_face(self)
end

function tbaby_run6(self)
	animate(self, 30, tbaby_run7)
	ai_face(self)
end

function tbaby_run7(self)
	animate(self, 31, tbaby_run8)
	ai_face(self)
end

function tbaby_run8(self)
	animate(self, 32, tbaby_run9)
	ai_face(self)
end

function tbaby_run9(self)
	animate(self, 33, tbaby_run10)
	ai_face(self)
end

function tbaby_run10(self)
	animate(self, 34, tbaby_run11)
	ai_face(self)
end

function tbaby_run11(self)
	animate(self, 35, tbaby_run12)
	ai_run(self, 2)
end

function tbaby_run12(self)
	animate(self, 36, tbaby_run13)
	ai_run(self, 2)
end

function tbaby_run13(self)
	animate(self, 37, tbaby_run14)
	ai_run(self, 2)
end

function tbaby_run14(self)
	animate(self, 38, tbaby_run15)
	ai_run(self, 2)
end

function tbaby_run15(self)
	animate(self, 39, tbaby_run16)
	ai_run(self, 2)
end

function tbaby_run16(self)
	animate(self, 40, tbaby_run17)
	ai_run(self, 2)
end

function tbaby_run17(self)
	animate(self, 41, tbaby_run18)
	ai_run(self, 2)
end

function tbaby_run18(self)
	animate(self, 42, tbaby_run19)
	ai_run(self, 2)
end

function tbaby_run19(self)
	animate(self, 43, tbaby_run20)
	ai_run(self, 2)
end

function tbaby_run20(self)
	animate(self, 44, tbaby_run21)
	ai_run(self, 2)
end

function tbaby_run21(self)
	animate(self, 45, tbaby_run22)
	ai_run(self, 2)
end

function tbaby_run22(self)
	animate(self, 46, tbaby_run23)
	ai_run(self, 2)
end

function tbaby_run23(self)
	animate(self, 47, tbaby_run24)
	ai_run(self, 2)
end

function tbaby_run24(self)
	animate(self, 48, tbaby_run25)
	ai_run(self, 2)
end

function tbaby_run25(self)
	animate(self, 49, tbaby_run1)
	ai_run(self, 2)
end

function Tar_JumpTouch(self, other)
	local ldmg
	if other.takedamage ~= 0 and other.classname ~= self.classname then
		if vector.magnitude(self.velocity) > 400 then
			ldmg = 10 + 10 * random()
			T_Damage(other, self, self, ldmg)
			sound(self, CHAN_WEAPON, "blob/hit1.wav", 1, ATTN_NORM)
		end
	else
		sound(self, CHAN_WEAPON, "blob/land1.wav", 1, ATTN_NORM)
	end
	if not checkbottom(self) then
		if bit32.band(self.flags, FL_ONGROUND) ~= 0 then
			-- jump randomly to not get hung up
			self.touch = SUB_Null
			self.think = tbaby_run1
			self.movetype = MOVETYPE_STEP
			self.nextthink = time() + 0.1
		end
		return
		-- not on ground yet
	end
	self.touch = SUB_Null
	self.think = tbaby_jump1
	self.nextthink = time() + 0.1
end

function tbaby_fly1(self)
	animate(self, 56, tbaby_fly2)
end

function tbaby_fly2(self)
	animate(self, 57, tbaby_fly3)
end

function tbaby_fly3(self)
	animate(self, 58, tbaby_fly4)
end

function tbaby_fly4(self)
	animate(self, 59, tbaby_fly1)
	self.cnt = self.cnt + 1
	if self.cnt == 4 then
		tbaby_jump5(self)
	end
end

function tbaby_jump1(self)
	animate(self, 50, tbaby_jump2)
	ai_face(self)
end

function tbaby_jump2(self)
	animate(self, 51, tbaby_jump3)
	ai_face(self)
end

function tbaby_jump3(self)
	animate(self, 52, tbaby_jump4)
	ai_face(self)
end

function tbaby_jump4(self)
	animate(self, 53, tbaby_jump5)
	ai_face(self)
end

function tbaby_jump5(self)
	animate(self, 54, tbaby_jump6)
	self.movetype = MOVETYPE_BOUNCE
	self.touch = Tar_JumpTouch
	makevectors(self.angles)
	self.origin = with_z(self.origin, self.origin.z + 1)
	self.velocity = v_forward * 600 + vector.create(0, 0, 200)
	self.velocity = with_z(self.velocity, self.velocity.z + random() * 150)
	if bit32.band(self.flags, FL_ONGROUND) ~= 0 then
		self.flags = self.flags - FL_ONGROUND
	end
	self.cnt = 0
end

function tbaby_jump6(self)
	animate(self, 55, tbaby_fly1)
end

function tbaby_die1(self)
	animate(self, 60, tbaby_die2)
	self.takedamage = DAMAGE_NO
end

function tbaby_die2(self)
	animate(self, 60, tbaby_run1)
	T_RadiusDamage(self, self, 120, world)
	sound(self, CHAN_VOICE, "blob/death1.wav", 1, ATTN_NORM)
	self.origin = self.origin - 8 * normalize(self.velocity)
	temp_entity(TE_TAREXPLOSION, self.origin)
	BecomeExplosion(self)
end

function monster_tarbaby(self)
	if deathmatch ~= 0 then
		remove(self)
		return
	end
	self.solid = SOLID_SLIDEBOX
	self.movetype = MOVETYPE_STEP
	setmodel(self, "progs/tarbaby.mdl")
	setsize(self, vector.create(-16, -16, -24), vector.create(16, 16, 40))
	self.health = 80
	self.th_stand = tbaby_stand1
	self.th_walk = tbaby_walk1
	self.th_run = tbaby_run1
	self.th_missile = tbaby_jump1
	self.th_melee = tbaby_jump1
	self.th_die = tbaby_die1
	walkmonster_start(self)
end
