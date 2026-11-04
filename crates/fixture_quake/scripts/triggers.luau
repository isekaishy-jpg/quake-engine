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

found_secrets = 0
total_secrets = 0

-- The trigger was just touched or used. self.enemy should be set to the
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
		sound(self, self.noise)
	end

	activator = self.enemy
	SUB_UseTargets(self)

	if self.wait > 0 then
		self.think = SUB_Null
		self.nextthink = time() + self.wait
	else
		-- We can't just remove it here, because this is a touch function.
		self.touch = SUB_Null
		self.nextthink = time() + 0.1
		self.think = SUB_Remove
	end
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
		local forward = makevectors(other.view)
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

	if bit32.band(self.spawnflags, SPAWNFLAG_NOTOUCH) == 0 then
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

function teleport_touch(self, other)
	if has_text(self.targetname) and self.nextthink < time() then
		return
	end

	if bit32.band(self.spawnflags, PLAYER_ONLY) ~= 0 and other.classname ~= "player" then
		return
	end

	if other.health <= 0 then
		return
	end

	SUB_UseTargets(self)

	local t = find(nil, "targetname", self.target)
	if t == nil then
		objerror("couldn't find target")
	end

	-- A teleport can target a train, which has no mangle.
	local mangle = t.mangle or vector.zero
	local forward = makevectors(mangle)
	setorigin(other, t.origin)
	if other.classname == "player" then
		engine.set_view(mangle.y, mangle.x)
		other.velocity = forward * 300
	end
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
	self.think = SUB_Null
end

function trigger_teleport(self)
	InitTrigger(self)
	self.touch = teleport_touch
	if not has_text(self.target) then
		objerror("no target")
	end
	self.use = teleport_use
end

function trigger_skill_touch(_self, _other) end

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
	self.message = ""
	SUB_UseTargets(self)
	remove(self)
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
	if (other.health or 0) > 0 then
		other.velocity = self.movedir * (self.speed * 10)
		if other.classname == "player" and (other.fly_sound or 0) < time() then
			other.fly_sound = time() + 1.5
			sound(other, "ambience/windfly.wav")
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
