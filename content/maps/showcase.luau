-- The showcase's own entity types, which its prefabs place.

-- A guard that walks a closed spline, by how far round it is.
function patrol_walker(self)
	self.solid = SOLID_NOT
	self.share = engine.seeded(self, 0)
	setmodel(self, "progs/soldier.mdl")
	self.nextthink = time() + 0.1
	self.think = function(me)
		me.share = (me.share + 0.004) % 1
		local place = engine.spline_point(me.route, me.share)
		if place then
			setorigin(me, place)
		end
		me.nextthink = time() + 0.05
	end
	-- Asleep it didn't move, so it catches up by how long that was.
	self.wake = function(me)
		me.share = (me.share + engine.slept * 0.08) % 1
	end
end

-- Walks to a goal round whatever's in the way.
function seeker(self)
	self.solid = SOLID_NOT
	setmodel(self, "progs/dog.mdl")
	self.goal = engine.parse_vector(self.goal)
	self.nextthink = time() + 0.1
	self.think = function(me)
		local path = engine.find_path(me.origin, me.goal)
		if path and path[2] then
			local step = path[2] - me.origin
			local length = vector.magnitude(step)
			if length > 1 then
				setorigin(me, me.origin + step * math.min(4 / length, 1))
			end
		end
		me.nextthink = time() + 0.05
	end
end

-- Says something when it's touched.
function tell_tale(self)
	self.solid = SOLID_TRIGGER
	setsize(self, vector.create(-16, -16, -16), vector.create(16, 16, 16))
	self.touch = function(me, other)
		if other.classname == "player" and not me.told then
			me.told = true
			engine.print(me.message)
		end
	end
end
