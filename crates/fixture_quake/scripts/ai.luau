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
-- Ported from ai.qc.

current_yaw = 0

sight_entity = nil

sight_entity_time = 0

function anglemod(v)
	return v % 360
end

function movetarget_f(self)
	if not has_text(self.targetname) then
		objerror("monster_movetarget: no targetname")
	end
	self.solid = SOLID_TRIGGER
	self.touch = t_movetarget
	setsize(self, vector.create(-8, -8, -8), vector.create(8, 8, 8))
end

function path_corner(self)
	movetarget_f(self)
end

function t_movetarget(self, other)
	local temp
	if other.movetarget ~= self then
		return
	end
	if exists(other.enemy) then
		return
	end
	temp = self
	self = other
	other = temp
	if self.classname == "monster_ogre" then
		sound(self, CHAN_VOICE, "ogre/ogdrag.wav", 1, ATTN_IDLE)
	end
	self.movetarget = find_entity(world, "targetname", other.target)
	self.goalentity = self.movetarget
	self.ideal_yaw = vectoyaw(ent(self.goalentity).origin - self.origin)
	if not exists(self.movetarget) then
		self.pausetime = time() + 999999
		self.th_stand(self)
		return
	end
end

function range(self, targ)
	local spot1, spot2, r
	spot1 = self.origin + self.view_ofs
	spot2 = targ.origin + targ.view_ofs
	r = vector.magnitude(spot1 - spot2)
	if r < 120 then
		return RANGE_MELEE
	end
	if r < 500 then
		return RANGE_NEAR
	end
	if r < 1000 then
		return RANGE_MID
	end
	return RANGE_FAR
end

function visible(self, targ)
	local spot1, spot2
	spot1 = self.origin + self.view_ofs
	spot2 = targ.origin + targ.view_ofs
	traceline(spot1, spot2, true, self)
	-- see through other monsters
	if trace_inopen and trace_inwater then
		return false
	end
	if trace_fraction == 1 then
		return true
	end
	return false
end

function infront(self, targ)
	local vec, dot
	makevectors(self.angles)
	vec = normalize(targ.origin - self.origin)
	dot = vector.dot(vec, v_forward)
	if dot > 0.3 then
		return true
	end
	return false
end

function HuntTarget(self)
	self.goalentity = self.enemy
	self.think = self.th_run
	self.ideal_yaw = vectoyaw(ent(self.enemy).origin - self.origin)
	self.nextthink = time() + 0.1
	SUB_AttackFinished(self, 1)
	-- wait a while before first attack
end

function SightSound(self)
	local rsnd
	if self.classname == "monster_ogre" then
		sound(self, CHAN_VOICE, "ogre/ogwake.wav", 1, ATTN_NORM)
	elseif self.classname == "monster_knight" then
		sound(self, CHAN_VOICE, "knight/ksight.wav", 1, ATTN_NORM)
	elseif self.classname == "monster_shambler" then
		sound(self, CHAN_VOICE, "shambler/ssight.wav", 1, ATTN_NORM)
	elseif self.classname == "monster_demon1" then
		sound(self, CHAN_VOICE, "demon/sight2.wav", 1, ATTN_NORM)
	elseif self.classname == "monster_wizard" then
		sound(self, CHAN_VOICE, "wizard/wsight.wav", 1, ATTN_NORM)
	elseif self.classname == "monster_zombie" then
		sound(self, CHAN_VOICE, "zombie/z_idle.wav", 1, ATTN_NORM)
	elseif self.classname == "monster_dog" then
		sound(self, CHAN_VOICE, "dog/dsight.wav", 1, ATTN_NORM)
	elseif self.classname == "monster_hell_knight" then
		sound(self, CHAN_VOICE, "hknight/sight1.wav", 1, ATTN_NORM)
	elseif self.classname == "monster_tarbaby" then
		sound(self, CHAN_VOICE, "blob/sight1.wav", 1, ATTN_NORM)
	elseif self.classname == "monster_vomit" then
		sound(self, CHAN_VOICE, "vomitus/v_sight1.wav", 1, ATTN_NORM)
	elseif self.classname == "monster_enforcer" then
		rsnd = rint(random() * 3)
		if rsnd == 1 then
			sound(self, CHAN_VOICE, "enforcer/sight1.wav", 1, ATTN_NORM)
		elseif rsnd == 2 then
			sound(self, CHAN_VOICE, "enforcer/sight2.wav", 1, ATTN_NORM)
		elseif rsnd == 0 then
			sound(self, CHAN_VOICE, "enforcer/sight3.wav", 1, ATTN_NORM)
		else
			sound(self, CHAN_VOICE, "enforcer/sight4.wav", 1, ATTN_NORM)
		end
	elseif self.classname == "monster_army" then
		sound(self, CHAN_VOICE, "soldier/sight1.wav", 1, ATTN_NORM)
	elseif self.classname == "monster_shalrath" then
		sound(self, CHAN_VOICE, "shalrath/sight.wav", 1, ATTN_NORM)
	end
end

function FoundTarget(self)
	if ent(self.enemy).classname == "player" then
		-- let other monsters see this monster for a while
		sight_entity = self
		sight_entity_time = time()
	end
	self.show_hostile = time() + 1
	-- wake up other monsters
	SightSound(self)
	HuntTarget(self)
end

function FindTarget(self)
	local client, r
	-- if the first spawnflag bit is set, the monster will only wake up on
	-- really seeing the player, not another monster getting angry
	-- spawnflags & 3 is a big hack, because zombie crucified used the first
	-- spawn flag prior to the ambush flag, and I forgot about it, so the second
	-- spawn flag works as well
	if sight_entity_time >= time() - 0.1 and bit32.band(self.spawnflags, 3) == 0 then
		client = ent(sight_entity)
		if client.enemy == self.enemy then
			return false
		end
	else
		client = checkclient(self)
		if not exists(client) then
			return false
		end
	end
	if client == self.enemy then
		return false
	end
	if bit32.band(client.flags, FL_NOTARGET) ~= 0 then
		return false
	end
	if bit32.band(client.items, IT_INVISIBILITY) ~= 0 then
		return false
	end
	r = range(self, client)
	if r == RANGE_FAR then
		return false
	end
	if not visible(self, client) then
		return false
	end
	if r == RANGE_NEAR then
		if client.show_hostile < time() and not infront(self, client) then
			return false
		end
	elseif r == RANGE_MID then
		if not infront(self, client) then
			return false
		end
	end
	self.enemy = client
	if ent(self.enemy).classname ~= "player" then
		self.enemy = ent(self.enemy).enemy
		if ent(self.enemy).classname ~= "player" then
			self.enemy = nil
			return false
		end
	end
	FoundTarget(self)
	return true
end

function ai_forward(self, dist)
	walkmove(self, self.angles.y, dist)
end

function ai_back(self, dist)
	walkmove(self, (self.angles.y + 180), dist)
end

function ai_pain(self, dist)
	ai_back(self, dist)
end

function ai_painforward(self, dist)
	walkmove(self, self.ideal_yaw, dist)
end

function ai_walk(self, dist)
	movedist = dist
	if self.classname == "monster_dragon" then
		movetogoal(self, dist)
		return
	end
	if FindTarget(self) then
		return
	end
	movetogoal(self, dist)
end

function ai_stand(self)
	if FindTarget(self) then
		return
	end
	if time() > self.pausetime then
		self.th_walk(self)
		return
	end
end

function ai_turn(self)
	if FindTarget(self) then
		return
	end
	ChangeYaw(self)
end

function ChooseTurn(self, dest3)
	local dir = vector.zero
	local newdir = vector.zero
	dir = self.origin - dest3
	newdir = with_x(newdir, trace_plane_normal.y)
	newdir = with_y(newdir, 0 - trace_plane_normal.x)
	newdir = with_z(newdir, 0)
	if vector.dot(dir, newdir) > 0 then
		dir = with_x(dir, 0 - trace_plane_normal.y)
		dir = with_y(dir, trace_plane_normal.x)
	else
		dir = with_x(dir, trace_plane_normal.y)
		dir = with_y(dir, 0 - trace_plane_normal.x)
	end
	dir = with_z(dir, 0)
	self.ideal_yaw = vectoyaw(dir)
end

function FacingIdeal(self)
	local delta
	delta = anglemod(self.angles.y - self.ideal_yaw)
	if delta > 45 and delta < 315 then
		return false
	end
	return true
end

function CheckAnyAttack(self)
	if not enemy_vis then
		return false
	end
	if self.classname == "monster_army" then
		return SoldierCheckAttack(self)
	end
	if self.classname == "monster_ogre" then
		return OgreCheckAttack(self)
	end
	if self.classname == "monster_shambler" then
		return ShamCheckAttack(self)
	end
	if self.classname == "monster_demon1" then
		return DemonCheckAttack(self)
	end
	if self.classname == "monster_dog" then
		return DogCheckAttack(self)
	end
	if self.classname == "monster_wizard" then
		return WizardCheckAttack(self)
	end
	return CheckAttack(self)
end

function ai_run_melee(self)
	self.ideal_yaw = enemy_yaw
	ChangeYaw(self)
	if FacingIdeal(self) then
		self.th_melee(self)
		self.attack_state = AS_STRAIGHT
	end
end

function ai_run_missile(self)
	self.ideal_yaw = enemy_yaw
	ChangeYaw(self)
	if FacingIdeal(self) then
		self.th_missile(self)
		self.attack_state = AS_STRAIGHT
	end
end

function ai_run_slide(self)
	local ofs
	self.ideal_yaw = enemy_yaw
	ChangeYaw(self)
	if self.lefty ~= 0 then
		ofs = 90
	else
		ofs = -90
	end
	if walkmove(self, self.ideal_yaw + ofs, movedist) then
		return
	end
	self.lefty = 1 - self.lefty
	walkmove(self, self.ideal_yaw - ofs, movedist)
end

function ai_run(self, dist)
	movedist = dist
	-- see if the enemy is dead
	if ent(self.enemy).health <= 0 then
		self.enemy = nil
		-- FIXME: look all around for other targets
		if ent(self.oldenemy).health > 0 then
			self.enemy = self.oldenemy
			HuntTarget(self)
		else
			if exists(self.movetarget) then
				self.th_walk(self)
			else
				self.th_stand(self)
			end
			return
		end
	end
	self.show_hostile = time() + 1
	-- wake up other monsters
	-- check knowledge of enemy
	enemy_vis = visible(self, self.enemy)
	if enemy_vis then
		self.search_time = time() + 5
	end
	if coop ~= 0 and self.search_time < time() then
		if FindTarget(self) then
			return
		end
	end
	enemy_infront = infront(self, self.enemy)
	enemy_range = range(self, self.enemy)
	enemy_yaw = vectoyaw(ent(self.enemy).origin - self.origin)
	if self.attack_state == AS_MISSILE then
		ai_run_missile(self)
		return
	end
	if self.attack_state == AS_MELEE then
		ai_run_melee(self)
		return
	end
	if CheckAnyAttack(self) then
		return
	end
	if self.attack_state == AS_SLIDING then
		ai_run_slide(self)
		return
	end
	movetogoal(self, dist)
	-- done in C code...
end
