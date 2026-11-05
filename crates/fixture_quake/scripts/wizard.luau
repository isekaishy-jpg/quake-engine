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
-- Ported from wizard.qc.

function LaunchMissile(self, missile, mspeed, accuracy)
	local move = vector.zero
	local vec, fly
	makevectors(self.angles)
	-- set missile speed
	vec = ent(self.enemy).origin + ent(self.enemy).mins + ent(self.enemy).size * 0.7 - missile.origin
	-- calc aproximate time for missile to reach vec
	fly = vector.magnitude(vec) / mspeed
	-- get the entities xy velocity
	move = ent(self.enemy).velocity
	move = with_z(move, 0)
	-- project the target forward in time
	vec = vec + move * fly
	vec = normalize(vec)
	vec = vec + accuracy * v_up * (random() - 0.5) + accuracy * v_right * (random() - 0.5)
	missile.velocity = vec * mspeed
	missile.angles = vector.zero
	missile.angles = with_y(missile.angles, vectoyaw(missile.velocity))
	-- set missile duration
	missile.nextthink = time() + 5
	missile.think = SUB_Remove
end

function WizardCheckAttack(self)
	local spot1, spot2, targ, chance
	if time() < self.attack_finished then
		return false
	end
	if not enemy_vis then
		return false
	end
	if enemy_range == RANGE_FAR then
		if self.attack_state ~= AS_STRAIGHT then
			self.attack_state = AS_STRAIGHT
			wiz_run1(self)
		end
		return false
	end
	targ = self.enemy
	-- see if any entities are in the way of the shot
	spot1 = self.origin + self.view_ofs
	spot2 = targ.origin + targ.view_ofs
	traceline(spot1, spot2, false, self)
	if trace_ent ~= targ then
		-- don't have a clear shot, so move to a side
		if self.attack_state ~= AS_STRAIGHT then
			self.attack_state = AS_STRAIGHT
			wiz_run1(self)
		end
		return false
	end
	if enemy_range == RANGE_MELEE then
		chance = 0.9
	elseif enemy_range == RANGE_NEAR then
		chance = 0.6
	elseif enemy_range == RANGE_MID then
		chance = 0.2
	else
		chance = 0
	end
	if random() < chance then
		self.attack_state = AS_MISSILE
		return true
	end
	if enemy_range == RANGE_MID then
		if self.attack_state ~= AS_STRAIGHT then
			self.attack_state = AS_STRAIGHT
			wiz_run1(self)
		end
	elseif self.attack_state ~= AS_SLIDING then
		self.attack_state = AS_SLIDING
		wiz_side1(self)
	end
	return false
end

function WizardAttackFinished(self)
	if enemy_range >= RANGE_MID or not enemy_vis then
		self.attack_state = AS_STRAIGHT
		self.think = wiz_run1
	else
		self.attack_state = AS_SLIDING
		self.think = wiz_side1
	end
end

function Wiz_FastFire(self)
	local vec, dst
	if ent(self.owner).health > 0 then
		ent(self.owner).effects = bit32.bor(ent(self.owner).effects, EF_MUZZLEFLASH)
		makevectors(ent(self.enemy).angles)
		dst = ent(self.enemy).origin - 13 * self.movedir
		vec = normalize(dst - self.origin)
		sound(self, CHAN_WEAPON, "wizard/wattack.wav", 1, ATTN_NORM)
		launch_spike(self, self.origin, vec, 600)
		newmis.owner = self.owner
		newmis.classname = "wizspike"
		setmodel(newmis, "progs/w_spike.mdl")
		setsize(newmis, VEC_ORIGIN, VEC_ORIGIN)
	end
	remove(self)
end

function Wiz_StartFast(self)
	local missile
	sound(self, CHAN_WEAPON, "wizard/wattack.wav", 1, ATTN_NORM)
	self.v_angle = self.angles
	makevectors(self.angles)
	missile = spawn()
	missile.owner = self
	missile.nextthink = time() + 0.6
	setsize(missile, vector.zero, vector.zero)
	setorigin(missile, self.origin + vector.create(0, 0, 30) + v_forward * 14 + v_right * 14)
	missile.enemy = self.enemy
	missile.nextthink = time() + 0.8
	missile.think = Wiz_FastFire
	missile.movedir = v_right
	missile = spawn()
	missile.owner = self
	missile.nextthink = time() + 1
	setsize(missile, vector.zero, vector.zero)
	setorigin(missile, self.origin + vector.create(0, 0, 30) + v_forward * 14 + v_right * -14)
	missile.enemy = self.enemy
	missile.nextthink = time() + 0.3
	missile.think = Wiz_FastFire
	missile.movedir = VEC_ORIGIN - v_right
end

function Wiz_idlesound(self)
	local wr
	wr = random() * 5
	if self.waitmin < time() then
		self.waitmin = time() + 2
		if wr > 4.5 then
			sound(self, CHAN_VOICE, "wizard/widle1.wav", 1, ATTN_IDLE)
		end
		if wr < 1.5 then
			sound(self, CHAN_VOICE, "wizard/widle2.wav", 1, ATTN_IDLE)
		end
	end
	return
end

function wiz_stand1(self)
	animate(self, 0, wiz_stand2)
	ai_stand(self)
end

function wiz_stand2(self)
	animate(self, 1, wiz_stand3)
	ai_stand(self)
end

function wiz_stand3(self)
	animate(self, 2, wiz_stand4)
	ai_stand(self)
end

function wiz_stand4(self)
	animate(self, 3, wiz_stand5)
	ai_stand(self)
end

function wiz_stand5(self)
	animate(self, 4, wiz_stand6)
	ai_stand(self)
end

function wiz_stand6(self)
	animate(self, 5, wiz_stand7)
	ai_stand(self)
end

function wiz_stand7(self)
	animate(self, 6, wiz_stand8)
	ai_stand(self)
end

function wiz_stand8(self)
	animate(self, 7, wiz_stand1)
	ai_stand(self)
end

function wiz_walk1(self)
	animate(self, 0, wiz_walk2)
	ai_walk(self, 8)
	Wiz_idlesound(self)
end

function wiz_walk2(self)
	animate(self, 1, wiz_walk3)
	ai_walk(self, 8)
end

function wiz_walk3(self)
	animate(self, 2, wiz_walk4)
	ai_walk(self, 8)
end

function wiz_walk4(self)
	animate(self, 3, wiz_walk5)
	ai_walk(self, 8)
end

function wiz_walk5(self)
	animate(self, 4, wiz_walk6)
	ai_walk(self, 8)
end

function wiz_walk6(self)
	animate(self, 5, wiz_walk7)
	ai_walk(self, 8)
end

function wiz_walk7(self)
	animate(self, 6, wiz_walk8)
	ai_walk(self, 8)
end

function wiz_walk8(self)
	animate(self, 7, wiz_walk1)
	ai_walk(self, 8)
end

function wiz_side1(self)
	animate(self, 0, wiz_side2)
	ai_run(self, 8)
	Wiz_idlesound(self)
end

function wiz_side2(self)
	animate(self, 1, wiz_side3)
	ai_run(self, 8)
end

function wiz_side3(self)
	animate(self, 2, wiz_side4)
	ai_run(self, 8)
end

function wiz_side4(self)
	animate(self, 3, wiz_side5)
	ai_run(self, 8)
end

function wiz_side5(self)
	animate(self, 4, wiz_side6)
	ai_run(self, 8)
end

function wiz_side6(self)
	animate(self, 5, wiz_side7)
	ai_run(self, 8)
end

function wiz_side7(self)
	animate(self, 6, wiz_side8)
	ai_run(self, 8)
end

function wiz_side8(self)
	animate(self, 7, wiz_side1)
	ai_run(self, 8)
end

function wiz_run1(self)
	animate(self, 15, wiz_run2)
	ai_run(self, 16)
	Wiz_idlesound(self)
end

function wiz_run2(self)
	animate(self, 16, wiz_run3)
	ai_run(self, 16)
end

function wiz_run3(self)
	animate(self, 17, wiz_run4)
	ai_run(self, 16)
end

function wiz_run4(self)
	animate(self, 18, wiz_run5)
	ai_run(self, 16)
end

function wiz_run5(self)
	animate(self, 19, wiz_run6)
	ai_run(self, 16)
end

function wiz_run6(self)
	animate(self, 20, wiz_run7)
	ai_run(self, 16)
end

function wiz_run7(self)
	animate(self, 21, wiz_run8)
	ai_run(self, 16)
end

function wiz_run8(self)
	animate(self, 22, wiz_run9)
	ai_run(self, 16)
end

function wiz_run9(self)
	animate(self, 23, wiz_run10)
	ai_run(self, 16)
end

function wiz_run10(self)
	animate(self, 24, wiz_run11)
	ai_run(self, 16)
end

function wiz_run11(self)
	animate(self, 25, wiz_run12)
	ai_run(self, 16)
end

function wiz_run12(self)
	animate(self, 26, wiz_run13)
	ai_run(self, 16)
end

function wiz_run13(self)
	animate(self, 27, wiz_run14)
	ai_run(self, 16)
end

function wiz_run14(self)
	animate(self, 28, wiz_run1)
	ai_run(self, 16)
end

function wiz_fast1(self)
	animate(self, 29, wiz_fast2)
	ai_face(self)
	Wiz_StartFast(self)
end

function wiz_fast2(self)
	animate(self, 30, wiz_fast3)
	ai_face(self)
end

function wiz_fast3(self)
	animate(self, 31, wiz_fast4)
	ai_face(self)
end

function wiz_fast4(self)
	animate(self, 32, wiz_fast5)
	ai_face(self)
end

function wiz_fast5(self)
	animate(self, 33, wiz_fast6)
	ai_face(self)
end

function wiz_fast6(self)
	animate(self, 34, wiz_fast7)
	ai_face(self)
end

function wiz_fast7(self)
	animate(self, 33, wiz_fast8)
	ai_face(self)
end

function wiz_fast8(self)
	animate(self, 32, wiz_fast9)
	ai_face(self)
end

function wiz_fast9(self)
	animate(self, 31, wiz_fast10)
	ai_face(self)
end

function wiz_fast10(self)
	animate(self, 30, wiz_run1)
	ai_face(self)
	SUB_AttackFinished(self, 2)
	WizardAttackFinished(self)
end

function wiz_pain1(self)
	animate(self, 42, wiz_pain2)
end

function wiz_pain2(self)
	animate(self, 43, wiz_pain3)
end

function wiz_pain3(self)
	animate(self, 44, wiz_pain4)
end

function wiz_pain4(self)
	animate(self, 45, wiz_run1)
end

function wiz_death1(self)
	animate(self, 46, wiz_death2)
	self.velocity = with_x(self.velocity, -200 + 400 * random())
	self.velocity = with_y(self.velocity, -200 + 400 * random())
	self.velocity = with_z(self.velocity, 100 + 100 * random())
	self.flags = self.flags - (bit32.band(self.flags, FL_ONGROUND))
	sound(self, CHAN_VOICE, "wizard/wdeath.wav", 1, ATTN_NORM)
end

function wiz_death2(self)
	animate(self, 47, wiz_death3)
end

function wiz_death3(self)
	animate(self, 48, wiz_death4)
	self.solid = SOLID_NOT
end

function wiz_death4(self)
	animate(self, 49, wiz_death5)
end

function wiz_death5(self)
	animate(self, 50, wiz_death6)
end

function wiz_death6(self)
	animate(self, 51, wiz_death7)
end

function wiz_death7(self)
	animate(self, 52, wiz_death8)
end

function wiz_death8(self)
	animate(self, 53, wiz_death8)
end

function wiz_die(self)
	-- check for gib
	if self.health < -40 then
		sound(self, CHAN_VOICE, "player/udeath.wav", 1, ATTN_NORM)
		ThrowHead(self, "progs/h_wizard.mdl", self.health)
		ThrowGib(self, "progs/gib2.mdl", self.health)
		ThrowGib(self, "progs/gib2.mdl", self.health)
		ThrowGib(self, "progs/gib2.mdl", self.health)
		return
	end
	wiz_death1(self)
end

function Wiz_Pain(self, attacker, damage)
	sound(self, CHAN_VOICE, "wizard/wpain.wav", 1, ATTN_NORM)
	if random() * 70 > damage then
		return
	end
	wiz_pain1(self)
end

function Wiz_Missile(self)
	wiz_fast1(self)
end

function monster_wizard(self)
	if deathmatch ~= 0 then
		remove(self)
		return
	end
	-- used by c code
	self.solid = SOLID_SLIDEBOX
	self.movetype = MOVETYPE_STEP
	setmodel(self, "progs/wizard.mdl")
	setsize(self, vector.create(-16, -16, -24), vector.create(16, 16, 40))
	self.health = 80
	self.th_stand = wiz_stand1
	self.th_walk = wiz_walk1
	self.th_run = wiz_run1
	self.th_missile = Wiz_Missile
	self.th_pain = Wiz_Pain
	self.th_die = wiz_die
	flymonster_start(self)
end
