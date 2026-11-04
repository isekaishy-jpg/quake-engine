-- The engine's side of a script: entities as tables, with engine fields kept in
-- Rust. See proposal 0025.

local ENGINE_FIELDS = {
	origin = true,
	velocity = true,
	mins = true,
	maxs = true,
	size = true,
	absmin = true,
	absmax = true,
	solid = true,
	movetype = true,
	nextthink = true,
	ltime = true,
	visible = true,
	view = true,
	waterlevel = true,
	watertype = true,
	onground = true,
	button2 = true,
	waterjump = true,
	noclip = true,
}

-- Map fields that aren't text.
local NUMBER_FIELDS = {
	speed = true, wait = true, lip = true, dmg = true, health = true, spawnflags = true,
	sounds = true, height = true, delay = true, style = true, count = true, t_length = true,
	t_width = true, worldtype = true, angle = true, volume = true,
}
local VECTOR_FIELDS = { origin = true, angles = true, mangle = true, mins = true, maxs = true }

local entities = {}

local Entity = {}

function Entity.__index(entity, key)
	if ENGINE_FIELDS[key] then
		return engine.get(rawget(entity, "_id"), key)
	end
	return nil
end

function Entity.__newindex(entity, key, value)
	if ENGINE_FIELDS[key] then
		engine.set(rawget(entity, "_id"), key, value)
	else
		rawset(entity, key, value)
	end
end

local function wrap(id)
	local entity = setmetatable({ _id = id }, Entity)
	entities[id] = entity
	return entity
end

function spawn()
	return wrap(engine.spawn())
end

function remove(entity)
	local id = rawget(entity, "_id")
	entities[id] = nil
	rawset(entity, "_removed", true)
	engine.remove(id)
end

function is_removed(entity)
	return rawget(entity, "_removed") == true
end

-- The next entity after `from` whose `field` is `value`, in slot order. A nil
-- `from` starts at the beginning, as `find (world, ...)` does.
function find(from, field, value)
	local passed = from == nil
	for _, id in engine.ids() do
		local entity = entities[id]
		if entity then
			if passed and rawget(entity, field) == value then
				return entity
			end
			if entity == from then
				passed = true
			end
		end
	end
	return nil
end

-- Every entity whose `field` is `value`, in slot order.
function find_all(field, value)
	local found = {}
	for _, id in engine.ids() do
		local entity = entities[id]
		if entity and rawget(entity, field) == value then
			table.insert(found, entity)
		end
	end
	return found
end

function setmodel(entity, model)
	rawset(entity, "model", model)
	if model then
		engine.set_model(rawget(entity, "_id"), model)
	end
end

function setsize(entity, mins, maxs)
	entity.mins = mins
	entity.maxs = maxs
end

function setorigin(entity, origin)
	entity.origin = origin
end

function time()
	return engine.time()
end

function sound(entity, channel, sample, volume, attenuation)
	if sample ~= nil and sample ~= "" then
		engine.sound(rawget(entity, "_id"), channel, sample, volume, attenuation)
	end
end

function ambientsound(origin, sample, volume, attenuation)
	engine.ambientsound(origin, sample, volume, attenuation)
end

local random_state = 1

-- A number from 0 up to 1, the same each run, so a game replays.
function random()
	random_state = random_state * 16807 % 2147483647
	return random_state / 2147483647
end

-- The forward, right and up directions of `angles`, from `AngleVectors`.
function makevectors(angles)
	local pitch = math.rad(angles.x)
	local yaw = math.rad(angles.y)
	local roll = math.rad(angles.z)
	local sp, cp = math.sin(pitch), math.cos(pitch)
	local sy, cy = math.sin(yaw), math.cos(yaw)
	local sr, cr = math.sin(roll), math.cos(roll)
	local forward = vector.create(cp * cy, cp * sy, -sp)
	local right = vector.create(-sr * sp * cy + cr * sy, -sr * sp * sy - cr * cy, -sr * cp)
	local up = vector.create(cr * sp * cy + sr * sy, cr * sp * sy - sr * cy, cr * cp)
	return forward, right, up
end

local function parse_vector(text)
	local x, y, z = string.match(text, "(%S+)%s+(%S+)%s+(%S+)")
	return vector.create(tonumber(x) or 0, tonumber(y) or 0, tonumber(z) or 0)
end

-- Fills an entity from a map's fields, as `ED_ParseEdict` does.
local function fill(entity, fields)
	for _, pair in fields do
		local key, value = pair[1], pair[2]
		if key == "angle" then
			local angle = tonumber(value) or 0
			entity.angles = vector.create(0, angle, 0)
		elseif key == "origin" then
			entity.origin = parse_vector(value)
		elseif VECTOR_FIELDS[key] then
			rawset(entity, key, parse_vector(value))
		elseif NUMBER_FIELDS[key] then
			rawset(entity, key, tonumber(value) or 0)
		elseif key ~= "_id" then
			rawset(entity, key, value)
		end
	end
	if rawget(entity, "angles") == nil then
		rawset(entity, "angles", vector.zero)
	end
	for key in NUMBER_FIELDS do
		if rawget(entity, key) == nil then
			rawset(entity, key, 0)
		end
	end
end

-- Spawns one map entity through `spawner`, the function named after its class.
-- With no function it's removed, as `ED_LoadFromFile` does. A game can also
-- define `spawn_filter` to turn entities away.
function spawn_from_map(fields, spawner)
	local entity = spawn()
	fill(entity, fields)
	if spawner and (spawn_filter == nil or spawn_filter(entity)) then
		spawner(entity)
	else
		remove(entity)
	end
end

-- The player, from the slot the engine gave it.
function adopt_player(id)
	local player = wrap(id)
	rawset(player, "classname", "player")
	rawset(player, "health", 100)
	rawset(player, "items", 0)
	rawset(player, "angles", vector.zero)
	return player
end

-- Runs a callback from the engine.
function dispatch(kind, id, other_id)
	local entity = entities[id]
	if entity == nil then
		return
	end
	local callback = rawget(entity, kind)
	if callback then
		callback(entity, entities[other_id])
	end
end
