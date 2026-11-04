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
-- Ported from monsters.qc.

-- expands to:
-- name ()
-- self.think = nextthink
-- <code>
function monster_use(self)
	if exists(self.enemy) then
		return
	end
	if self.health <= 0 then
		return
	end
	if bit32.band(activator.items, IT_INVISIBILITY) ~= 0 then
		return
	end
	if bit32.band(activator.flags, FL_NOTARGET) ~= 0 then
		return
	end
	if activator.classname ~= "player" then
		return
	end
	self.enemy = activator
	self.nextthink = time() + 0.1
	self.think = FoundTarget
end

function monster_death_use(self)
	-- fall to ground
	if bit32.band(self.flags, FL_FLY) ~= 0 then
		self.flags = self.flags - FL_FLY
	end
	if bit32.band(self.flags, FL_SWIM) ~= 0 then
		self.flags = self.flags - FL_SWIM
	end
	if not has_text(self.target) then
		return
	end
	activator = self.enemy
	SUB_UseTargets(self)
end

function walkmonster_start_go(self)
	self.origin = with_z(self.origin, self.origin.z + 1)
	-- raise off floor a bit
	droptofloor(self)
	if not walkmove(self, 0, 0) then
	end
	self.takedamage = DAMAGE_AIM
	self.ideal_yaw = vector.dot(self.angles, vector.create(0, 1, 0))
	if self.yaw_speed == 0 then
		self.yaw_speed = 20
	end
	self.view_ofs = vector.create(0, 0, 25)
	self.use = monster_use
	self.flags = bit32.bor(self.flags, FL_MONSTER)
	if has_text(self.target) then
		self.movetarget = find_entity(world, "targetname", self.target)
		self.goalentity = self.movetarget
		self.ideal_yaw = vectoyaw(ent(self.goalentity).origin - self.origin)
		if not exists(self.movetarget) then
		end
		if ent(self.movetarget).classname == "path_corner" then
			self.th_walk(self)
		else
			self.pausetime = 99999999
		end
		self.th_stand(self)
	else
		self.pausetime = 99999999
		self.th_stand(self)
	end
	-- spread think times so they don't all happen at same time
	self.nextthink = self.nextthink + random() * 0.5
end

function walkmonster_start(self)
	-- delay drop to floor to make sure all doors have been spawned
	-- spread think times so they don't all happen at same time
	self.nextthink = self.nextthink + random() * 0.5
	self.think = walkmonster_start_go
	total_monsters = total_monsters + 1
end

function flymonster_start_go(self)
	self.takedamage = DAMAGE_AIM
	self.ideal_yaw = vector.dot(self.angles, vector.create(0, 1, 0))
	if self.yaw_speed == 0 then
		self.yaw_speed = 10
	end
	self.view_ofs = vector.create(0, 0, 25)
	self.use = monster_use
	self.flags = bit32.bor(self.flags, FL_FLY)
	self.flags = bit32.bor(self.flags, FL_MONSTER)
	if not walkmove(self, 0, 0) then
	end
	if has_text(self.target) then
		self.movetarget = find_entity(world, "targetname", self.target)
		self.goalentity = self.movetarget
		if not exists(self.movetarget) then
		end
		if ent(self.movetarget).classname == "path_corner" then
			self.th_walk(self)
		else
			self.pausetime = 99999999
		end
		self.th_stand(self)
	else
		self.pausetime = 99999999
		self.th_stand(self)
	end
end

function flymonster_start(self)
	-- spread think times so they don't all happen at same time
	self.nextthink = self.nextthink + random() * 0.5
	self.think = flymonster_start_go
	total_monsters = total_monsters + 1
end

function swimmonster_start_go(self)
	if deathmatch ~= 0 then
		remove(self)
		return
	end
	self.takedamage = DAMAGE_AIM
	total_monsters = total_monsters + 1
	self.ideal_yaw = vector.dot(self.angles, vector.create(0, 1, 0))
	if self.yaw_speed == 0 then
		self.yaw_speed = 10
	end
	self.view_ofs = vector.create(0, 0, 10)
	self.use = monster_use
	self.flags = bit32.bor(self.flags, FL_SWIM)
	self.flags = bit32.bor(self.flags, FL_MONSTER)
	if has_text(self.target) then
		self.movetarget = find_entity(world, "targetname", self.target)
		self.goalentity = self.movetarget
		if not exists(self.movetarget) then
		end
		self.ideal_yaw = vectoyaw(ent(self.goalentity).origin - self.origin)
		self.th_walk(self)
	else
		self.pausetime = 99999999
		self.th_stand(self)
	end
	-- spread think times so they don't all happen at same time
	self.nextthink = self.nextthink + random() * 0.5
end

function swimmonster_start(self)
	-- spread think times so they don't all happen at same time
	self.nextthink = self.nextthink + random() * 0.5
	self.think = swimmonster_start_go
	total_monsters = total_monsters + 1
end
