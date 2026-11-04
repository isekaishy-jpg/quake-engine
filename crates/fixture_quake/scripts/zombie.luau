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
-- Ported from zombie.qc.

SPAWN_CRUCIFIED = 1

function zombie_stand1(self)
	animate(self, 0, zombie_stand2)
	ai_stand(self)
end

function zombie_stand2(self)
	animate(self, 1, zombie_stand3)
	ai_stand(self)
end

function zombie_stand3(self)
	animate(self, 2, zombie_stand4)
	ai_stand(self)
end

function zombie_stand4(self)
	animate(self, 3, zombie_stand5)
	ai_stand(self)
end

function zombie_stand5(self)
	animate(self, 4, zombie_stand6)
	ai_stand(self)
end

function zombie_stand6(self)
	animate(self, 5, zombie_stand7)
	ai_stand(self)
end

function zombie_stand7(self)
	animate(self, 6, zombie_stand8)
	ai_stand(self)
end

function zombie_stand8(self)
	animate(self, 7, zombie_stand9)
	ai_stand(self)
end

function zombie_stand9(self)
	animate(self, 8, zombie_stand10)
	ai_stand(self)
end

function zombie_stand10(self)
	animate(self, 9, zombie_stand11)
	ai_stand(self)
end

function zombie_stand11(self)
	animate(self, 10, zombie_stand12)
	ai_stand(self)
end

function zombie_stand12(self)
	animate(self, 11, zombie_stand13)
	ai_stand(self)
end

function zombie_stand13(self)
	animate(self, 12, zombie_stand14)
	ai_stand(self)
end

function zombie_stand14(self)
	animate(self, 13, zombie_stand15)
	ai_stand(self)
end

function zombie_stand15(self)
	animate(self, 14, zombie_stand1)
	ai_stand(self)
end

function zombie_cruc1(self)
	animate(self, 192, zombie_cruc2)
	if random() < 0.1 then
		sound(self, CHAN_VOICE, "zombie/idle_w2.wav", 1, ATTN_STATIC)
	end
end

function zombie_cruc2(self)
	animate(self, 193, zombie_cruc3)
	self.nextthink = time() + 0.1 + random() * 0.1
end

function zombie_cruc3(self)
	animate(self, 194, zombie_cruc4)
	self.nextthink = time() + 0.1 + random() * 0.1
end

function zombie_cruc4(self)
	animate(self, 195, zombie_cruc5)
	self.nextthink = time() + 0.1 + random() * 0.1
end

function zombie_cruc5(self)
	animate(self, 196, zombie_cruc6)
	self.nextthink = time() + 0.1 + random() * 0.1
end

function zombie_cruc6(self)
	animate(self, 197, zombie_cruc1)
	self.nextthink = time() + 0.1 + random() * 0.1
end

function zombie_walk1(self)
	animate(self, 15, zombie_walk2)
	ai_walk(self, 0)
end

function zombie_walk2(self)
	animate(self, 16, zombie_walk3)
	ai_walk(self, 2)
end

function zombie_walk3(self)
	animate(self, 17, zombie_walk4)
	ai_walk(self, 3)
end

function zombie_walk4(self)
	animate(self, 18, zombie_walk5)
	ai_walk(self, 2)
end

function zombie_walk5(self)
	animate(self, 19, zombie_walk6)
	ai_walk(self, 1)
end

function zombie_walk6(self)
	animate(self, 20, zombie_walk7)
	ai_walk(self, 0)
end

function zombie_walk7(self)
	animate(self, 21, zombie_walk8)
	ai_walk(self, 0)
end

function zombie_walk8(self)
	animate(self, 22, zombie_walk9)
	ai_walk(self, 0)
end

function zombie_walk9(self)
	animate(self, 23, zombie_walk10)
	ai_walk(self, 0)
end

function zombie_walk10(self)
	animate(self, 24, zombie_walk11)
	ai_walk(self, 0)
end

function zombie_walk11(self)
	animate(self, 25, zombie_walk12)
	ai_walk(self, 2)
end

function zombie_walk12(self)
	animate(self, 26, zombie_walk13)
	ai_walk(self, 2)
end

function zombie_walk13(self)
	animate(self, 27, zombie_walk14)
	ai_walk(self, 1)
end

function zombie_walk14(self)
	animate(self, 28, zombie_walk15)
	ai_walk(self, 0)
end

function zombie_walk15(self)
	animate(self, 29, zombie_walk16)
	ai_walk(self, 0)
end

function zombie_walk16(self)
	animate(self, 30, zombie_walk17)
	ai_walk(self, 0)
end

function zombie_walk17(self)
	animate(self, 31, zombie_walk18)
	ai_walk(self, 0)
end

function zombie_walk18(self)
	animate(self, 32, zombie_walk19)
	ai_walk(self, 0)
end

function zombie_walk19(self)
	animate(self, 33, zombie_walk1)
	ai_walk(self, 0)
	if random() < 0.2 then
		sound(self, CHAN_VOICE, "zombie/z_idle.wav", 1, ATTN_IDLE)
	end
end

function zombie_run1(self)
	animate(self, 34, zombie_run2)
	ai_run(self, 1)
	self.inpain = 0
end

function zombie_run2(self)
	animate(self, 35, zombie_run3)
	ai_run(self, 1)
end

function zombie_run3(self)
	animate(self, 36, zombie_run4)
	ai_run(self, 0)
end

function zombie_run4(self)
	animate(self, 37, zombie_run5)
	ai_run(self, 1)
end

function zombie_run5(self)
	animate(self, 38, zombie_run6)
	ai_run(self, 2)
end

function zombie_run6(self)
	animate(self, 39, zombie_run7)
	ai_run(self, 3)
end

function zombie_run7(self)
	animate(self, 40, zombie_run8)
	ai_run(self, 4)
end

function zombie_run8(self)
	animate(self, 41, zombie_run9)
	ai_run(self, 4)
end

function zombie_run9(self)
	animate(self, 42, zombie_run10)
	ai_run(self, 2)
end

function zombie_run10(self)
	animate(self, 43, zombie_run11)
	ai_run(self, 0)
end

function zombie_run11(self)
	animate(self, 44, zombie_run12)
	ai_run(self, 0)
end

function zombie_run12(self)
	animate(self, 45, zombie_run13)
	ai_run(self, 0)
end

function zombie_run13(self)
	animate(self, 46, zombie_run14)
	ai_run(self, 2)
end

function zombie_run14(self)
	animate(self, 47, zombie_run15)
	ai_run(self, 4)
end

function zombie_run15(self)
	animate(self, 48, zombie_run16)
	ai_run(self, 6)
end

function zombie_run16(self)
	animate(self, 49, zombie_run17)
	ai_run(self, 7)
end

function zombie_run17(self)
	animate(self, 50, zombie_run18)
	ai_run(self, 3)
end

function zombie_run18(self)
	animate(self, 51, zombie_run1)
	ai_run(self, 8)
	if random() < 0.2 then
		sound(self, CHAN_VOICE, "zombie/z_idle.wav", 1, ATTN_IDLE)
	end
	if random() > 0.8 then
		sound(self, CHAN_VOICE, "zombie/z_idle1.wav", 1, ATTN_IDLE)
	end
end

function ZombieGrenadeTouch(self, other)
	if other == self.owner then
		return
	end
	if other.takedamage ~= 0 then
		T_Damage(other, self, self.owner, 10)
		sound(self, CHAN_WEAPON, "zombie/z_hit.wav", 1, ATTN_NORM)
		remove(self)
		return
	end
	sound(self, CHAN_WEAPON, "zombie/z_miss.wav", 1, ATTN_NORM)
	-- bounce sound
	self.velocity = vector.zero
	self.avelocity = vector.zero
	self.touch = SUB_Remove
end

function ZombieFireGrenade(self, st)
	local missile, org
	sound(self, CHAN_WEAPON, "zombie/z_shot1.wav", 1, ATTN_NORM)
	missile = spawn()
	missile.owner = self
	missile.movetype = MOVETYPE_BOUNCE
	missile.solid = SOLID_BBOX
	-- calc org
	org = self.origin + st.x * v_forward + st.y * v_right + (st.z - 24) * v_up
	-- set missile speed
	makevectors(self.angles)
	missile.velocity = normalize(ent(self.enemy).origin - org)
	missile.velocity = missile.velocity * 600
	missile.velocity = with_z(missile.velocity, 200)
	missile.avelocity = vector.create(3000, 1000, 2000)
	missile.touch = ZombieGrenadeTouch
	-- set missile duration
	missile.nextthink = time() + 2.5
	missile.think = SUB_Remove
	setmodel(missile, "progs/zom_gib.mdl")
	setsize(missile, vector.zero, vector.zero)
	setorigin(missile, org)
end

function zombie_atta1(self)
	animate(self, 52, zombie_atta2)
	ai_face(self)
end

function zombie_atta2(self)
	animate(self, 53, zombie_atta3)
	ai_face(self)
end

function zombie_atta3(self)
	animate(self, 54, zombie_atta4)
	ai_face(self)
end

function zombie_atta4(self)
	animate(self, 55, zombie_atta5)
	ai_face(self)
end

function zombie_atta5(self)
	animate(self, 56, zombie_atta6)
	ai_face(self)
end

function zombie_atta6(self)
	animate(self, 57, zombie_atta7)
	ai_face(self)
end

function zombie_atta7(self)
	animate(self, 58, zombie_atta8)
	ai_face(self)
end

function zombie_atta8(self)
	animate(self, 59, zombie_atta9)
	ai_face(self)
end

function zombie_atta9(self)
	animate(self, 60, zombie_atta10)
	ai_face(self)
end

function zombie_atta10(self)
	animate(self, 61, zombie_atta11)
	ai_face(self)
end

function zombie_atta11(self)
	animate(self, 62, zombie_atta12)
	ai_face(self)
end

function zombie_atta12(self)
	animate(self, 63, zombie_atta13)
	ai_face(self)
end

function zombie_atta13(self)
	animate(self, 64, zombie_run1)
	ai_face(self)
	ZombieFireGrenade(self, vector.create(-10, -22, 30))
end

function zombie_attb1(self)
	animate(self, 65, zombie_attb2)
	ai_face(self)
end

function zombie_attb2(self)
	animate(self, 66, zombie_attb3)
	ai_face(self)
end

function zombie_attb3(self)
	animate(self, 67, zombie_attb4)
	ai_face(self)
end

function zombie_attb4(self)
	animate(self, 68, zombie_attb5)
	ai_face(self)
end

function zombie_attb5(self)
	animate(self, 69, zombie_attb6)
	ai_face(self)
end

function zombie_attb6(self)
	animate(self, 70, zombie_attb7)
	ai_face(self)
end

function zombie_attb7(self)
	animate(self, 71, zombie_attb8)
	ai_face(self)
end

function zombie_attb8(self)
	animate(self, 72, zombie_attb9)
	ai_face(self)
end

function zombie_attb9(self)
	animate(self, 73, zombie_attb10)
	ai_face(self)
end

function zombie_attb10(self)
	animate(self, 74, zombie_attb11)
	ai_face(self)
end

function zombie_attb11(self)
	animate(self, 75, zombie_attb12)
	ai_face(self)
end

function zombie_attb12(self)
	animate(self, 76, zombie_attb13)
	ai_face(self)
end

function zombie_attb13(self)
	animate(self, 77, zombie_attb14)
	ai_face(self)
end

function zombie_attb14(self)
	animate(self, 77, zombie_run1)
	ai_face(self)
	ZombieFireGrenade(self, vector.create(-10, -24, 29))
end

function zombie_attc1(self)
	animate(self, 79, zombie_attc2)
	ai_face(self)
end

function zombie_attc2(self)
	animate(self, 80, zombie_attc3)
	ai_face(self)
end

function zombie_attc3(self)
	animate(self, 81, zombie_attc4)
	ai_face(self)
end

function zombie_attc4(self)
	animate(self, 82, zombie_attc5)
	ai_face(self)
end

function zombie_attc5(self)
	animate(self, 83, zombie_attc6)
	ai_face(self)
end

function zombie_attc6(self)
	animate(self, 84, zombie_attc7)
	ai_face(self)
end

function zombie_attc7(self)
	animate(self, 85, zombie_attc8)
	ai_face(self)
end

function zombie_attc8(self)
	animate(self, 86, zombie_attc9)
	ai_face(self)
end

function zombie_attc9(self)
	animate(self, 87, zombie_attc10)
	ai_face(self)
end

function zombie_attc10(self)
	animate(self, 88, zombie_attc11)
	ai_face(self)
end

function zombie_attc11(self)
	animate(self, 89, zombie_attc12)
	ai_face(self)
end

function zombie_attc12(self)
	animate(self, 90, zombie_run1)
	ai_face(self)
	ZombieFireGrenade(self, vector.create(-12, -19, 29))
end

function zombie_missile(self)
	local r
	r = random()
	if r < 0.3 then
		zombie_atta1(self)
	elseif r < 0.6 then
		zombie_attb1(self)
	else
		zombie_attc1(self)
	end
end

function zombie_paina1(self)
	animate(self, 91, zombie_paina2)
	sound(self, CHAN_VOICE, "zombie/z_pain.wav", 1, ATTN_NORM)
end

function zombie_paina2(self)
	animate(self, 92, zombie_paina3)
	ai_painforward(self, 3)
end

function zombie_paina3(self)
	animate(self, 93, zombie_paina4)
	ai_painforward(self, 1)
end

function zombie_paina4(self)
	animate(self, 94, zombie_paina5)
	ai_pain(self, 1)
end

function zombie_paina5(self)
	animate(self, 95, zombie_paina6)
	ai_pain(self, 3)
end

function zombie_paina6(self)
	animate(self, 96, zombie_paina7)
	ai_pain(self, 1)
end

function zombie_paina7(self)
	animate(self, 97, zombie_paina8)
end

function zombie_paina8(self)
	animate(self, 98, zombie_paina9)
end

function zombie_paina9(self)
	animate(self, 99, zombie_paina10)
end

function zombie_paina10(self)
	animate(self, 100, zombie_paina11)
end

function zombie_paina11(self)
	animate(self, 101, zombie_paina12)
end

function zombie_paina12(self)
	animate(self, 102, zombie_run1)
end

function zombie_painb1(self)
	animate(self, 103, zombie_painb2)
	sound(self, CHAN_VOICE, "zombie/z_pain1.wav", 1, ATTN_NORM)
end

function zombie_painb2(self)
	animate(self, 104, zombie_painb3)
	ai_pain(self, 2)
end

function zombie_painb3(self)
	animate(self, 105, zombie_painb4)
	ai_pain(self, 8)
end

function zombie_painb4(self)
	animate(self, 106, zombie_painb5)
	ai_pain(self, 6)
end

function zombie_painb5(self)
	animate(self, 107, zombie_painb6)
	ai_pain(self, 2)
end

function zombie_painb6(self)
	animate(self, 108, zombie_painb7)
end

function zombie_painb7(self)
	animate(self, 109, zombie_painb8)
end

function zombie_painb8(self)
	animate(self, 110, zombie_painb9)
end

function zombie_painb9(self)
	animate(self, 111, zombie_painb10)
	sound(self, CHAN_BODY, "zombie/z_fall.wav", 1, ATTN_NORM)
end

function zombie_painb10(self)
	animate(self, 112, zombie_painb11)
end

function zombie_painb11(self)
	animate(self, 113, zombie_painb12)
end

function zombie_painb12(self)
	animate(self, 114, zombie_painb13)
end

function zombie_painb13(self)
	animate(self, 115, zombie_painb14)
end

function zombie_painb14(self)
	animate(self, 116, zombie_painb15)
end

function zombie_painb15(self)
	animate(self, 117, zombie_painb16)
end

function zombie_painb16(self)
	animate(self, 118, zombie_painb17)
end

function zombie_painb17(self)
	animate(self, 119, zombie_painb18)
end

function zombie_painb18(self)
	animate(self, 120, zombie_painb19)
end

function zombie_painb19(self)
	animate(self, 121, zombie_painb20)
end

function zombie_painb20(self)
	animate(self, 122, zombie_painb21)
end

function zombie_painb21(self)
	animate(self, 123, zombie_painb22)
end

function zombie_painb22(self)
	animate(self, 124, zombie_painb23)
end

function zombie_painb23(self)
	animate(self, 125, zombie_painb24)
end

function zombie_painb24(self)
	animate(self, 126, zombie_painb25)
end

function zombie_painb25(self)
	animate(self, 127, zombie_painb26)
	ai_painforward(self, 1)
end

function zombie_painb26(self)
	animate(self, 128, zombie_painb27)
end

function zombie_painb27(self)
	animate(self, 129, zombie_painb28)
end

function zombie_painb28(self)
	animate(self, 130, zombie_run1)
end

function zombie_painc1(self)
	animate(self, 131, zombie_painc2)
	sound(self, CHAN_VOICE, "zombie/z_pain1.wav", 1, ATTN_NORM)
end

function zombie_painc2(self)
	animate(self, 132, zombie_painc3)
end

function zombie_painc3(self)
	animate(self, 133, zombie_painc4)
	ai_pain(self, 3)
end

function zombie_painc4(self)
	animate(self, 134, zombie_painc5)
	ai_pain(self, 1)
end

function zombie_painc5(self)
	animate(self, 135, zombie_painc6)
end

function zombie_painc6(self)
	animate(self, 136, zombie_painc7)
end

function zombie_painc7(self)
	animate(self, 137, zombie_painc8)
end

function zombie_painc8(self)
	animate(self, 138, zombie_painc9)
end

function zombie_painc9(self)
	animate(self, 139, zombie_painc10)
end

function zombie_painc10(self)
	animate(self, 140, zombie_painc11)
end

function zombie_painc11(self)
	animate(self, 141, zombie_painc12)
	ai_painforward(self, 1)
end

function zombie_painc12(self)
	animate(self, 142, zombie_painc13)
	ai_painforward(self, 1)
end

function zombie_painc13(self)
	animate(self, 143, zombie_painc14)
end

function zombie_painc14(self)
	animate(self, 144, zombie_painc15)
end

function zombie_painc15(self)
	animate(self, 145, zombie_painc16)
end

function zombie_painc16(self)
	animate(self, 146, zombie_painc17)
end

function zombie_painc17(self)
	animate(self, 147, zombie_painc18)
end

function zombie_painc18(self)
	animate(self, 148, zombie_run1)
end

function zombie_paind1(self)
	animate(self, 149, zombie_paind2)
	sound(self, CHAN_VOICE, "zombie/z_pain.wav", 1, ATTN_NORM)
end

function zombie_paind2(self)
	animate(self, 150, zombie_paind3)
end

function zombie_paind3(self)
	animate(self, 151, zombie_paind4)
end

function zombie_paind4(self)
	animate(self, 152, zombie_paind5)
end

function zombie_paind5(self)
	animate(self, 153, zombie_paind6)
end

function zombie_paind6(self)
	animate(self, 154, zombie_paind7)
end

function zombie_paind7(self)
	animate(self, 155, zombie_paind8)
end

function zombie_paind8(self)
	animate(self, 156, zombie_paind9)
end

function zombie_paind9(self)
	animate(self, 157, zombie_paind10)
	ai_pain(self, 1)
end

function zombie_paind10(self)
	animate(self, 158, zombie_paind11)
end

function zombie_paind11(self)
	animate(self, 159, zombie_paind12)
end

function zombie_paind12(self)
	animate(self, 160, zombie_paind13)
end

function zombie_paind13(self)
	animate(self, 161, zombie_run1)
end

function zombie_paine1(self)
	animate(self, 162, zombie_paine2)
	sound(self, CHAN_VOICE, "zombie/z_pain.wav", 1, ATTN_NORM)
	self.health = 60
end

function zombie_paine2(self)
	animate(self, 163, zombie_paine3)
	ai_pain(self, 8)
end

function zombie_paine3(self)
	animate(self, 164, zombie_paine4)
	ai_pain(self, 5)
end

function zombie_paine4(self)
	animate(self, 165, zombie_paine5)
	ai_pain(self, 3)
end

function zombie_paine5(self)
	animate(self, 166, zombie_paine6)
	ai_pain(self, 1)
end

function zombie_paine6(self)
	animate(self, 167, zombie_paine7)
	ai_pain(self, 2)
end

function zombie_paine7(self)
	animate(self, 168, zombie_paine8)
	ai_pain(self, 1)
end

function zombie_paine8(self)
	animate(self, 169, zombie_paine9)
	ai_pain(self, 1)
end

function zombie_paine9(self)
	animate(self, 170, zombie_paine10)
	ai_pain(self, 2)
end

function zombie_paine10(self)
	animate(self, 171, zombie_paine11)
	sound(self, CHAN_BODY, "zombie/z_fall.wav", 1, ATTN_NORM)
	self.solid = SOLID_NOT
end

function zombie_paine11(self)
	animate(self, 172, zombie_paine12)
	self.nextthink = self.nextthink + 5
	self.health = 60
end

function zombie_paine12(self)
	animate(self, 173, zombie_paine13)
	-- see if ok to stand up
	self.health = 60
	sound(self, CHAN_VOICE, "zombie/z_idle.wav", 1, ATTN_IDLE)
	self.solid = SOLID_SLIDEBOX
	if not walkmove(self, 0, 0) then
		self.think = zombie_paine11
		self.solid = SOLID_NOT
		return
	end
end

function zombie_paine13(self)
	animate(self, 174, zombie_paine14)
end

function zombie_paine14(self)
	animate(self, 175, zombie_paine15)
end

function zombie_paine15(self)
	animate(self, 176, zombie_paine16)
end

function zombie_paine16(self)
	animate(self, 177, zombie_paine17)
end

function zombie_paine17(self)
	animate(self, 178, zombie_paine18)
end

function zombie_paine18(self)
	animate(self, 179, zombie_paine19)
end

function zombie_paine19(self)
	animate(self, 180, zombie_paine20)
end

function zombie_paine20(self)
	animate(self, 181, zombie_paine21)
end

function zombie_paine21(self)
	animate(self, 182, zombie_paine22)
end

function zombie_paine22(self)
	animate(self, 183, zombie_paine23)
end

function zombie_paine23(self)
	animate(self, 184, zombie_paine24)
end

function zombie_paine24(self)
	animate(self, 185, zombie_paine25)
end

function zombie_paine25(self)
	animate(self, 186, zombie_paine26)
	ai_painforward(self, 5)
end

function zombie_paine26(self)
	animate(self, 187, zombie_paine27)
	ai_painforward(self, 3)
end

function zombie_paine27(self)
	animate(self, 188, zombie_paine28)
	ai_painforward(self, 1)
end

function zombie_paine28(self)
	animate(self, 189, zombie_paine29)
	ai_pain(self, 1)
end

function zombie_paine29(self)
	animate(self, 190, zombie_paine30)
end

function zombie_paine30(self)
	animate(self, 191, zombie_run1)
end

function zombie_die(self)
	sound(self, CHAN_VOICE, "zombie/z_gib.wav", 1, ATTN_NORM)
	ThrowHead(self, "progs/h_zombie.mdl", self.health)
	ThrowGib(self, "progs/gib1.mdl", self.health)
	ThrowGib(self, "progs/gib2.mdl", self.health)
	ThrowGib(self, "progs/gib3.mdl", self.health)
end

function zombie_pain(self, attacker, take)
	local r
	self.health = 60
	-- allways reset health
	if take < 9 then
		return
	end
	if self.inpain == 2 then
		return
	end
	if take >= 25 then
		self.inpain = 2
		zombie_paine1(self)
		return
	end
	if self.inpain ~= 0 then
		-- if hit again in next gre seconds while not in pain frames, definately drop
		self.pain_finished = time() + 3
		return
		-- currently going through an animation, don't change
	end
	if self.pain_finished > time() then
		-- hit again, so drop down
		self.inpain = 2
		zombie_paine1(self)
		return
	end
	self.inpain = 1
	r = random()
	if r < 0.25 then
		zombie_paina1(self)
	elseif r < 0.5 then
		zombie_painb1(self)
	elseif r < 0.75 then
		zombie_painc1(self)
	else
		zombie_paind1(self)
	end
end

function monster_zombie(self)
	if deathmatch ~= 0 then
		remove(self)
		return
	end
	self.solid = SOLID_SLIDEBOX
	self.movetype = MOVETYPE_STEP
	setmodel(self, "progs/zombie.mdl")
	setsize(self, vector.create(-16, -16, -24), vector.create(16, 16, 40))
	self.health = 60
	self.th_stand = zombie_stand1
	self.th_walk = zombie_walk1
	self.th_run = zombie_run1
	self.th_pain = zombie_pain
	self.th_die = zombie_die
	self.th_missile = zombie_missile
	if bit32.band(self.spawnflags, SPAWN_CRUCIFIED) ~= 0 then
		self.movetype = MOVETYPE_NONE
		zombie_cruc1(self)
	else
		walkmonster_start(self)
	end
end
