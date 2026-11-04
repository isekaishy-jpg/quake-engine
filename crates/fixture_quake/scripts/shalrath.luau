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
-- Ported from shalrath.qc.

function shal_stand(self)
	animate(self, 23, shal_stand)
	ai_stand(self)
end

function shal_walk1(self)
	animate(self, 24, shal_walk2)
	if random() < 0.2 then
		sound(self, CHAN_VOICE, "shalrath/idle.wav", 1, ATTN_IDLE)
	end
	ai_walk(self, 6)
end

function shal_walk2(self)
	animate(self, 25, shal_walk3)
	ai_walk(self, 4)
end

function shal_walk3(self)
	animate(self, 26, shal_walk4)
	ai_walk(self, 0)
end

function shal_walk4(self)
	animate(self, 27, shal_walk5)
	ai_walk(self, 0)
end

function shal_walk5(self)
	animate(self, 28, shal_walk6)
	ai_walk(self, 0)
end

function shal_walk6(self)
	animate(self, 29, shal_walk7)
	ai_walk(self, 0)
end

function shal_walk7(self)
	animate(self, 30, shal_walk8)
	ai_walk(self, 5)
end

function shal_walk8(self)
	animate(self, 31, shal_walk9)
	ai_walk(self, 6)
end

function shal_walk9(self)
	animate(self, 32, shal_walk10)
	ai_walk(self, 5)
end

function shal_walk10(self)
	animate(self, 33, shal_walk11)
	ai_walk(self, 0)
end

function shal_walk11(self)
	animate(self, 34, shal_walk12)
	ai_walk(self, 4)
end

function shal_walk12(self)
	animate(self, 23, shal_walk1)
	ai_walk(self, 5)
end

function shal_run1(self)
	animate(self, 24, shal_run2)
	if random() < 0.2 then
		sound(self, CHAN_VOICE, "shalrath/idle.wav", 1, ATTN_IDLE)
	end
	ai_run(self, 6)
end

function shal_run2(self)
	animate(self, 25, shal_run3)
	ai_run(self, 4)
end

function shal_run3(self)
	animate(self, 26, shal_run4)
	ai_run(self, 0)
end

function shal_run4(self)
	animate(self, 27, shal_run5)
	ai_run(self, 0)
end

function shal_run5(self)
	animate(self, 28, shal_run6)
	ai_run(self, 0)
end

function shal_run6(self)
	animate(self, 29, shal_run7)
	ai_run(self, 0)
end

function shal_run7(self)
	animate(self, 30, shal_run8)
	ai_run(self, 5)
end

function shal_run8(self)
	animate(self, 31, shal_run9)
	ai_run(self, 6)
end

function shal_run9(self)
	animate(self, 32, shal_run10)
	ai_run(self, 5)
end

function shal_run10(self)
	animate(self, 33, shal_run11)
	ai_run(self, 0)
end

function shal_run11(self)
	animate(self, 34, shal_run12)
	ai_run(self, 4)
end

function shal_run12(self)
	animate(self, 23, shal_run1)
	ai_run(self, 5)
end

function shal_attack1(self)
	animate(self, 0, shal_attack2)
	sound(self, CHAN_VOICE, "shalrath/attack.wav", 1, ATTN_NORM)
	ai_face(self)
end

function shal_attack2(self)
	animate(self, 1, shal_attack3)
	ai_face(self)
end

function shal_attack3(self)
	animate(self, 2, shal_attack4)
	ai_face(self)
end

function shal_attack4(self)
	animate(self, 3, shal_attack5)
	ai_face(self)
end

function shal_attack5(self)
	animate(self, 4, shal_attack6)
	ai_face(self)
end

function shal_attack6(self)
	animate(self, 5, shal_attack7)
	ai_face(self)
end

function shal_attack7(self)
	animate(self, 6, shal_attack8)
	ai_face(self)
end

function shal_attack8(self)
	animate(self, 7, shal_attack9)
	ai_face(self)
end

function shal_attack9(self)
	animate(self, 8, shal_attack10)
	ShalMissile(self)
end

function shal_attack10(self)
	animate(self, 9, shal_attack11)
	ai_face(self)
end

function shal_attack11(self)
	animate(self, 10, shal_run1)
end

function shal_pain1(self)
	animate(self, 11, shal_pain2)
end

function shal_pain2(self)
	animate(self, 12, shal_pain3)
end

function shal_pain3(self)
	animate(self, 13, shal_pain4)
end

function shal_pain4(self)
	animate(self, 14, shal_pain5)
end

function shal_pain5(self)
	animate(self, 15, shal_run1)
end

function shal_death1(self)
	animate(self, 16, shal_death2)
end

function shal_death2(self)
	animate(self, 17, shal_death3)
end

function shal_death3(self)
	animate(self, 18, shal_death4)
end

function shal_death4(self)
	animate(self, 19, shal_death5)
end

function shal_death5(self)
	animate(self, 20, shal_death6)
end

function shal_death6(self)
	animate(self, 21, shal_death7)
end

function shal_death7(self)
	animate(self, 22, shal_death7)
end

function shalrath_pain(self)
	if self.pain_finished > time() then
		return
	end
	sound(self, CHAN_VOICE, "shalrath/pain.wav", 1, ATTN_NORM)
	shal_pain1(self)
	self.pain_finished = time() + 3
end

function shalrath_die(self)
	-- check for gib
	if self.health < -90 then
		sound(self, CHAN_VOICE, "player/udeath.wav", 1, ATTN_NORM)
		ThrowHead(self, "progs/h_shal.mdl", self.health)
		ThrowGib(self, "progs/gib1.mdl", self.health)
		ThrowGib(self, "progs/gib2.mdl", self.health)
		ThrowGib(self, "progs/gib3.mdl", self.health)
		return
	end
	sound(self, CHAN_VOICE, "shalrath/death.wav", 1, ATTN_NORM)
	shal_death1(self)
	self.solid = SOLID_NOT
	-- insert death sounds here
end

function ShalMissile(self)
	local missile, dir, dist, flytime
	dir = normalize((ent(self.enemy).origin + vector.create(0, 0, 10)) - self.origin)
	dist = vector.magnitude(ent(self.enemy).origin - self.origin)
	flytime = dist * 0.002
	if flytime < 0.1 then
		flytime = 0.1
	end
	self.effects = bit32.bor(self.effects, EF_MUZZLEFLASH)
	sound(self, CHAN_WEAPON, "shalrath/attack2.wav", 1, ATTN_NORM)
	missile = spawn()
	missile.owner = self
	missile.solid = SOLID_BBOX
	missile.movetype = MOVETYPE_FLYMISSILE
	setmodel(missile, "progs/v_spike.mdl")
	setsize(missile, vector.zero, vector.zero)
	missile.origin = self.origin + vector.create(0, 0, 10)
	missile.velocity = dir * 400
	missile.avelocity = vector.create(300, 300, 300)
	missile.nextthink = flytime + time()
	missile.think = ShalHome
	missile.enemy = self.enemy
	missile.touch = ShalMissileTouch
end

function ShalHome(self)
	local dir, vtemp
	vtemp = ent(self.enemy).origin + vector.create(0, 0, 10)
	if ent(self.enemy).health < 1 then
		remove(self)
		return
	end
	dir = normalize(vtemp - self.origin)
	if skill == 3 then
		self.velocity = dir * 350
	else
		self.velocity = dir * 250
	end
	self.nextthink = time() + 0.2
	self.think = ShalHome
end

function ShalMissileTouch(self, other)
	if other == self.owner then
		return
	end
	if other.classname == "monster_zombie" then
		T_Damage(other, self, self, 110)
	end
	T_RadiusDamage(self, self.owner, 40, world)
	sound(self, CHAN_WEAPON, "weapons/r_exp3.wav", 1, ATTN_NORM)
	temp_entity(TE_EXPLOSION, self.origin)
	self.velocity = vector.zero
	self.touch = SUB_Null
	setmodel(self, "progs/s_explod.spr")
	self.solid = SOLID_NOT
	s_explode1(self)
end

function monster_shalrath(self)
	if deathmatch ~= 0 then
		remove(self)
		return
	end
	self.solid = SOLID_SLIDEBOX
	self.movetype = MOVETYPE_STEP
	setmodel(self, "progs/shalrath.mdl")
	setsize(self, VEC_HULL2_MIN, VEC_HULL2_MAX)
	self.health = 400
	self.th_stand = shal_stand
	self.th_walk = shal_walk1
	self.th_run = shal_run1
	self.th_die = shalrath_die
	self.th_pain = shalrath_pain
	self.th_missile = shal_attack1
	self.think = walkmonster_start
	self.nextthink = time() + 0.1 + random() * 0.1
end
