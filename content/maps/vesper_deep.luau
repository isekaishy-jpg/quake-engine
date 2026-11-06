-- The showcase's own entity types, which its prefabs place. What an entity
-- keeps in a field is saved by its global name, so these are named functions.

-- Goes to `place`, facing the way it's going, with its legs moving through `count`
-- frames from `first`. Thinking 20 times a second, a frame lasts two thinks.
local function step_to(self, place, first, count)
	local step = place - self.origin
	if vector.magnitude(step) > 0.1 then
		self.angles = vector.create(0, math.deg(math.atan2(step.y, step.x)), 0)
	end
	setorigin(self, place)
	self.stride = ((self.stride or 0) + 1) % (count * 2)
	self.frame = first + math.floor(self.stride / 2)
end

-- Moves a guard along its route, on the soldier's prowling frames.
function patrol_walker_think(self)
	self.share = (self.share + 0.004) % 1
	local place = engine.spline_point(self.route, self.share)
	if place then
		step_to(self, place, 90, 24)
	end
	self.nextthink = time() + 0.05
end

-- Asleep it didn't move, so it catches up by how long that was.
function patrol_walker_wake(self)
	self.share = (self.share + engine.slept * 0.08) % 1
end

-- A guard that walks a closed spline, by how far round it is.
function patrol_walker(self)
	self.solid = SOLID_NOT
	self.share = engine.seeded(self, 0)
	setmodel(self, "progs/soldier.mdl")
	self.nextthink = time() + 0.1
	self.think = patrol_walker_think
	self.wake = patrol_walker_wake
end

-- Takes a seeker a step along its way.
function seeker_think(self)
	local path = engine.find_path(self.origin, self.goal)
	if path and path[2] then
		local step = path[2] - self.origin
		local length = vector.magnitude(step)
		if length > 1 then
			step_to(self, self.origin + step * math.min(4 / length, 1), 78, 8)
		else
			self.frame = 69
		end
	end
	self.nextthink = time() + 0.05
end

-- Walks to a goal round whatever's in the way.
function seeker(self)
	self.solid = SOLID_NOT
	setmodel(self, "progs/dog.mdl")
	self.goal = engine.parse_vector(self.goal)
	self.nextthink = time() + 0.1
	self.think = seeker_think
end

-- Says its message the first time a player touches it.
function tell_tale_touch(self, other)
	if other.classname == "player" and not self.told then
		self.told = true
		engine.print(self.message)
	end
end

-- Says something when it's touched.
function tell_tale(self)
	self.solid = SOLID_TRIGGER
	setsize(self, vector.create(-16, -16, -16), vector.create(16, 16, 16))
	self.touch = tell_tale_touch
end
