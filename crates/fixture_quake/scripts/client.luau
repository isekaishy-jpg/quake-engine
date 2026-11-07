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
-- Ported from client.qc: level changes, the intermission, obituaries, and the
-- player's sounds.

-- What the engine asks of a game: which of `ClientData`'s numbers, counting
-- from zero, is someone's frags, and which commands guests can use.
GameRules = {
	frags_stat = 1,
	guest_commands = { "kill" },
}

-- Whether someone hears a team message from another, from `Host_Say`.
function SameTeam(colors, heard_by)
	return teamplay == 0 or bit32.band(colors, 15) == bit32.band(heard_by, 15)
end

-- The next level, and where the last one was.
nextmap = ""
mapname = ""
framecount = 0

-- From `SV_SpawnServer`: what's fixed for the level, before anything spawns.
function level_started(name, flags)
	mapname = name
	serverflags = flags
	skill = math.clamp(rint(cvar("skill")), 0, 3)
	deathmatch = cvar("deathmatch")
	coop = cvar("coop")
	teamplay = cvar("teamplay")
	if coop ~= 0 then
		deathmatch = 0
	end
end

-- Runs at the start of each tick, so rules changed in play take effect.
function StartFrame()
	teamplay = cvar("teamplay")
	skill = math.clamp(rint(cvar("skill")), 0, 3)
	framecount += 1
end

-- The camera for the end of a level, from `FindIntermission`.
function FindIntermission()
	local spot = find_entity(world, "classname", "info_intermission")
	if exists(spot) then
		-- Pick a random one.
		local cyc = random() * 4
		while cyc > 1 do
			spot = find_entity(spot, "classname", "info_intermission")
			if not exists(spot) then
				spot = find_entity(spot, "classname", "info_intermission")
			end
			cyc -= 1
		end
		return spot
	end
	-- Then look for the start position.
	spot = find_entity(world, "classname", "info_player_start")
	if exists(spot) then
		return spot
	end
	objerror("FindIntermission: no spot")
	return spot
end

-- Whether a level change has been asked for, from `PF_changelevel`.
changelevel_issued = false

-- Moves on, taking what the player has and what's been finished.
function changelevel(map)
	if changelevel_issued then
		return
	end
	changelevel_issued = true
	for _, player in find_all("classname", "player") do
		SetChangeParms(player)
		engine.set_parms(parms, rawget(player, "_id"))
	end
	engine.changelevel(map, serverflags)
end

function GotoNextMap()
	if cvar("samelevel") ~= 0 then
		changelevel(mapname)
	else
		changelevel(nextmap)
	end
end

-- The text at the end of each episode.
local EPISODE_ENDS = {
	["maps/e2m6.bsp"] = "The Rune of Black Magic throbs evilly in\nyour hand and whispers dark thoughts\ninto your brain. You learn the inmost\nlore of the Hell-Mother; Shub-Niggurath!\nYou now know that she is behind all the\nterrible plotting which has led to so\nmuch death and horror. But she is not\ninviolate! Armed with this Rune, you\nrealize that once all four Runes are\ncombined, the gate to Shub-Niggurath's\nPit will open, and you can face the\nWitch-Goddess herself in her frightful\notherworld cathedral.",
	["maps/e3m6.bsp"] = "The charred viscera of diabolic horrors\nbubble viscously as you seize the Rune\nof Hell Magic. Its heat scorches your\nhand, and its terrible secrets blight\nyour mind. Gathering the shreds of your\ncourage, you shake the devil's shackles\nfrom your soul, and become ever more\nhard and determined to destroy the\nhideous creatures whose mere existence\nthreatens the souls and psyches of all\nthe population of Earth.",
	["maps/e4m7.bsp"] = "Despite the awful might of the Elder\nWorld, you have achieved the Rune of\nElder Magic, capstone of all types of\narcane wisdom. Beyond good and evil,\nbeyond life and death, the Rune\npulsates, heavy with import. Patient and\npotent, the Elder Being Shub-Niggurath\nweaves her dire plans to clear off all\nlife from the Earth, and bring her own\nfoul offspring to our world! For all the\ndwellers in these nightmare dimensions\nare her descendants! Once all Runes of\nmagic power are united, the energy\nbehind them will blast open the Gateway\nto Shub-Niggurath, and you can travel\nthere to foil the Hell-Mother's plots\nin person.",
}

-- The first episode's end, which depends on the edition.
local SHAREWARE_END =
	"As the corpse of the monstrous entity\nChthon sinks back into the lava whence\nit rose, you grip the Rune of Earth\nMagic tightly. Now that you have\nconquered the Dimension of the Doomed,\nrealm of Earth Magic, you are ready to\ncomplete your task in the other three\nhaunted lands of Quake. Or are you? If\nyou don't register Quake, you'll never\nknow what awaits you in the Realm of\nBlack Magic, the Netherworld, and the\nElder World!"
local REGISTERED_END =
	"As the corpse of the monstrous entity\nChthon sinks back into the lava whence\nit rose, you grip the Rune of Earth\nMagic tightly. Now that you have\nconquered the Dimension of the Doomed,\nrealm of Earth Magic, you are ready to\ncomplete your task. A Rune of magic\npower lies at the end of each haunted\nland of Quake. Go forth, seek the\ntotality of the four Runes!"

local ALL_RUNES =
	"Now, you have all four Runes. You sense\ntremendous invisible forces moving to\nunseal ancient barriers. Shub-Niggurath\nhad hoped to use the Runes Herself to\nclear off the Earth, but now instead,\nyou will use them to enter her home and\nconfront her as an avatar of avenging\nEarth-life. If you defeat her, you will\nbe remembered forever as the savior of\nthe planet. If she conquers, it will be\nas if you had never been born."

function ExitIntermission()
	-- Skip any text in deathmatch.
	if deathmatch ~= 0 then
		GotoNextMap()
		return
	end

	intermission_exittime = time() + 1
	intermission_running += 1

	-- Run some text if at the end of an episode.
	if intermission_running == 2 then
		local text = EPISODE_ENDS[world.model]
		if world.model == "maps/e1m7.bsp" then
			text = if cvar("registered") ~= 0 then REGISTERED_END else SHAREWARE_END
		end
		if text then
			engine.music(2)
			finale(text)
			return
		end
		GotoNextMap()
		return
	end

	if intermission_running == 3 then
		if cvar("registered") == 0 then
			-- The shareware episode is over, so go to the sell screen.
			localcmd("help")
			return
		end
		if bit32.band(serverflags, 15) == 15 then
			finale(ALL_RUNES)
			return
		end
	end

	GotoNextMap()
end

-- When the player presses attack or jump, change to the next level.
function IntermissionThink(self)
	if time() < intermission_exittime then
		return
	end
	if not self.button0 and not self.button2 then
		return
	end
	ExitIntermission()
end

-- Holds the player at `pos`, looking its way, for the end of a level or game.
function hold_at_camera(player, pos)
	local mangle = pos.mangle or vector.zero
	player.view_ofs = vector.zero
	player.angles = mangle
	-- Turn this way immediately.
	set_view(player, mangle.y, mangle.x)
	-- The end of a level is seen from its spot, and it holds still.
	local place = pos.origin
	engine.camera(
		string.format("fixed %g %g %g %g %g 0", place.x, place.y, place.z, mangle.y, mangle.x),
		0,
		rawget(player, "_id")
	)
	player.nextthink = time() + 0.5
	player.takedamage = DAMAGE_NO
	player.solid = SOLID_NOT
	player.movetype = MOVETYPE_NONE
	player.visible = false
	player.frozen = true
	player.weaponhidden = true
	setorigin(player, pos.origin)
end

function execute_changelevel(_self)
	intermission_running = 1
	-- Enforce a wait time before allowing changelevel.
	intermission_exittime = time() + (if deathmatch ~= 0 then 5 else 2)
	engine.music(3)
	local pos = FindIntermission()
	for _, player in find_all("classname", "player") do
		hold_at_camera(player, pos)
	end
	engine.intermission(1)
end

function changelevel_touch(self, other)
	if other.classname ~= "player" then
		return
	end
	if cvar("noexit") ~= 0 then
		T_Damage(other, self, self, 50000)
		return
	end
	bprint(other.netname .. " exited the level")
	nextmap = self.map
	activator = other
	SUB_UseTargets(self)
	if bit32.band(self.spawnflags, 1) ~= 0 and deathmatch == 0 then
		-- NO_INTERMISSION.
		GotoNextMap()
		return
	end
	self.touch = SUB_Null
	-- Touch functions run in the middle of movement, so move people later.
	self.think = execute_changelevel
	self.nextthink = time() + 0.1
end

function trigger_changelevel(self)
	if not has_text(self.map) then
		objerror("changelevel trigger doesn't have map")
	end
	InitTrigger(self)
	self.touch = changelevel_touch
end

-- Dead bodies stay where they fell, a few at a time, from `InitBodyQue`.
bodyque_head = nil

function InitBodyQue()
	local first = spawn()
	first.classname = "bodyque"
	local last = first
	for _ = 1, 3 do
		local body = spawn()
		body.classname = "bodyque"
		last.next_body = body
		last = body
	end
	last.next_body = first
	bodyque_head = first
end

function bodyque(_self) end

function CopyToBodyQue(entity)
	if bodyque_head == nil then
		InitBodyQue()
	end
	local body = bodyque_head
	body.angles = entity.angles
	setmodel(body, entity.model)
	body.frame = entity.frame
	body.movetype = MOVETYPE_TOSS
	body.velocity = entity.velocity
	body.flags = 0
	setorigin(body, entity.origin)
	setsize(body, entity.mins, entity.maxs)
	bodyque_head = body.next_body
end

-- Called by ClientKill and PlayerDeathThink.
function respawn(self)
	if coop ~= 0 then
		-- Make a copy of the dead body for appearance's sake.
		CopyToBodyQue(self)
		-- Get the spawn parms as they were at level start.
		parms = self.start_parms or parms
		PutClientInServer(self)
	elseif deathmatch ~= 0 then
		CopyToBodyQue(self)
		SetNewParms()
		PutClientInServer(self)
	else
		-- Restart the entire server.
		engine.restart()
	end
end

function ClientKill(self)
	bprint(self.netname .. " suicides")
	set_suicide_frame(self)
	self.frags = (self.frags or 0) - 2
	respawn(self)
end

-- Goes to the next level for deathmatch, from `NextLevel`.
function NextLevel()
	local exit = find(nil, "classname", "trigger_changelevel")
	if exit == nil or mapname == "start" then
		-- Go back to the same map with no trigger_changelevel.
		exit = spawn()
		exit.map = mapname
	end
	nextmap = exit.map
	if (exit.nextthink or 0) < time() then
		exit.think = execute_changelevel
		exit.nextthink = time() + 0.1
	end
end

-- Exits deathmatch games on their limits, from `CheckRules`.
function CheckRules(self)
	if intermission_running ~= 0 then
		return
	end
	local timelimit = cvar("timelimit") * 60
	local fraglimit = cvar("fraglimit")
	if timelimit ~= 0 and time() >= timelimit then
		NextLevel()
	elseif fraglimit ~= 0 and (self.frags or 0) >= fraglimit then
		NextLevel()
	end
end

function ClientDisconnect(self)
	bprint(self.netname .. " left the game with " .. tostring(self.frags or 0) .. " frags")
	sound(self, CHAN_BODY, "player/tornoff2.wav", 1, ATTN_NONE)
	set_suicide_frame(self)
	-- The original's body stays where it is. This one goes with them, so
	-- something else has to stay, and nothing should go on hunting it.
	CopyToBodyQue(self)
	self.health = 0
end

-- What each monster did to the player.
local MONSTER_OBITUARIES = {
	monster_army = " was shot by a Grunt",
	monster_demon1 = " was eviscerated by a Fiend",
	monster_dog = " was mauled by a Rottweiler",
	monster_dragon = " was fried by a Dragon",
	monster_enforcer = " was blasted by an Enforcer",
	monster_fish = " was fed to the Rotfish",
	monster_hell_knight = " was slain by a Death Knight",
	monster_knight = " was slashed by a Knight",
	monster_ogre = " was destroyed by an Ogre",
	monster_oldone = " became one with Shub-Niggurath",
	monster_shalrath = " was exploded by a Vore",
	monster_shambler = " was smashed by a Shambler",
	monster_tarbaby = " was slimed by a Spawn",
	monster_vomit = " was vomited on by a Vomitus",
	monster_wizard = " was scragged by a Scrag",
	monster_zombie = " joins the Zombies",
}

-- What each weapon says about a kill, as before and after the killer's name.
local WEAPON_OBITUARIES = {
	[IT_AXE] = { " was ax-murdered by ", "" },
	[IT_SHOTGUN] = { " chewed on ", "'s boomstick" },
	[IT_SUPER_SHOTGUN] = { " ate 2 loads of ", "'s buckshot" },
	[IT_NAILGUN] = { " was nailed by ", "" },
	[IT_SUPER_NAILGUN] = { " was punctured by ", "" },
}

-- How one player killed another.
local function kill_message(targ, attacker)
	local weapon = attacker.weapon
	local gibbed = targ.health < -40
	if weapon == IT_GRENADE_LAUNCHER then
		if gibbed then
			return targ.netname .. " was gibbed by " .. attacker.netname .. "'s grenade"
		end
		return targ.netname .. " eats " .. attacker.netname .. "'s pineapple"
	elseif weapon == IT_ROCKET_LAUNCHER then
		if gibbed then
			return targ.netname .. " was gibbed by " .. attacker.netname .. "'s rocket"
		end
		return targ.netname .. " rides " .. attacker.netname .. "'s rocket"
	elseif weapon == IT_LIGHTNING then
		local ending = if attacker.waterlevel > 1 then "'s discharge" else "'s shaft"
		return targ.netname .. " accepts " .. attacker.netname .. ending
	end
	local words = WEAPON_OBITUARIES[weapon] or { " was killed by ", "" }
	return targ.netname .. words[1] .. attacker.netname .. words[2]
end

-- How a player killed themselves.
local function killed_self(targ)
	if targ.weapon == 64 and targ.waterlevel > 1 then
		return " discharges into the water."
	end
	if targ.weapon == 16 then
		return " tries to put the pin back in"
	end
	if random() ~= 0 then
		return " becomes bored with life"
	end
	return " checks if their weapon is loaded"
end

-- What took a player without another player's help.
local function death_message(targ, attacker)
	local liquid = targ.watertype
	if liquid == CONTENT_WATER then
		return if random() < 0.5 then " sleeps with the fishes" else " sucks it down"
	elseif liquid == CONTENT_SLIME then
		return if random() < 0.5 then " gulped a load of slime" else " can't exist on slime alone"
	elseif liquid == CONTENT_LAVA then
		if targ.health < -15 then
			return " burst into flames"
		end
		return if random() < 0.5 then " turned into hot slag" else " visits the Volcano God"
	end
	if bit32.band(attacker.flags, FL_MONSTER) ~= 0 then
		return MONSTER_OBITUARIES[attacker.classname] or " died"
	end
	if attacker.classname == "explo_box" then
		return " blew up"
	end
	if attacker.solid == SOLID_BSP and exists(attacker) then
		return " was squished"
	end
	if targ.deathtype == "falling" then
		targ.deathtype = ""
		return " fell to their death"
	end
	if attacker.classname == "trap_shooter" or attacker.classname == "trap_spikeshooter" then
		return " was spiked"
	end
	if attacker.classname == "fireball" then
		return " ate a lavaball"
	end
	if attacker.classname == "trigger_changelevel" then
		return " tried to leave"
	end
	return " died"
end

-- Called when anything dies, and keeps the score.
function ClientObituary(targ, attacker)
	if targ.classname ~= "player" then
		return
	end
	attacker = ent(attacker)
	if attacker.classname == "teledeath" then
		bprint(targ.netname .. " was telefragged by " .. attacker.owner.netname)
		attacker.owner.frags = (attacker.owner.frags or 0) + 1
	elseif attacker.classname == "teledeath2" then
		bprint("Satan's power deflects " .. targ.netname .. "'s telefrag")
		targ.frags = (targ.frags or 0) - 1
	elseif attacker == targ then
		targ.frags = (targ.frags or 0) - 1
		bprint(targ.netname .. killed_self(targ))
	elseif attacker.classname == "player" then
		attacker.frags = (attacker.frags or 0) + 1
		bprint(kill_message(targ, attacker))
	else
		targ.frags = (targ.frags or 0) - 1
		bprint(targ.netname .. death_message(targ, attacker))
	end
end

-- Cheats and other things the player can ask for from the console.
function ClientCommand(self, command, argument)
	if command == "kill" then
		ClientKill(self)
	elseif command == "changelevel" then
		changelevel(argument)
	elseif command == "god" then
		self.flags = bit32.bxor(self.flags, FL_GODMODE)
		local on = bit32.band(self.flags, FL_GODMODE) ~= 0
		sprint(self, "godmode " .. (if on then "ON" else "OFF"))
	elseif command == "notarget" then
		self.flags = bit32.bxor(self.flags, FL_NOTARGET)
		local on = bit32.band(self.flags, FL_NOTARGET) ~= 0
		sprint(self, "notarget " .. (if on then "ON" else "OFF"))
	elseif command == "give" then
		ClientGive(self, argument)
	end
end

-- From `Host_Give_f`: a weapon by its number, or some of an ammo or health.
function ClientGive(self, argument)
	local kind, amount = string.match(argument, "^(%S+)%s*(%S*)$")
	local count = tonumber(amount) or 0
	if kind == nil then
		return
	end
	local number = tonumber(kind)
	if number and number >= 2 and number <= 8 then
		self.items = bit32.bor(self.items, bit32.lshift(IT_SHOTGUN, number - 2))
	elseif kind == "s" then
		self.ammo_shells = count
	elseif kind == "n" then
		self.ammo_nails = count
	elseif kind == "r" then
		self.ammo_rockets = count
	elseif kind == "c" then
		self.ammo_cells = count
	elseif kind == "h" then
		self.health = count
	end
	W_SetCurrentAmmo(self)
end

-- What's sent to the player each tick, from `SV_WriteClientdataToMessage`. The
-- numbers are the original's `STAT_` values, with its items last.
function ClientData(self)
	local take, save = self.dmg_take or 0, self.dmg_save or 0
	if take ~= 0 or save ~= 0 then
		local from = ent(self.dmg_inflictor)
		local at = from.origin + (from.mins + from.maxs) * 0.5
		engine.damage(rawget(self, "_id"), save, take, at)
		self.dmg_take = 0
		self.dmg_save = 0
	end
	return {
		self.health,
		self.frags or 0,
		0,
		self.currentammo or 0,
		self.armorvalue or 0,
		self.weaponframe,
		self.ammo_shells or 0,
		self.ammo_nails or 0,
		self.ammo_rockets or 0,
		self.ammo_cells or 0,
		self.weapon or 0,
		total_secrets,
		total_monsters,
		found_secrets,
		killed_monsters,
		bit32.bor(self.items or 0, bit32.lshift(serverflags, 28)),
		deathmatch,
	}
end

function PlayerJump(self)
	if self.waterjump then
		return
	end

	if self.waterlevel >= 2 then
		-- Play a swimming sound.
		if (self.swim_flag or 0) < time() then
			self.swim_flag = time() + 1
			local sample = if random() < 0.5 then "misc/water1.wav" else "misc/water2.wav"
			sound(self, CHAN_BODY, sample, 1, ATTN_NORM)
		end
		return
	end

	if not self.onground then
		return
	end

	-- Don't pogo stick.
	if not self.jump_released then
		return
	end

	self.jump_released = false
	sound(self, CHAN_BODY, "player/plyrjmp8.wav", 1, ATTN_NORM)
end

function WaterMove(self)
	if self.noclip or self.health < 0 then
		return
	end

	if self.waterlevel ~= 3 then
		if self.air_finished < time() then
			sound(self, CHAN_VOICE, "player/gasp2.wav", 1, ATTN_NORM)
		elseif self.air_finished < time() + 9 then
			sound(self, CHAN_VOICE, "player/gasp1.wav", 1, ATTN_NORM)
		end
		self.air_finished = time() + 12
		self.dmg = 2
	elseif self.air_finished < time() and (self.pain_finished or 0) < time() then
		-- Drown.
		self.dmg += 2
		if self.dmg > 15 then
			self.dmg = 10
		end
		T_Damage(self, world, world, self.dmg)
		self.pain_finished = time() + 1
	end

	if self.waterlevel == 0 then
		if self.in_water then
			sound(self, CHAN_BODY, "misc/outwater.wav", 1, ATTN_NORM)
			self.in_water = false
		end
		return
	end

	local dmgtime = self.dmgtime or 0
	if self.watertype == CONTENT_LAVA and dmgtime < time() then
		local suited = (self.radsuit_finished or 0) > time()
		self.dmgtime = time() + (if suited then 1 else 0.2)
		T_Damage(self, world, world, 10 * self.waterlevel)
	elseif self.watertype == CONTENT_SLIME then
		if dmgtime < time() and (self.radsuit_finished or 0) < time() then
			self.dmgtime = time() + 1
			T_Damage(self, world, world, 4 * self.waterlevel)
		end
	end

	if not self.in_water then
		if self.watertype == CONTENT_LAVA then
			sound(self, CHAN_BODY, "player/inlava.wav", 1, ATTN_NORM)
		elseif self.watertype == CONTENT_WATER then
			sound(self, CHAN_BODY, "player/inh2o.wav", 1, ATTN_NORM)
		elseif self.watertype == CONTENT_SLIME then
			sound(self, CHAN_BODY, "player/slimbrn2.wav", 1, ATTN_NORM)
		end
		self.in_water = true
		self.dmgtime = 0
	end
end

-- A shot, as soon as it arrives. The command behind it finds the weapon busy,
-- so nothing fires twice (proposal 0050).
function ClientFire(self, seed)
	if intermission_running ~= 0 or self.deadflag ~= DEAD_NO then
		return
	end
	if self.view_ofs == vector.zero or time() < (self.attack_finished or 0) then
		return
	end
	self.fire_seed = seed
	W_Attack(self)
	self.fire_seed = nil
end

-- Runs before each of the player's moves.
function PlayerPreThink(self)
	if intermission_running ~= 0 then
		-- Otherwise a button could be missed between the think tics.
		IntermissionThink(self)
		return
	end
	if self.view_ofs == vector.zero then
		-- Intermission or finale.
		return
	end
	CheckRules(self)
	if self.deadflag == DEAD_DYING then
		return
	end
	if self.deadflag >= DEAD_DEAD then
		PlayerDeathThink(self)
		return
	end
	WaterMove(self)
	if self.button2 then
		PlayerJump(self)
	else
		self.jump_released = true
	end
end

-- Runs after each of the player's moves.
function PlayerPostThink(self)
	if self.view_ofs == vector.zero then
		-- Intermission or finale.
		return
	end
	if self.deadflag ~= DEAD_NO then
		return
	end
	W_WeaponFrame(self)
	local fall = self.jump_flag or 0
	if fall < -300 and self.onground and self.health > 0 then
		if self.watertype == CONTENT_WATER then
			sound(self, CHAN_BODY, "player/h2ojump.wav", 1, ATTN_NORM)
		elseif fall < -650 then
			T_Damage(self, world, world, 5)
			sound(self, CHAN_VOICE, "player/land2.wav", 1, ATTN_NORM)
			self.deathtype = "falling"
		else
			sound(self, CHAN_VOICE, "player/land.wav", 1, ATTN_NORM)
		end
		self.jump_flag = 0
	end
	if not self.onground then
		self.jump_flag = self.velocity.z
	end
	CheckPowerups(self)
end

-- A thing crossed a liquid's surface.
-- A monster that lands hard makes a noise, from `SV_Physics_Step`.
function entity_landed(self)
	sound(self, CHAN_AUTO, "demon/dland2.wav", 1, ATTN_NORM)
end

function entity_splash(self)
	sound(self, CHAN_AUTO, "misc/h2ohit1.wav", 1, ATTN_NORM)
end

-- The engine readies the player once the map has spawned.
-- Who a player is. Their team is their pants colour, plus one so that 0 can
-- mean none, from `Host_Color_f` and `Host_Spawn_f`.
function ClientInfo(self, netname, colors)
	self.netname = netname
	self.team = bit32.band(colors, 15) + 1
end

function ClientConnect(self, name, saved, netname)
	self.netname = netname or "player"
	self.frags = self.frags or 0
	if world then
		world.model = "maps/" .. name .. ".bsp"
		engine.level_title(world.message or "")
	end
	if saved then
		parms = saved
	else
		SetNewParms()
	end
	self.start_parms = parms
	bprint(self.netname .. " entered the game")
	PutClientInServer(self)
	-- A client connecting during an intermission can cause problems.
	if intermission_running ~= 0 then
		ExitIntermission()
	end
end

-- Places that are looked up, which have to stay in the world to be found.
function info_intermission(_self) end
function info_player_start(_self) end
function info_player_start2(_self) end
function info_player_deathmatch(_self) end
function info_player_coop(_self) end
