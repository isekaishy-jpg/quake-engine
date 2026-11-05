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
-- Ported from oldone.qc.

shub = nil

function old_idle1(self)
	animate(self, 0, old_idle2)
end

function old_idle2(self)
	animate(self, 1, old_idle3)
end

function old_idle3(self)
	animate(self, 2, old_idle4)
end

function old_idle4(self)
	animate(self, 3, old_idle5)
end

function old_idle5(self)
	animate(self, 4, old_idle6)
end

function old_idle6(self)
	animate(self, 5, old_idle7)
end

function old_idle7(self)
	animate(self, 6, old_idle8)
end

function old_idle8(self)
	animate(self, 7, old_idle9)
end

function old_idle9(self)
	animate(self, 8, old_idle10)
end

function old_idle10(self)
	animate(self, 9, old_idle11)
end

function old_idle11(self)
	animate(self, 10, old_idle12)
end

function old_idle12(self)
	animate(self, 11, old_idle13)
end

function old_idle13(self)
	animate(self, 12, old_idle14)
end

function old_idle14(self)
	animate(self, 13, old_idle15)
end

function old_idle15(self)
	animate(self, 14, old_idle16)
end

function old_idle16(self)
	animate(self, 15, old_idle17)
end

function old_idle17(self)
	animate(self, 16, old_idle18)
end

function old_idle18(self)
	animate(self, 17, old_idle19)
end

function old_idle19(self)
	animate(self, 18, old_idle20)
end

function old_idle20(self)
	animate(self, 19, old_idle21)
end

function old_idle21(self)
	animate(self, 20, old_idle22)
end

function old_idle22(self)
	animate(self, 21, old_idle23)
end

function old_idle23(self)
	animate(self, 22, old_idle24)
end

function old_idle24(self)
	animate(self, 23, old_idle25)
end

function old_idle25(self)
	animate(self, 24, old_idle26)
end

function old_idle26(self)
	animate(self, 25, old_idle27)
end

function old_idle27(self)
	animate(self, 26, old_idle28)
end

function old_idle28(self)
	animate(self, 27, old_idle29)
end

function old_idle29(self)
	animate(self, 28, old_idle30)
end

function old_idle30(self)
	animate(self, 29, old_idle31)
end

function old_idle31(self)
	animate(self, 30, old_idle32)
end

function old_idle32(self)
	animate(self, 31, old_idle33)
end

function old_idle33(self)
	animate(self, 32, old_idle34)
end

function old_idle34(self)
	animate(self, 33, old_idle35)
end

function old_idle35(self)
	animate(self, 34, old_idle36)
end

function old_idle36(self)
	animate(self, 35, old_idle37)
end

function old_idle37(self)
	animate(self, 36, old_idle38)
end

function old_idle38(self)
	animate(self, 37, old_idle39)
end

function old_idle39(self)
	animate(self, 38, old_idle40)
end

function old_idle40(self)
	animate(self, 39, old_idle41)
end

function old_idle41(self)
	animate(self, 40, old_idle42)
end

function old_idle42(self)
	animate(self, 41, old_idle43)
end

function old_idle43(self)
	animate(self, 42, old_idle44)
end

function old_idle44(self)
	animate(self, 43, old_idle45)
end

function old_idle45(self)
	animate(self, 44, old_idle46)
end

function old_idle46(self)
	animate(self, 45, old_idle1)
end

function old_thrash1(self)
	animate(self, 46, old_thrash2)
	engine.lightstyle(0, "m")
end

function old_thrash2(self)
	animate(self, 47, old_thrash3)
	engine.lightstyle(0, "k")
end

function old_thrash3(self)
	animate(self, 48, old_thrash4)
	engine.lightstyle(0, "k")
end

function old_thrash4(self)
	animate(self, 49, old_thrash5)
	engine.lightstyle(0, "i")
end

function old_thrash5(self)
	animate(self, 50, old_thrash6)
	engine.lightstyle(0, "g")
end

function old_thrash6(self)
	animate(self, 51, old_thrash7)
	engine.lightstyle(0, "e")
end

function old_thrash7(self)
	animate(self, 52, old_thrash8)
	engine.lightstyle(0, "c")
end

function old_thrash8(self)
	animate(self, 53, old_thrash9)
	engine.lightstyle(0, "a")
end

function old_thrash9(self)
	animate(self, 54, old_thrash10)
	engine.lightstyle(0, "c")
end

function old_thrash10(self)
	animate(self, 55, old_thrash11)
	engine.lightstyle(0, "e")
end

function old_thrash11(self)
	animate(self, 56, old_thrash12)
	engine.lightstyle(0, "g")
end

function old_thrash12(self)
	animate(self, 58, old_thrash13)
	engine.lightstyle(0, "i")
end

function old_thrash13(self)
	animate(self, 59, old_thrash14)
	engine.lightstyle(0, "k")
end

function old_thrash14(self)
	animate(self, 60, old_thrash15)
	engine.lightstyle(0, "m")
end

function old_thrash15(self)
	animate(self, 61, old_thrash16)
	engine.lightstyle(0, "m")
	self.cnt = self.cnt + 1
	if self.cnt ~= 3 then
		self.think = old_thrash1
	end
end

function old_thrash16(self)
	animate(self, 62, old_thrash17)
	engine.lightstyle(0, "g")
end

function old_thrash17(self)
	animate(self, 63, old_thrash18)
	engine.lightstyle(0, "c")
end

function old_thrash18(self)
	animate(self, 64, old_thrash19)
	engine.lightstyle(0, "b")
end

function old_thrash19(self)
	animate(self, 65, old_thrash20)
	engine.lightstyle(0, "a")
end

function old_thrash20(self)
	animate(self, 66, old_thrash20)
	finale_4(self)
end

function finale_1(self)
	local pos, pl, timer
	intermission_exittime = time() + 10000000
	-- never allow exit
	intermission_running = 1
	-- find the intermission spot
	pos = find_entity(world, "classname", "info_intermission")
	if not exists(pos) then
		error("no info_intermission")
	end
	pl = find_entity(world, "classname", "misc_teleporttrain")
	if not exists(pl) then
		error("no teleporttrain")
	end
	remove(pl)
	finale("")
	pl = find_entity(world, "classname", "player")
	while exists(pl) do
		pl.map = self.map
		hold_at_camera(pl, pos)
		pl = find_entity(pl, "classname", "player")
	end
	-- make fake versions of all players as standins, and move the real
	-- players to the intermission spot
	-- wait for 1 second
	timer = spawn()
	timer.nextthink = time() + 1
	timer.think = finale_2
end

function finale_2(self)
	local o
	-- start a teleport splash inside shub
	o = shub.origin - vector.create(0, 100, 0)
	temp_entity(TE_TELEPORT, o)
	sound(shub, CHAN_VOICE, "misc/r_tele1.wav", 1, ATTN_NORM)
	self.nextthink = time() + 2
	self.think = finale_3
end

function finale_3(self)
	-- start shub thrashing wildly
	shub.think = old_thrash1
	sound(shub, CHAN_VOICE, "boss2/death.wav", 1, ATTN_NORM)
	engine.lightstyle(0, "abcdefghijklmlkjihgfedcb")
end

function finale_4(self)
	local oldo, x, y, z, r, n
	-- throw tons of meat chunks
	sound(self, CHAN_VOICE, "boss2/pop2.wav", 1, ATTN_NORM)
	oldo = self.origin
	z = 16
	while z <= 144 do
		x = -64
		while x <= 64 do
			y = -64
			while y <= 64 do
				self.origin = with_x(self.origin, oldo.x + x)
				self.origin = with_y(self.origin, oldo.y + y)
				self.origin = with_z(self.origin, oldo.z + z)
				r = random()
				if r < 0.3 then
					ThrowGib(self, "progs/gib1.mdl", -999)
				elseif r < 0.6 then
					ThrowGib(self, "progs/gib2.mdl", -999)
				else
					ThrowGib(self, "progs/gib3.mdl", -999)
				end
				y = y + 32
			end
			x = x + 32
		end
		z = z + 96
	end
	-- start the end text
	finale(
		"Congratulations and well done! You have\n"
			.. "beaten the hideous Shub-Niggurath, and\n"
			.. "her hundreds of ugly changelings and\n"
			.. "monsters. You have proven that your\n"
			.. "skill and your cunning are greater than\n"
			.. "all the powers of Quake. You are the\n"
			.. "master now. Id Software salutes you."
	)
	-- put a player model down
	n = spawn()
	setmodel(n, "progs/player.mdl")
	oldo = oldo - vector.create(32, 264, 0)
	setorigin(n, oldo)
	n.angles = vector.create(0, 290, 0)
	n.frame = 1
	remove(self)
	-- switch cd track
	engine.music(3)
	engine.lightstyle(0, "m")
end

function monster_oldone(self)
	if deathmatch ~= 0 then
		remove(self)
		return
	end
	self.solid = SOLID_SLIDEBOX
	self.movetype = MOVETYPE_STEP
	setmodel(self, "progs/oldone.mdl")
	setsize(self, vector.create(-160, -128, -24), vector.create(160, 128, 256))
	self.health = 40000
	-- kill by telefrag
	self.think = old_idle1
	self.nextthink = time() + 0.1
	self.takedamage = DAMAGE_YES
	self.th_die = finale_1
	shub = self
	total_monsters = total_monsters + 1
end
