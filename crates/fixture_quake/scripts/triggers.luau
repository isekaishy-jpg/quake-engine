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
-- Ported from triggers.qc.

local SPAWNFLAG_NOMESSAGE = 1
local SPAWNFLAG_NOTOUCH = 1
local PLAYER_ONLY = 1
local PUSH_ONCE = 1
local SILENT = 2

found_secrets = 0
total_secrets = 0

-- A trigger that can be shot gets ready to be shot again.
function multi_wait(self)
	if (self.max_health or 0) ~= 0 then
		self.health = self.max_health
		self.takedamage = DAMAGE_YES
		self.solid = SOLID_BBOX
	end
end

-- The trigger was just touched, shot or used. self.enemy should be set to the
-- activator, so it can be held through a delay.
function multi_trigger(self)
	if self.nextthink > time() then
		return
	end

	if self.classname == "trigger_secret" then
		if self.enemy.classname ~= "player" then
			return
		end
		found_secrets += 1
	end

	if self.noise then
		sound(self, CHAN_VOICE, self.noise, 1, ATTN_NORM)
	end

	-- Don't trigger again until reset.
	self.takedamage = DAMAGE_NO

	activator = self.enemy
	SUB_UseTargets(self)

	if self.wait > 0 then
		self.think = multi_wait
		self.nextthink = time() + self.wait
	else
		-- We can't just remove it here, because this is a touch function.
		self.touch = SUB_Null
		self.nextthink = time() + 0.1
		self.think = SUB_Remove
	end
end

function multi_killed(self)
	self.enemy = damage_attacker
	multi_trigger(self)
end

function multi_use(self, _other)
	self.enemy = activator
	multi_trigger(self)
end

function multi_touch(self, other)
	if other.classname ~= "player" then
		return
	end

	-- If the trigger has an angles field, check the player's facing direction.
	if self.movedir ~= vector.zero then
		-- A player's `angles`, which `SV_ClientThink` makes of their view.
		local view = other.view
		local forward = makevectors(vector.create(-view.x / 3, view.y, 0))
		if vector.dot(forward, self.movedir) < 0 then
			return
		end
	end

	self.enemy = other
	multi_trigger(self)
end

function trigger_multiple(self)
	if self.sounds == 1 then
		self.noise = "misc/secret.wav"
	elseif self.sounds == 2 then
		self.noise = "misc/talk.wav"
	elseif self.sounds == 3 then
		self.noise = "misc/trigger1.wav"
	end

	if self.wait == 0 then
		self.wait = 0.2
	end
	self.use = multi_use

	InitTrigger(self)

	local notouch = bit32.band(self.spawnflags, SPAWNFLAG_NOTOUCH) ~= 0
	if (self.health or 0) ~= 0 then
		if notouch then
			objerror("health and notouch don't make sense")
		end
		self.max_health = self.health
		self.th_die = multi_killed
		self.takedamage = DAMAGE_YES
		self.solid = SOLID_BBOX
		setorigin(self, self.origin)
	elseif not notouch then
		self.touch = multi_touch
	end
end

function trigger_once(self)
	self.wait = -1
	trigger_multiple(self)
end

function trigger_relay(self)
	self.use = SUB_UseTargets
end

function trigger_secret(self)
	total_secrets += 1
	self.wait = -1
	if not has_text(self.message) then
		self.message = "You found a secret area!"
	end
	if self.sounds == 0 then
		self.sounds = 1
	end
	if self.sounds == 1 then
		self.noise = "misc/secret.wav"
	elseif self.sounds == 2 then
		self.noise = "misc/talk.wav"
	end
	trigger_multiple(self)
end

function counter_use(self, _other)
	self.count -= 1
	if self.count < 0 then
		return
	end

	local tell = activator
		and activator.classname == "player"
		and bit32.band(self.spawnflags, SPAWNFLAG_NOMESSAGE) == 0

	if self.count ~= 0 then
		if tell then
			if self.count >= 4 then
				centerprint(activator, "There are more to go...")
			elseif self.count == 3 then
				centerprint(activator, "Only 3 more to go...")
			elseif self.count == 2 then
				centerprint(activator, "Only 2 more to go...")
			else
				centerprint(activator, "Only 1 more to go...")
			end
		end
		return
	end

	if tell then
		centerprint(activator, "Sequence completed!")
	end
	self.enemy = activator
	multi_trigger(self)
end

function trigger_counter(self)
	self.wait = -1
	if self.count == 0 then
		self.count = 2
	end
	self.use = counter_use
end

function play_teleport(self)
	local v = random() * 5
	local sample = if v < 1
		then "misc/r_tele1.wav"
		elseif v < 2 then "misc/r_tele2.wav"
		elseif v < 3 then "misc/r_tele3.wav"
		elseif v < 4 then "misc/r_tele4.wav"
		else "misc/r_tele5.wav"
	sound(self, CHAN_VOICE, sample, 1, ATTN_NORM)
	remove(self)
end

function spawn_tfog(org)
	local s = spawn()
	s.origin = org
	s.nextthink = time() + 0.2
	s.think = play_teleport
	temp_entity(TE_TELEPORT, org)
end

function tdeath_touch(self, other)
	if other == self.owner then
		return
	end
	-- Frag anyone who teleports in on top of an invincible player.
	if other.classname == "player" then
		if (other.invincible_finished or 0) > time() then
			self.classname = "teledeath2"
		end
		if self.owner.classname ~= "player" then
			-- Other monsters explode themselves.
			T_Damage(self.owner, self, self, 50000)
			return
		end
	end
	if other.health ~= 0 then
		T_Damage(other, self, self, 50000)
	end
end

function spawn_tdeath(org, death_owner)
	local death = spawn()
	death.classname = "teledeath"
	death.movetype = MOVETYPE_NONE
	death.solid = SOLID_TRIGGER
	death.angles = vector.zero
	local margin = vector.create(1, 1, 1)
	setsize(death, death_owner.mins - margin, death_owner.maxs + margin)
	setorigin(death, org)
	death.touch = tdeath_touch
	death.nextthink = time() + 0.2
	death.think = SUB_Remove
	death.owner = death_owner
	engine.force_retouch()
end

function teleport_touch(self, other)
	if has_text(self.targetname) and self.nextthink < time() then
		return
	end

	if bit32.band(self.spawnflags, PLAYER_ONLY) ~= 0 and other.classname ~= "player" then
		return
	end

	-- Only living creatures teleport.
	if other.health <= 0 or other.solid ~= SOLID_SLIDEBOX then
		return
	end

	SUB_UseTargets(self)

	-- Put a fog where the player was.
	spawn_tfog(other.origin)

	local t = find(nil, "targetname", self.target)
	if t == nil then
		objerror("couldn't find target")
	end

	-- A teleport can target a train, which has no mangle.
	local mangle = t.mangle or vector.zero
	local forward = makevectors(mangle)

	-- And another in front of the destination.
	spawn_tfog(t.origin + forward * 32)
	spawn_tdeath(t.origin, other)

	setorigin(other, t.origin)
	other.angles = mangle
	if other.classname == "player" then
		set_view(other, mangle.y, mangle.x)
		engine.no_backing(rawget(other, "_id"), 0.7)
		other.velocity = forward * 300
	end
	other.flags -= bit32.band(other.flags, FL_ONGROUND)
end

function info_teleport_destination(self)
	self.mangle = self.angles
	self.angles = vector.zero
	self.model = ""
	self.origin = self.origin + vector.create(0, 0, 27)
	if not has_text(self.targetname) then
		objerror("no targetname")
	end
end

function teleport_use(self, _other)
	self.nextthink = time() + 0.2
	engine.force_retouch()
	self.think = SUB_Null
end

function trigger_teleport(self)
	InitTrigger(self)
	self.touch = teleport_touch
	if not has_text(self.target) then
		objerror("no target")
	end
	self.use = teleport_use

	if bit32.band(self.spawnflags, SILENT) == 0 then
		ambientsound((self.mins + self.maxs) * 0.5, "ambience/hum1.wav", 0.5, ATTN_STATIC)
	end
end

function trigger_skill_touch(self, other)
	if other.classname ~= "player" then
		return
	end
	localcmd("skill " .. self.message)
end

function trigger_setskill(self)
	InitTrigger(self)
	self.touch = trigger_skill_touch
end

-- The registered check was dropped, so these always fire.
function trigger_onlyregistered_touch(self, other)
	if other.classname ~= "player" then
		return
	end
	if (self.attack_finished or 0) > time() then
		return
	end
	self.attack_finished = time() + 2
	if cvar("registered") ~= 0 then
		self.message = ""
		SUB_UseTargets(self)
		remove(self)
	elseif has_text(self.message) then
		centerprint(other, self.message)
		sound(other, CHAN_BODY, "misc/talk.wav", 1, ATTN_NORM)
	end
end

function trigger_onlyregistered(self)
	InitTrigger(self)
	self.touch = trigger_onlyregistered_touch
end

function hurt_on(self)
	self.solid = SOLID_TRIGGER
	self.nextthink = -1
end

function hurt_touch(self, other)
	if (other.health or 0) > 0 then
		self.solid = SOLID_NOT
		T_Damage(other, self, self, self.dmg)
		self.think = hurt_on
		self.nextthink = time() + 1
	end
end

function trigger_hurt(self)
	InitTrigger(self)
	self.touch = hurt_touch
	if self.dmg == 0 then
		self.dmg = 5
	end
end

function trigger_push_touch(self, other)
	if other.classname == "grenade" then
		other.velocity = self.movedir * (self.speed * 10)
	elseif (other.health or 0) > 0 then
		other.velocity = self.movedir * (self.speed * 10)
		if other.classname == "player" and (other.fly_sound or 0) < time() then
			other.fly_sound = time() + 1.5
			sound(other, CHAN_AUTO, "ambience/windfly.wav", 1, ATTN_NORM)
		end
	end
	if bit32.band(self.spawnflags, PUSH_ONCE) ~= 0 then
		remove(self)
	end
end

function trigger_push(self)
	InitTrigger(self)
	self.touch = trigger_push_touch
	if self.speed == 0 then
		self.speed = 1000
	end
end

function trigger_monsterjump_touch(self, other)
	local kind = bit32.band(other.flags, bit32.bor(FL_MONSTER, FL_FLY, FL_SWIM))
	if kind ~= FL_MONSTER then
		return
	end
	-- Set X and Y even if not on the ground, so the jump will clear lips.
	local velocity = other.velocity
	local x, y = self.movedir.x * self.speed, self.movedir.y * self.speed
	other.velocity = vector.create(x, y, velocity.z)
	if bit32.band(other.flags, FL_ONGROUND) == 0 then
		return
	end
	other.flags -= FL_ONGROUND
	other.velocity = with_z(other.velocity, self.height)
end

function trigger_monsterjump(self)
	if self.speed == 0 then
		self.speed = 200
	end
	if self.height == 0 then
		self.height = 200
	end
	if self.angles == vector.zero then
		self.angles = vector.create(0, 360, 0)
	end
	InitTrigger(self)
	self.touch = trigger_monsterjump_touch
end
