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
-- Ported from demon.qc.

function demon1_stand1(self)
	animate(self, 0, demon1_stand2)
	ai_stand(self)
end

function demon1_stand2(self)
	animate(self, 1, demon1_stand3)
	ai_stand(self)
end

function demon1_stand3(self)
	animate(self, 2, demon1_stand4)
	ai_stand(self)
end

function demon1_stand4(self)
	animate(self, 3, demon1_stand5)
	ai_stand(self)
end

function demon1_stand5(self)
	animate(self, 4, demon1_stand6)
	ai_stand(self)
end

function demon1_stand6(self)
	animate(self, 5, demon1_stand7)
	ai_stand(self)
end

function demon1_stand7(self)
	animate(self, 6, demon1_stand8)
	ai_stand(self)
end

function demon1_stand8(self)
	animate(self, 7, demon1_stand9)
	ai_stand(self)
end

function demon1_stand9(self)
	animate(self, 8, demon1_stand10)
	ai_stand(self)
end

function demon1_stand10(self)
	animate(self, 9, demon1_stand11)
	ai_stand(self)
end

function demon1_stand11(self)
	animate(self, 10, demon1_stand12)
	ai_stand(self)
end

function demon1_stand12(self)
	animate(self, 11, demon1_stand13)
	ai_stand(self)
end

function demon1_stand13(self)
	animate(self, 12, demon1_stand1)
	ai_stand(self)
end

function demon1_walk1(self)
	animate(self, 13, demon1_walk2)
	if random() < 0.2 then
		sound(self, CHAN_VOICE, "demon/idle1.wav", 1, ATTN_IDLE)
	end
	ai_walk(self, 8)
end

function demon1_walk2(self)
	animate(self, 14, demon1_walk3)
	ai_walk(self, 6)
end

function demon1_walk3(self)
	animate(self, 15, demon1_walk4)
	ai_walk(self, 6)
end

function demon1_walk4(self)
	animate(self, 16, demon1_walk5)
	ai_walk(self, 7)
end

function demon1_walk5(self)
	animate(self, 17, demon1_walk6)
	ai_walk(self, 4)
end

function demon1_walk6(self)
	animate(self, 18, demon1_walk7)
	ai_walk(self, 6)
end

function demon1_walk7(self)
	animate(self, 19, demon1_walk8)
	ai_walk(self, 10)
end

function demon1_walk8(self)
	animate(self, 20, demon1_walk1)
	ai_walk(self, 10)
end

function demon1_run1(self)
	animate(self, 21, demon1_run2)
	if random() < 0.2 then
		sound(self, CHAN_VOICE, "demon/idle1.wav", 1, ATTN_IDLE)
	end
	ai_run(self, 20)
end

function demon1_run2(self)
	animate(self, 22, demon1_run3)
	ai_run(self, 15)
end

function demon1_run3(self)
	animate(self, 23, demon1_run4)
	ai_run(self, 36)
end

function demon1_run4(self)
	animate(self, 24, demon1_run5)
	ai_run(self, 20)
end

function demon1_run5(self)
	animate(self, 25, demon1_run6)
	ai_run(self, 15)
end

function demon1_run6(self)
	animate(self, 26, demon1_run1)
	ai_run(self, 36)
end

function demon1_jump1(self)
	animate(self, 27, demon1_jump2)
	ai_face(self)
end

function demon1_jump2(self)
	animate(self, 28, demon1_jump3)
	ai_face(self)
end

function demon1_jump3(self)
	animate(self, 29, demon1_jump4)
	ai_face(self)
end

function demon1_jump4(self)
	animate(self, 30, demon1_jump5)
	ai_face(self)
	self.touch = Demon_JumpTouch
	makevectors(self.angles)
	self.origin = with_z(self.origin, self.origin.z + 1)
	self.velocity = v_forward * 600 + vector.create(0, 0, 250)
	if bit32.band(self.flags, FL_ONGROUND) ~= 0 then
		self.flags = self.flags - FL_ONGROUND
	end
end

function demon1_jump5(self)
	animate(self, 31, demon1_jump6)
end

function demon1_jump6(self)
	animate(self, 32, demon1_jump7)
end

function demon1_jump7(self)
	animate(self, 33, demon1_jump8)
end

function demon1_jump8(self)
	animate(self, 34, demon1_jump9)
end

function demon1_jump9(self)
	animate(self, 35, demon1_jump10)
end

function demon1_jump10(self)
	animate(self, 36, demon1_jump1)
	self.nextthink = time() + 3
	-- if three seconds pass, assume demon is stuck and jump again
end

function demon1_jump11(self)
	animate(self, 37, demon1_jump12)
end

function demon1_jump12(self)
	animate(self, 38, demon1_run1)
end

function demon1_atta1(self)
	animate(self, 54, demon1_atta2)
	ai_charge(self, 4)
end

function demon1_atta2(self)
	animate(self, 55, demon1_atta3)
	ai_charge(self, 0)
end

function demon1_atta3(self)
	animate(self, 56, demon1_atta4)
	ai_charge(self, 0)
end

function demon1_atta4(self)
	animate(self, 57, demon1_atta5)
	ai_charge(self, 1)
end

function demon1_atta5(self)
	animate(self, 58, demon1_atta6)
	ai_charge(self, 2)
	Demon_Melee(self, 200)
end

function demon1_atta6(self)
	animate(self, 59, demon1_atta7)
	ai_charge(self, 1)
end

function demon1_atta7(self)
	animate(self, 60, demon1_atta8)
	ai_charge(self, 6)
end

function demon1_atta8(self)
	animate(self, 61, demon1_atta9)
	ai_charge(self, 8)
end

function demon1_atta9(self)
	animate(self, 62, demon1_atta10)
	ai_charge(self, 4)
end

function demon1_atta10(self)
	animate(self, 63, demon1_atta11)
	ai_charge(self, 2)
end

function demon1_atta11(self)
	animate(self, 64, demon1_atta12)
	Demon_Melee(self, -200)
end

function demon1_atta12(self)
	animate(self, 65, demon1_atta13)
	ai_charge(self, 5)
end

function demon1_atta13(self)
	animate(self, 66, demon1_atta14)
	ai_charge(self, 8)
end

function demon1_atta14(self)
	animate(self, 67, demon1_atta15)
	ai_charge(self, 4)
end

function demon1_atta15(self)
	animate(self, 68, demon1_run1)
	ai_charge(self, 4)
end

function demon1_pain1(self)
	animate(self, 39, demon1_pain2)
end

function demon1_pain2(self)
	animate(self, 40, demon1_pain3)
end

function demon1_pain3(self)
	animate(self, 41, demon1_pain4)
end

function demon1_pain4(self)
	animate(self, 42, demon1_pain5)
end

function demon1_pain5(self)
	animate(self, 43, demon1_pain6)
end

function demon1_pain6(self)
	animate(self, 44, demon1_run1)
end

function demon1_pain(self, attacker, damage)
	if self.touch == Demon_JumpTouch then
		return
	end
	if self.pain_finished > time() then
		return
	end
	self.pain_finished = time() + 1
	sound(self, CHAN_VOICE, "demon/dpain1.wav", 1, ATTN_NORM)
	if random() * 200 > damage then
		return
	end
	demon1_pain1(self)
end

function demon1_die1(self)
	animate(self, 45, demon1_die2)
	sound(self, CHAN_VOICE, "demon/ddeath.wav", 1, ATTN_NORM)
end

function demon1_die2(self)
	animate(self, 46, demon1_die3)
end

function demon1_die3(self)
	animate(self, 47, demon1_die4)
end

function demon1_die4(self)
	animate(self, 48, demon1_die5)
end

function demon1_die5(self)
	animate(self, 49, demon1_die6)
end

function demon1_die6(self)
	animate(self, 50, demon1_die7)
	self.solid = SOLID_NOT
end

function demon1_die7(self)
	animate(self, 51, demon1_die8)
end

function demon1_die8(self)
	animate(self, 52, demon1_die9)
end

function demon1_die9(self)
	animate(self, 53, demon1_die9)
end

function demon_die(self)
	-- check for gib
	if self.health < -80 then
		sound(self, CHAN_VOICE, "player/udeath.wav", 1, ATTN_NORM)
		ThrowHead(self, "progs/h_demon.mdl", self.health)
		ThrowGib(self, "progs/gib1.mdl", self.health)
		ThrowGib(self, "progs/gib1.mdl", self.health)
		ThrowGib(self, "progs/gib1.mdl", self.health)
		return
	end
	demon1_die1(self)
end

function Demon_MeleeAttack(self)
	demon1_atta1(self)
end

function monster_demon1(self)
	if deathmatch ~= 0 then
		remove(self)
		return
	end
	self.solid = SOLID_SLIDEBOX
	self.movetype = MOVETYPE_STEP
	setmodel(self, "progs/demon.mdl")
	setsize(self, VEC_HULL2_MIN, VEC_HULL2_MAX)
	self.health = 300
	self.th_stand = demon1_stand1
	self.th_walk = demon1_walk1
	self.th_run = demon1_run1
	self.th_die = demon_die
	self.th_melee = Demon_MeleeAttack
	-- one of two attacks
	self.th_missile = demon1_jump1
	-- jump attack
	self.th_pain = demon1_pain
	walkmonster_start(self)
end

function CheckDemonMelee(self)
	if enemy_range == RANGE_MELEE then
		-- FIXME: check canreach
		self.attack_state = AS_MELEE
		return true
	end
	return false
end

function CheckDemonJump(self)
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
	if d < 100 then
		return false
	end
	if d > 200 then
		if random() < 0.9 then
			return false
		end
	end
	return true
end

function DemonCheckAttack(self)
	-- if close enough for slashing, go for it
	if CheckDemonMelee(self) then
		self.attack_state = AS_MELEE
		return true
	end
	if CheckDemonJump(self) then
		self.attack_state = AS_MISSILE
		sound(self, CHAN_VOICE, "demon/djump.wav", 1, ATTN_NORM)
		return true
	end
	return false
end

function Demon_Melee(self, side)
	local ldmg, delta
	ai_face(self)
	walkmove(self, self.ideal_yaw, 12)
	-- allow a little closing
	delta = ent(self.enemy).origin - self.origin
	if vector.magnitude(delta) > 100 then
		return
	end
	if not CanDamage(self.enemy, self) then
		return
	end
	sound(self, CHAN_WEAPON, "demon/dhit2.wav", 1, ATTN_NORM)
	ldmg = 10 + 5 * random()
	T_Damage(self.enemy, self, self, ldmg)
	makevectors(self.angles)
	SpawnMeatSpray(self, self.origin + v_forward * 16, side * v_right)
end

function Demon_JumpTouch(self, other)
	local ldmg
	if self.health <= 0 then
		return
	end
	if other.takedamage ~= 0 then
		if vector.magnitude(self.velocity) > 400 then
			ldmg = 40 + 10 * random()
			T_Damage(other, self, self, ldmg)
		end
	end
	if not checkbottom(self) then
		if bit32.band(self.flags, FL_ONGROUND) ~= 0 then
			-- jump randomly to not get hung up
			self.touch = SUB_Null
			self.think = demon1_jump1
			self.nextthink = time() + 0.1
		end
		return
		-- not on ground yet
	end
	self.touch = SUB_Null
	self.think = demon1_jump11
	self.nextthink = time() + 0.1
end
