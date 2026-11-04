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
	angles = true,
	frame = true,
	skin = true,
	effects = true,
	weaponmodel = true,
	weaponframe = true,
	flags = true,
	avelocity = true,
	button0 = true,
	impulse = true,
	v_angle = true,
	punchangle = true,
	weaponhidden = true,
	ideal_yaw = true,
	yaw_speed = true,
	dead = true,
	frozen = true,
}

-- Map fields that aren't text.
local NUMBER_FIELDS = {
	speed = true, wait = true, lip = true, dmg = true, health = true, spawnflags = true,
	sounds = true, height = true, delay = true, style = true, count = true, t_length = true,
	t_width = true, worldtype = true, angle = true, volume = true,
}
local VECTOR_FIELDS = {
	origin = true,
	angles = true,
	mangle = true,
	mins = true,
	maxs = true,
	velocity = true,
	avelocity = true,
}

-- Fields that hold functions, which a map's text can't.
local FUNCTION_FIELDS = {
	think = true,
	touch = true,
	use = true,
	blocked = true,
	th_die = true,
	th_pain = true,
}

-- Engine fields a map can set from its text, which are numbers.
local MAP_NUMBER_FIELDS = {
	solid = true,
	movetype = true,
	nextthink = true,
	frame = true,
	skin = true,
	effects = true,
	flags = true,
}

local entities = {}

local Entity = {}

-- Fields the engine has to hear about every time. A metamethod only sees a write
-- to a key that isn't there, so these live under other names.
local WATCHED_FIELDS = { model = "_model", owner = "_owner" }

-- What a field reads as until it's set. A game adds its own with
-- `set_field_defaults`.
local field_defaults = {
	origin = vector.zero,
	velocity = vector.zero,
	mins = vector.zero,
	maxs = vector.zero,
	size = vector.zero,
	absmin = vector.zero,
	absmax = vector.zero,
	angles = vector.zero,
	avelocity = vector.zero,
	v_angle = vector.zero,
	flags = 0,
}

function set_field_defaults(defaults)
	for field, value in defaults do
		field_defaults[field] = value
	end
end

function Entity.__index(entity, key)
	if ENGINE_FIELDS[key] then
		local id = rawget(entity, "_id")
		local value = if id >= 0 then engine.get(id, key) else nil
		if value ~= nil then
			return value
		end
		return field_defaults[key]
	end
	local watched = WATCHED_FIELDS[key]
	if watched then
		return rawget(entity, watched)
	end
	return field_defaults[key]
end

function Entity.__newindex(entity, key, value)
	if ENGINE_FIELDS[key] then
		engine.set(rawget(entity, "_id"), key, value)
		return
	end
	local watched = WATCHED_FIELDS[key]
	if watched == nil then
		rawset(entity, key, value)
		return
	end
	rawset(entity, watched, value)
	if key == "owner" then
		engine.set_owner(rawget(entity, "_id"), value and rawget(value, "_id"))
	else
		-- An empty model hides what's there, as `SV_WriteEntitiesToClient` does.
		engine.set(rawget(entity, "_id"), "visible", value ~= nil and value ~= "")
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
	entity.model = model
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

-- What `makevectors` last worked out.
v_forward = vector.create(1, 0, 0)
v_right = vector.create(0, -1, 0)
v_up = vector.create(0, 0, 1)

-- What `traceline` last found.
trace_fraction = 1
trace_endpos = vector.zero
trace_plane_normal = vector.zero
trace_ent = nil

trace_inopen = false
trace_inwater = false

-- Stands in for the world in a map that has none.
local no_world = setmetatable({ _id = -1 }, Entity)

-- The player, once there is one.
the_player = nil

-- `entity`, or the world for none, as QuakeC's empty entity fields are.
function ent(entity)
	return entity or world or no_world
end

-- Whether `entity` is something, which the world isn't.
function exists(entity)
	return entity ~= nil and entity ~= world and entity ~= no_world
end

-- What a saved game reads and fills in.
function saved_entities()
	return entities
end

function no_world_entity()
	return no_world
end

function restored_entity(id, removed)
	if removed then
		return setmetatable({ _id = id }, Entity)
	end
	local entity = entities[id] or wrap(id)
	table.clear(entity)
	return entity
end

function random_seed(value)
	if value then
		random_state = value
	end
	return random_state
end

-- The next entity after `from` whose `field` is `value`, or the world.
function find_entity(from, field, value)
	return ent(find(if exists(from) then from else nil, field, value))
end

function with_x(value, x)
	return vector.create(x, value.y, value.z)
end

function with_y(value, y)
	return vector.create(value.x, y, value.z)
end

function with_z(value, z)
	return vector.create(value.x, value.y, z)
end

-- The engine's number for `entity`, if it's still there.
local function id_of(entity)
	if exists(entity) and not is_removed(entity) then
		return rawget(entity, "_id")
	end
	return nil
end

function walkmove(entity, yaw, distance)
	return engine.walkmove(rawget(entity, "_id"), yaw, distance, id_of(entity.enemy))
end

function movetogoal(entity, distance)
	local id = rawget(entity, "_id")
	engine.movetogoal(id, distance, id_of(entity.goalentity), id_of(entity.enemy))
end

function ChangeYaw(entity)
	engine.changeyaw(rawget(entity, "_id"))
end

function checkbottom(entity)
	return engine.checkbottom(rawget(entity, "_id"))
end

-- The player, if `entity` could be looking at them, from `PF_checkclient`.
function checkclient(entity)
	local player = the_player
	if player == nil or player.health <= 0 then
		return ent(nil)
	end
	if engine.player_in_view(rawget(entity, "_id"), entity.view_ofs) then
		return player
	end
	return ent(nil)
end

-- From `PF_rint`.
function rint(value)
	if value > 0 then
		return math.floor(value + 0.5)
	end
	return math.ceil(value - 0.5)
end

function traceline(start, finish, nomonsters, ignore)
	local fraction, endpos, normal, hit, inopen, inwater = engine.traceline(
		start,
		finish,
		nomonsters,
		ignore and rawget(ignore, "_id")
	)
	trace_fraction = fraction
	trace_endpos = endpos
	trace_plane_normal = normal
	trace_ent = hit and entities[hit] or ent(nil)
	trace_inopen = inopen
	trace_inwater = inwater
end

function pointcontents(point)
	return engine.pointcontents(point)
end

function droptofloor(entity)
	return engine.droptofloor(rawget(entity, "_id"))
end

function normalize(value)
	if value == vector.zero then
		return value
	end
	return vector.normalize(value)
end

-- The yaw of a direction, from `PF_vectoyaw`.
function vectoyaw(value)
	if value.x == 0 and value.y == 0 then
		return 0
	end
	local yaw = math.floor(math.deg(math.atan2(value.y, value.x)))
	if yaw < 0 then
		yaw += 360
	end
	return yaw
end

-- Pitch and yaw of a direction, from `PF_vectoangles`.
function vectoangles(value)
	local yaw, pitch = 0, 0
	if value.x == 0 and value.y == 0 then
		pitch = if value.z > 0 then 90 else 270
	else
		yaw = math.floor(math.deg(math.atan2(value.y, value.x)))
		if yaw < 0 then
			yaw += 360
		end
		local forward = math.sqrt(value.x * value.x + value.y * value.y)
		pitch = math.floor(math.deg(math.atan2(value.z, forward)))
		if pitch < 0 then
			pitch += 360
		end
	end
	return vector.create(pitch, yaw, 0)
end

-- Everything solid within `radius` of `origin`, from `PF_findradius`.
function findradius(origin, radius)
	local found = {}
	for _, id in engine.ids() do
		local entity = entities[id]
		if entity and entity.solid ~= 0 then
			local middle = entity.origin + (entity.mins + entity.maxs) * 0.5
			if vector.magnitude(origin - middle) <= radius then
				table.insert(found, entity)
			end
		end
	end
	return found
end

function temp_entity(kind, origin, finish, from)
	engine.temp_entity(kind, origin, finish or origin, from and rawget(from, "_id"))
end

function particle(origin, direction, color, count)
	engine.particle(origin, direction, color, count)
end

function sprint(_entity, message)
	engine.print(message)
end

function bprint(message)
	engine.print(message)
end

function centerprint(_entity, message)
	engine.centerprint(message)
end

function stuffcmd(_entity, text)
	engine.stuffcmd(text)
end

function localcmd(text)
	engine.localcmd(text)
end

-- What the host set for this level. A game reads these with `cvar`.
local level_settings = {}

function cvar(name)
	return level_settings[name] or 0
end

-- Takes what the host set. A game adds to this with `level_started`.
function start_level(name, settings, flags)
	level_settings = settings
	if level_started then
		level_started(name, flags)
	end
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
	v_forward, v_right, v_up = forward, right, up
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
			entity[key] = parse_vector(value)
		elseif NUMBER_FIELDS[key] then
			rawset(entity, key, tonumber(value) or 0)
		elseif MAP_NUMBER_FIELDS[key] then
			entity[key] = tonumber(value) or 0
		elseif key ~= "_id" and not ENGINE_FIELDS[key] and not FUNCTION_FIELDS[key] then
			entity[key] = value
		end
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
	the_player = player
	rawset(player, "classname", "player")
	rawset(player, "health", 100)
	rawset(player, "items", 0)
	return player
end

-- Runs a callback from the engine.
function dispatch(kind, id, other_id)
	local entity = entities[id]
	if entity == nil then
		return
	end
	if kind == "splash" then
		if entity_splash then
			entity_splash(entity)
		end
		return
	end
	if kind == "land" then
		if entity_landed then
			entity_landed(entity)
		end
		return
	end
	local callback = rawget(entity, kind)
	if callback then
		callback(entity, other_id and entities[other_id] or ent(nil))
	end
end
