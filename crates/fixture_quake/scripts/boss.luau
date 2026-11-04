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
-- Ported from boss.qc.

function boss_face(self)
	-- go for another player if multi player
	if ent(self.enemy).health <= 0 or random() < 0.02 then
		self.enemy = find_entity(self.enemy, "classname", "player")
		if not exists(self.enemy) then
			self.enemy = find_entity(self.enemy, "classname", "player")
		end
	end
	ai_face(self)
end

function boss_rise1(self)
	animate(self, 0, boss_rise2)
	sound(self, CHAN_WEAPON, "boss1/out1.wav", 1, ATTN_NORM)
end

function boss_rise2(self)
	animate(self, 1, boss_rise3)
	sound(self, CHAN_VOICE, "boss1/sight1.wav", 1, ATTN_NORM)
end

function boss_rise3(self)
	animate(self, 2, boss_rise4)
end

function boss_rise4(self)
	animate(self, 3, boss_rise5)
end

function boss_rise5(self)
	animate(self, 4, boss_rise6)
end

function boss_rise6(self)
	animate(self, 5, boss_rise7)
end

function boss_rise7(self)
	animate(self, 6, boss_rise8)
end

function boss_rise8(self)
	animate(self, 7, boss_rise9)
end

function boss_rise9(self)
	animate(self, 8, boss_rise10)
end

function boss_rise10(self)
	animate(self, 9, boss_rise11)
end

function boss_rise11(self)
	animate(self, 10, boss_rise12)
end

function boss_rise12(self)
	animate(self, 11, boss_rise13)
end

function boss_rise13(self)
	animate(self, 12, boss_rise14)
end

function boss_rise14(self)
	animate(self, 13, boss_rise15)
end

function boss_rise15(self)
	animate(self, 14, boss_rise16)
end

function boss_rise16(self)
	animate(self, 15, boss_rise17)
end

function boss_rise17(self)
	animate(self, 16, boss_missile1)
end

function boss_idle1(self)
	animate(self, 17, boss_idle2)
	-- look for other players
end

function boss_idle2(self)
	animate(self, 18, boss_idle3)
	boss_face(self)
end

function boss_idle3(self)
	animate(self, 19, boss_idle4)
	boss_face(self)
end

function boss_idle4(self)
	animate(self, 20, boss_idle5)
	boss_face(self)
end

function boss_idle5(self)
	animate(self, 21, boss_idle6)
	boss_face(self)
end

function boss_idle6(self)
	animate(self, 22, boss_idle7)
	boss_face(self)
end

function boss_idle7(self)
	animate(self, 23, boss_idle8)
	boss_face(self)
end

function boss_idle8(self)
	animate(self, 24, boss_idle9)
	boss_face(self)
end

function boss_idle9(self)
	animate(self, 25, boss_idle10)
	boss_face(self)
end

function boss_idle10(self)
	animate(self, 26, boss_idle11)
	boss_face(self)
end

function boss_idle11(self)
	animate(self, 27, boss_idle12)
	boss_face(self)
end

function boss_idle12(self)
	animate(self, 28, boss_idle13)
	boss_face(self)
end

function boss_idle13(self)
	animate(self, 29, boss_idle14)
	boss_face(self)
end

function boss_idle14(self)
	animate(self, 30, boss_idle15)
	boss_face(self)
end

function boss_idle15(self)
	animate(self, 31, boss_idle16)
	boss_face(self)
end

function boss_idle16(self)
	animate(self, 32, boss_idle17)
	boss_face(self)
end

function boss_idle17(self)
	animate(self, 33, boss_idle18)
	boss_face(self)
end

function boss_idle18(self)
	animate(self, 34, boss_idle19)
	boss_face(self)
end

function boss_idle19(self)
	animate(self, 35, boss_idle20)
	boss_face(self)
end

function boss_idle20(self)
	animate(self, 36, boss_idle21)
	boss_face(self)
end

function boss_idle21(self)
	animate(self, 37, boss_idle22)
	boss_face(self)
end

function boss_idle22(self)
	animate(self, 38, boss_idle23)
	boss_face(self)
end

function boss_idle23(self)
	animate(self, 39, boss_idle24)
	boss_face(self)
end

function boss_idle24(self)
	animate(self, 40, boss_idle25)
	boss_face(self)
end

function boss_idle25(self)
	animate(self, 41, boss_idle26)
	boss_face(self)
end

function boss_idle26(self)
	animate(self, 42, boss_idle27)
	boss_face(self)
end

function boss_idle27(self)
	animate(self, 43, boss_idle28)
	boss_face(self)
end

function boss_idle28(self)
	animate(self, 44, boss_idle29)
	boss_face(self)
end

function boss_idle29(self)
	animate(self, 45, boss_idle30)
	boss_face(self)
end

function boss_idle30(self)
	animate(self, 46, boss_idle31)
	boss_face(self)
end

function boss_idle31(self)
	animate(self, 47, boss_idle1)
	boss_face(self)
end

function boss_missile1(self)
	animate(self, 57, boss_missile2)
	boss_face(self)
end

function boss_missile2(self)
	animate(self, 58, boss_missile3)
	boss_face(self)
end

function boss_missile3(self)
	animate(self, 59, boss_missile4)
	boss_face(self)
end

function boss_missile4(self)
	animate(self, 60, boss_missile5)
	boss_face(self)
end

function boss_missile5(self)
	animate(self, 61, boss_missile6)
	boss_face(self)
end

function boss_missile6(self)
	animate(self, 62, boss_missile7)
	boss_face(self)
end

function boss_missile7(self)
	animate(self, 63, boss_missile8)
	boss_face(self)
end

function boss_missile8(self)
	animate(self, 64, boss_missile9)
	boss_face(self)
end

function boss_missile9(self)
	animate(self, 65, boss_missile10)
	boss_missile(self, vector.create(100, 100, 200))
end

function boss_missile10(self)
	animate(self, 66, boss_missile11)
	boss_face(self)
end

function boss_missile11(self)
	animate(self, 67, boss_missile12)
	boss_face(self)
end

function boss_missile12(self)
	animate(self, 68, boss_missile13)
	boss_face(self)
end

function boss_missile13(self)
	animate(self, 69, boss_missile14)
	boss_face(self)
end

function boss_missile14(self)
	animate(self, 70, boss_missile15)
	boss_face(self)
end

function boss_missile15(self)
	animate(self, 71, boss_missile16)
	boss_face(self)
end

function boss_missile16(self)
	animate(self, 72, boss_missile17)
	boss_face(self)
end

function boss_missile17(self)
	animate(self, 73, boss_missile18)
	boss_face(self)
end

function boss_missile18(self)
	animate(self, 74, boss_missile19)
	boss_face(self)
end

function boss_missile19(self)
	animate(self, 75, boss_missile20)
	boss_face(self)
end

function boss_missile20(self)
	animate(self, 76, boss_missile21)
	boss_missile(self, vector.create(100, -100, 200))
end

function boss_missile21(self)
	animate(self, 77, boss_missile22)
	boss_face(self)
end

function boss_missile22(self)
	animate(self, 78, boss_missile23)
	boss_face(self)
end

function boss_missile23(self)
	animate(self, 79, boss_missile1)
	boss_face(self)
end

function boss_shocka1(self)
	animate(self, 80, boss_shocka2)
end

function boss_shocka2(self)
	animate(self, 81, boss_shocka3)
end

function boss_shocka3(self)
	animate(self, 82, boss_shocka4)
end

function boss_shocka4(self)
	animate(self, 83, boss_shocka5)
end

function boss_shocka5(self)
	animate(self, 84, boss_shocka6)
end

function boss_shocka6(self)
	animate(self, 85, boss_shocka7)
end

function boss_shocka7(self)
	animate(self, 86, boss_shocka8)
end

function boss_shocka8(self)
	animate(self, 87, boss_shocka9)
end

function boss_shocka9(self)
	animate(self, 88, boss_shocka10)
end

function boss_shocka10(self)
	animate(self, 89, boss_missile1)
end

function boss_shockb1(self)
	animate(self, 90, boss_shockb2)
end

function boss_shockb2(self)
	animate(self, 91, boss_shockb3)
end

function boss_shockb3(self)
	animate(self, 92, boss_shockb4)
end

function boss_shockb4(self)
	animate(self, 93, boss_shockb5)
end

function boss_shockb5(self)
	animate(self, 94, boss_shockb6)
end

function boss_shockb6(self)
	animate(self, 95, boss_shockb7)
end

function boss_shockb7(self)
	animate(self, 90, boss_shockb8)
end

function boss_shockb8(self)
	animate(self, 91, boss_shockb9)
end

function boss_shockb9(self)
	animate(self, 92, boss_shockb10)
end

function boss_shockb10(self)
	animate(self, 93, boss_missile1)
end

function boss_shockc1(self)
	animate(self, 96, boss_shockc2)
end

function boss_shockc2(self)
	animate(self, 97, boss_shockc3)
end

function boss_shockc3(self)
	animate(self, 98, boss_shockc4)
end

function boss_shockc4(self)
	animate(self, 99, boss_shockc5)
end

function boss_shockc5(self)
	animate(self, 100, boss_shockc6)
end

function boss_shockc6(self)
	animate(self, 101, boss_shockc7)
end

function boss_shockc7(self)
	animate(self, 102, boss_shockc8)
end

function boss_shockc8(self)
	animate(self, 103, boss_shockc9)
end

function boss_shockc9(self)
	animate(self, 104, boss_shockc10)
end

function boss_shockc10(self)
	animate(self, 105, boss_death1)
end

function boss_death1(self)
	animate(self, 48, boss_death2)
	sound(self, CHAN_VOICE, "boss1/death.wav", 1, ATTN_NORM)
end

function boss_death2(self)
	animate(self, 49, boss_death3)
end

function boss_death3(self)
	animate(self, 50, boss_death4)
end

function boss_death4(self)
	animate(self, 51, boss_death5)
end

function boss_death5(self)
	animate(self, 52, boss_death6)
end

function boss_death6(self)
	animate(self, 53, boss_death7)
end

function boss_death7(self)
	animate(self, 54, boss_death8)
end

function boss_death8(self)
	animate(self, 55, boss_death9)
end

function boss_death9(self)
	animate(self, 56, boss_death10)
	sound(self, CHAN_BODY, "boss1/out1.wav", 1, ATTN_NORM)
	temp_entity(TE_LAVASPLASH, self.origin)
end

function boss_death10(self)
	animate(self, 56, boss_death10)
	killed_monsters = killed_monsters + 1
	-- FIXME: reliable broadcast
	SUB_UseTargets(self)
	remove(self)
end

function boss_missile(self, p)
	local vec = vector.zero
	local offang, org, d, t
	offang = vectoangles(ent(self.enemy).origin - self.origin)
	makevectors(offang)
	org = self.origin + p.x * v_forward + p.y * v_right + p.z * vector.create(0, 0, 1)
	-- lead the player on hard mode
	if skill > 1 then
		t = vector.magnitude(ent(self.enemy).origin - org) / 300
		vec = ent(self.enemy).velocity
		vec = with_z(vec, 0)
		d = ent(self.enemy).origin + t * vec
	else
		d = ent(self.enemy).origin
	end
	vec = normalize(d - org)
	launch_spike(self, org, vec)
	setmodel(newmis, "progs/lavaball.mdl")
	newmis.avelocity = vector.create(200, 100, 300)
	setsize(newmis, VEC_ORIGIN, VEC_ORIGIN)
	newmis.velocity = vec * 300
	newmis.touch = T_MissileTouch
	-- rocket explosion
	sound(self, CHAN_WEAPON, "boss1/throw.wav", 1, ATTN_NORM)
	-- check for dead enemy
	if ent(self.enemy).health <= 0 then
		boss_idle1(self)
	end
end

function boss_awake(self)
	self.solid = SOLID_SLIDEBOX
	self.movetype = MOVETYPE_STEP
	self.takedamage = DAMAGE_NO
	setmodel(self, "progs/boss.mdl")
	setsize(self, vector.create(-128, -128, -24), vector.create(128, 128, 256))
	if skill == 0 then
		self.health = 1
	else
		self.health = 3
	end
	self.enemy = activator
	temp_entity(TE_LAVASPLASH, self.origin)
	self.yaw_speed = 20
	boss_rise1(self)
end

function monster_boss(self)
	if deathmatch ~= 0 then
		remove(self)
		return
	end
	total_monsters = total_monsters + 1
	self.use = boss_awake
end

le1 = nil
le2 = nil

lightning_end = 0

function lightning_fire(self)
	local p1 = vector.zero
	local p2 = vector.zero
	if time() >= lightning_end then
		-- done here, put the terminals back up
		self = le1
		door_go_down(self)
		self = le2
		door_go_down(self)
		return
	end
	p1 = (le1.mins + le1.maxs) * 0.5
	p1 = with_z(p1, le1.absmin.z - 16)
	p2 = (le2.mins + le2.maxs) * 0.5
	p2 = with_z(p2, le2.absmin.z - 16)
	-- compensate for length of bolt
	p2 = p2 - normalize(p2 - p1) * 100
	self.nextthink = time() + 0.1
	self.think = lightning_fire
	temp_entity(TE_LIGHTNING3, p1, p2, world)
end

function lightning_use(self)
	if lightning_end >= time() + 1 then
		return
	end
	le1 = find_entity(world, "target", "lightning")
	le2 = find_entity(le1, "target", "lightning")
	if not exists(le1) or not exists(le2) then
		return
	end
	if
		(le1.state ~= STATE_TOP and le1.state ~= STATE_BOTTOM)
		or (le2.state ~= STATE_TOP and le2.state ~= STATE_BOTTOM)
		or le1.state ~= le2.state
	then
		return
	end
	le1.nextthink = -1
	le2.nextthink = -1
	lightning_end = time() + 1
	sound(self, CHAN_VOICE, "misc/power.wav", 1, ATTN_NORM)
	lightning_fire(self)
	-- advance the boss pain if down
	self = find_entity(world, "classname", "monster_boss")
	if not exists(self) then
		return
	end
	self.enemy = activator
	if le1.state == STATE_TOP and self.health > 0 then
		sound(self, CHAN_VOICE, "boss1/pain.wav", 1, ATTN_NORM)
		self.health = self.health - 1
		if self.health >= 2 then
			boss_shocka1(self)
		elseif self.health == 1 then
			boss_shockb1(self)
		elseif self.health == 0 then
			boss_shockc1(self)
		end
	end
end

function event_lightning(self)
	self.use = lightning_use
end
