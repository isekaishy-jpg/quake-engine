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
-- Ported from dog.qc.

function dog_bite(self)
	local delta, ldmg
	if not exists(self.enemy) then
		return
	end
	ai_charge(self, 10)
	if not CanDamage(self.enemy, self) then
		return
	end
	delta = ent(self.enemy).origin - self.origin
	if vector.magnitude(delta) > 100 then
		return
	end
	ldmg = (random() + random() + random()) * 8
	T_Damage(self.enemy, self, self, ldmg)
end

function Dog_JumpTouch(self, other)
	local ldmg
	if self.health <= 0 then
		return
	end
	if other.takedamage ~= 0 then
		if vector.magnitude(self.velocity) > 300 then
			ldmg = 10 + 10 * random()
			T_Damage(other, self, self, ldmg)
		end
	end
	if not checkbottom(self) then
		if bit32.band(self.flags, FL_ONGROUND) ~= 0 then
			-- jump randomly to not get hung up
			self.touch = SUB_Null
			self.think = dog_leap1
			self.nextthink = time() + 0.1
		end
		return
		-- not on ground yet
	end
	self.touch = SUB_Null
	self.think = dog_run1
	self.nextthink = time() + 0.1
end

function dog_stand1(self)
	animate(self, 69, dog_stand2)
	ai_stand(self)
end

function dog_stand2(self)
	animate(self, 70, dog_stand3)
	ai_stand(self)
end

function dog_stand3(self)
	animate(self, 71, dog_stand4)
	ai_stand(self)
end

function dog_stand4(self)
	animate(self, 72, dog_stand5)
	ai_stand(self)
end

function dog_stand5(self)
	animate(self, 73, dog_stand6)
	ai_stand(self)
end

function dog_stand6(self)
	animate(self, 74, dog_stand7)
	ai_stand(self)
end

function dog_stand7(self)
	animate(self, 75, dog_stand8)
	ai_stand(self)
end

function dog_stand8(self)
	animate(self, 76, dog_stand9)
	ai_stand(self)
end

function dog_stand9(self)
	animate(self, 77, dog_stand1)
	ai_stand(self)
end

function dog_walk1(self)
	animate(self, 78, dog_walk2)
	if random() < 0.2 then
		sound(self, CHAN_VOICE, "dog/idle.wav", 1, ATTN_IDLE)
	end
	ai_walk(self, 8)
end

function dog_walk2(self)
	animate(self, 79, dog_walk3)
	ai_walk(self, 8)
end

function dog_walk3(self)
	animate(self, 80, dog_walk4)
	ai_walk(self, 8)
end

function dog_walk4(self)
	animate(self, 81, dog_walk5)
	ai_walk(self, 8)
end

function dog_walk5(self)
	animate(self, 82, dog_walk6)
	ai_walk(self, 8)
end

function dog_walk6(self)
	animate(self, 83, dog_walk7)
	ai_walk(self, 8)
end

function dog_walk7(self)
	animate(self, 84, dog_walk8)
	ai_walk(self, 8)
end

function dog_walk8(self)
	animate(self, 85, dog_walk1)
	ai_walk(self, 8)
end

function dog_run1(self)
	animate(self, 48, dog_run2)
	if random() < 0.2 then
		sound(self, CHAN_VOICE, "dog/idle.wav", 1, ATTN_IDLE)
	end
	ai_run(self, 16)
end

function dog_run2(self)
	animate(self, 49, dog_run3)
	ai_run(self, 32)
end

function dog_run3(self)
	animate(self, 50, dog_run4)
	ai_run(self, 32)
end

function dog_run4(self)
	animate(self, 51, dog_run5)
	ai_run(self, 20)
end

function dog_run5(self)
	animate(self, 52, dog_run6)
	ai_run(self, 64)
end

function dog_run6(self)
	animate(self, 53, dog_run7)
	ai_run(self, 32)
end

function dog_run7(self)
	animate(self, 54, dog_run8)
	ai_run(self, 16)
end

function dog_run8(self)
	animate(self, 55, dog_run9)
	ai_run(self, 32)
end

function dog_run9(self)
	animate(self, 56, dog_run10)
	ai_run(self, 32)
end

function dog_run10(self)
	animate(self, 57, dog_run11)
	ai_run(self, 20)
end

function dog_run11(self)
	animate(self, 58, dog_run12)
	ai_run(self, 64)
end

function dog_run12(self)
	animate(self, 59, dog_run1)
	ai_run(self, 32)
end

function dog_atta1(self)
	animate(self, 0, dog_atta2)
	ai_charge(self, 10)
end

function dog_atta2(self)
	animate(self, 1, dog_atta3)
	ai_charge(self, 10)
end

function dog_atta3(self)
	animate(self, 2, dog_atta4)
	ai_charge(self, 10)
end

function dog_atta4(self)
	animate(self, 3, dog_atta5)
	sound(self, CHAN_VOICE, "dog/dattack1.wav", 1, ATTN_NORM)
	dog_bite(self)
end

function dog_atta5(self)
	animate(self, 4, dog_atta6)
	ai_charge(self, 10)
end

function dog_atta6(self)
	animate(self, 5, dog_atta7)
	ai_charge(self, 10)
end

function dog_atta7(self)
	animate(self, 6, dog_atta8)
	ai_charge(self, 10)
end

function dog_atta8(self)
	animate(self, 7, dog_run1)
	ai_charge(self, 10)
end

function dog_leap1(self)
	animate(self, 60, dog_leap2)
	ai_face(self)
end

function dog_leap2(self)
	animate(self, 61, dog_leap3)
	ai_face(self)
	self.touch = Dog_JumpTouch
	makevectors(self.angles)
	self.origin = with_z(self.origin, self.origin.z + 1)
	self.velocity = v_forward * 300 + vector.create(0, 0, 200)
	if bit32.band(self.flags, FL_ONGROUND) ~= 0 then
		self.flags = self.flags - FL_ONGROUND
	end
end

function dog_leap3(self)
	animate(self, 62, dog_leap4)
end

function dog_leap4(self)
	animate(self, 63, dog_leap5)
end

function dog_leap5(self)
	animate(self, 64, dog_leap6)
end

function dog_leap6(self)
	animate(self, 65, dog_leap7)
end

function dog_leap7(self)
	animate(self, 66, dog_leap8)
end

function dog_leap8(self)
	animate(self, 67, dog_leap9)
end

function dog_leap9(self)
	animate(self, 68, dog_leap9)
end

function dog_pain1(self)
	animate(self, 26, dog_pain2)
end

function dog_pain2(self)
	animate(self, 27, dog_pain3)
end

function dog_pain3(self)
	animate(self, 28, dog_pain4)
end

function dog_pain4(self)
	animate(self, 29, dog_pain5)
end

function dog_pain5(self)
	animate(self, 30, dog_pain6)
end

function dog_pain6(self)
	animate(self, 31, dog_run1)
end

function dog_painb1(self)
	animate(self, 32, dog_painb2)
end

function dog_painb2(self)
	animate(self, 33, dog_painb3)
end

function dog_painb3(self)
	animate(self, 34, dog_painb4)
	ai_pain(self, 4)
end

function dog_painb4(self)
	animate(self, 35, dog_painb5)
	ai_pain(self, 12)
end

function dog_painb5(self)
	animate(self, 36, dog_painb6)
	ai_pain(self, 12)
end

function dog_painb6(self)
	animate(self, 37, dog_painb7)
	ai_pain(self, 2)
end

function dog_painb7(self)
	animate(self, 38, dog_painb8)
end

function dog_painb8(self)
	animate(self, 39, dog_painb9)
	ai_pain(self, 4)
end

function dog_painb9(self)
	animate(self, 40, dog_painb10)
end

function dog_painb10(self)
	animate(self, 41, dog_painb11)
	ai_pain(self, 10)
end

function dog_painb11(self)
	animate(self, 42, dog_painb12)
end

function dog_painb12(self)
	animate(self, 43, dog_painb13)
end

function dog_painb13(self)
	animate(self, 44, dog_painb14)
end

function dog_painb14(self)
	animate(self, 45, dog_painb15)
end

function dog_painb15(self)
	animate(self, 46, dog_painb16)
end

function dog_painb16(self)
	animate(self, 47, dog_run1)
end

function dog_pain(self)
	sound(self, CHAN_VOICE, "dog/dpain1.wav", 1, ATTN_NORM)
	if random() > 0.5 then
		dog_pain1(self)
	else
		dog_painb1(self)
	end
end

function dog_die1(self)
	animate(self, 8, dog_die2)
end

function dog_die2(self)
	animate(self, 9, dog_die3)
end

function dog_die3(self)
	animate(self, 10, dog_die4)
end

function dog_die4(self)
	animate(self, 11, dog_die5)
end

function dog_die5(self)
	animate(self, 12, dog_die6)
end

function dog_die6(self)
	animate(self, 13, dog_die7)
end

function dog_die7(self)
	animate(self, 14, dog_die8)
end

function dog_die8(self)
	animate(self, 15, dog_die9)
end

function dog_die9(self)
	animate(self, 16, dog_die9)
end

function dog_dieb1(self)
	animate(self, 17, dog_dieb2)
end

function dog_dieb2(self)
	animate(self, 18, dog_dieb3)
end

function dog_dieb3(self)
	animate(self, 19, dog_dieb4)
end

function dog_dieb4(self)
	animate(self, 20, dog_dieb5)
end

function dog_dieb5(self)
	animate(self, 21, dog_dieb6)
end

function dog_dieb6(self)
	animate(self, 22, dog_dieb7)
end

function dog_dieb7(self)
	animate(self, 23, dog_dieb8)
end

function dog_dieb8(self)
	animate(self, 24, dog_dieb9)
end

function dog_dieb9(self)
	animate(self, 25, dog_dieb9)
end

function dog_die(self)
	-- check for gib
	if self.health < -35 then
		sound(self, CHAN_VOICE, "player/udeath.wav", 1, ATTN_NORM)
		ThrowGib(self, "progs/gib3.mdl", self.health)
		ThrowGib(self, "progs/gib3.mdl", self.health)
		ThrowGib(self, "progs/gib3.mdl", self.health)
		ThrowHead(self, "progs/h_dog.mdl", self.health)
		return
	end
	sound(self, CHAN_VOICE, "dog/ddeath.wav", 1, ATTN_NORM)
	self.solid = SOLID_NOT
	if random() > 0.5 then
		dog_die1(self)
	else
		dog_dieb1(self)
	end
end

function CheckDogMelee(self)
	if enemy_range == RANGE_MELEE then
		-- FIXME: check canreach
		self.attack_state = AS_MELEE
		return true
	end
	return false
end

function CheckDogJump(self)
	local dist = vector.zero
	local d
	local enemy = ent(self.enemy)
	if self.origin.z + self.mins.z > enemy.origin.z + enemy.mins.z + 0.75 * enemy.size.z then
		return false
	end
	if self.origin.z + self.maxs.z < enemy.origin.z + enemy.mins.z + 0.25 * enemy.size.z then
		return false
	end
	dist = enemy.origin - self.origin
	dist = with_z(dist, 0)
	d = vector.magnitude(dist)
	if d < 80 then
		return false
	end
	if d > 150 then
		return false
	end
	return true
end

function DogCheckAttack(self)
	-- if close enough for slashing, go for it
	if CheckDogMelee(self) then
		self.attack_state = AS_MELEE
		return true
	end
	if CheckDogJump(self) then
		self.attack_state = AS_MISSILE
		return true
	end
	return false
end

function monster_dog(self)
	if deathmatch ~= 0 then
		remove(self)
		return
	end
	self.solid = SOLID_SLIDEBOX
	self.movetype = MOVETYPE_STEP
	setmodel(self, "progs/dog.mdl")
	setsize(self, vector.create(-32, -32, -24), vector.create(32, 32, 40))
	self.health = 25
	self.th_stand = dog_stand1
	self.th_walk = dog_walk1
	self.th_run = dog_run1
	self.th_pain = dog_pain
	self.th_die = dog_die
	self.th_melee = dog_atta1
	self.th_missile = dog_leap1
	walkmonster_start(self)
end
