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
-- Ported from fight.qc.

enemy_vis = false
enemy_infront = false
enemy_range = 0

enemy_yaw = 0

function knight_attack(self)
	local len
	-- decide if now is a good swing time
	local enemy = ent(self.enemy)
	len = vector.magnitude(enemy.origin + enemy.view_ofs - (self.origin + self.view_ofs))
	if len < 80 then
		knight_atk1(self)
	else
		knight_runatk1(self)
	end
end

function CheckAttack(self)
	local spot1, spot2, targ, chance
	targ = self.enemy
	-- see if any entities are in the way of the shot
	spot1 = self.origin + self.view_ofs
	spot2 = targ.origin + targ.view_ofs
	traceline(spot1, spot2, false, self)
	if trace_ent ~= targ then
		return false
	end
	if trace_inopen and trace_inwater then
		return false
	end
	if enemy_range == RANGE_MELEE then
		-- melee attack
		if self.th_melee then
			if self.classname == "monster_knight" then
				knight_attack(self)
			else
				self.th_melee(self)
			end
			return true
		end
	end
	if not self.th_missile then
		return false
	end
	if time() < self.attack_finished then
		return false
	end
	if enemy_range == RANGE_FAR then
		return false
	end
	if enemy_range == RANGE_MELEE then
		chance = 0.9
		self.attack_finished = 0
	elseif enemy_range == RANGE_NEAR then
		if self.th_melee then
			chance = 0.2
		else
			chance = 0.4
		end
	elseif enemy_range == RANGE_MID then
		if self.th_melee then
			chance = 0.05
		else
			chance = 0.1
		end
	else
		chance = 0
	end
	if random() < chance then
		self.th_missile(self)
		SUB_AttackFinished(self, 2 * random())
		return true
	end
	return false
end

function ai_face(self)
	self.ideal_yaw = vectoyaw(ent(self.enemy).origin - self.origin)
	ChangeYaw(self)
end

function ai_charge(self, d)
	ai_face(self)
	movetogoal(self, d)
	-- done in C code...
end

function ai_charge_side(self)
	local dtemp, heading
	-- aim to the left of the enemy for a flyby
	self.ideal_yaw = vectoyaw(ent(self.enemy).origin - self.origin)
	ChangeYaw(self)
	makevectors(self.angles)
	dtemp = ent(self.enemy).origin - 30 * v_right
	heading = vectoyaw(dtemp - self.origin)
	walkmove(self, heading, 20)
end

function ai_melee(self)
	local delta, ldmg
	if not exists(self.enemy) then
		return
	end
	delta = ent(self.enemy).origin - self.origin
	if vector.magnitude(delta) > 60 then
		return
	end
	ldmg = (random() + random() + random()) * 3
	T_Damage(self.enemy, self, self, ldmg)
end

function ai_melee_side(self)
	local delta, ldmg
	if not exists(self.enemy) then
		return
	end
	ai_charge_side(self)
	delta = ent(self.enemy).origin - self.origin
	if vector.magnitude(delta) > 60 then
		return
	end
	if not CanDamage(self.enemy, self) then
		return
	end
	ldmg = (random() + random() + random()) * 3
	T_Damage(self.enemy, self, self, ldmg)
end

function SoldierCheckAttack(self)
	local spot1, spot2, targ, chance
	targ = self.enemy
	-- see if any entities are in the way of the shot
	spot1 = self.origin + self.view_ofs
	spot2 = targ.origin + targ.view_ofs
	traceline(spot1, spot2, false, self)
	if trace_inopen and trace_inwater then
		return false
	end
	if trace_ent ~= targ then
		return false
	end
	if time() < self.attack_finished then
		return false
	end
	if enemy_range == RANGE_FAR then
		return false
	end
	if enemy_range == RANGE_MELEE then
		chance = 0.9
	elseif enemy_range == RANGE_NEAR then
		chance = 0.4
	elseif enemy_range == RANGE_MID then
		chance = 0.05
	else
		chance = 0
	end
	if random() < chance then
		self.th_missile(self)
		SUB_AttackFinished(self, 1 + random())
		if random() < 0.3 then
			self.lefty = (if self.lefty == 0 then 1 else 0)
		end
		return true
	end
	return false
end

function ShamCheckAttack(self)
	local spot1, spot2, targ
	if enemy_range == RANGE_MELEE then
		if CanDamage(self.enemy, self) then
			self.attack_state = AS_MELEE
			return true
		end
	end
	if time() < self.attack_finished then
		return false
	end
	if not enemy_vis then
		return false
	end
	targ = self.enemy
	-- see if any entities are in the way of the shot
	spot1 = self.origin + self.view_ofs
	spot2 = targ.origin + targ.view_ofs
	if vector.magnitude(spot1 - spot2) > 600 then
		return false
	end
	traceline(spot1, spot2, false, self)
	if trace_inopen and trace_inwater then
		return false
	end
	if trace_ent ~= targ then
		return false
		-- don't have a clear shot
	end
	if enemy_range == RANGE_FAR then
		return false
	end
	self.attack_state = AS_MISSILE
	SUB_AttackFinished(self, 2 + 2 * random())
	return true
end

function OgreCheckAttack(self)
	local spot1, spot2, targ, chance
	if enemy_range == RANGE_MELEE then
		if CanDamage(self.enemy, self) then
			self.attack_state = AS_MELEE
			return true
		end
	end
	if time() < self.attack_finished then
		return false
	end
	if not enemy_vis then
		return false
	end
	targ = self.enemy
	-- see if any entities are in the way of the shot
	spot1 = self.origin + self.view_ofs
	spot2 = targ.origin + targ.view_ofs
	traceline(spot1, spot2, false, self)
	if trace_inopen and trace_inwater then
		return false
	end
	if trace_ent ~= targ then
		return false
		-- don't have a clear shot
	end
	if time() < self.attack_finished then
		return false
	end
	if enemy_range == RANGE_FAR then
		return false
	elseif enemy_range == RANGE_NEAR then
		chance = 0.1
	elseif enemy_range == RANGE_MID then
		chance = 0.05
	else
		chance = 0
	end
	self.attack_state = AS_MISSILE
	SUB_AttackFinished(self, 1 + 2 * random())
	return true
end
