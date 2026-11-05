-- Shapes prefabs share, given to each as `kit` (proposal 0097). Everything here
-- makes plain brushes, so once placed none of it is special.
local kit = {}

local function normalized(x, y, z)
	local length = math.sqrt(x * x + y * y + z * z)
	return x / length, y / length, z / length
end

local function face(x, y, z, distance)
	return { block = "face", normal = { x, y, z }, distance = distance }
end

-- A brush from planes.
function kit.solid(material, faces)
	return { block = "brush", material = material, children = faces }
end

-- A box between two corners.
function kit.box(material, x0, y0, z0, x1, y1, z1)
	return { block = "brush", material = material, cuboid = { x0, y0, z0, x1, y1, z1 } }
end

-- Points round a circle, going anticlockwise seen from above.
function kit.ring(radius, sides, phase)
	local points = {}
	for index = 0, sides - 1 do
		local angle = (phase or 0) + index / sides * math.pi * 2
		table.insert(points, { math.cos(angle) * radius, math.sin(angle) * radius })
	end
	return points
end

-- The planes round a convex outline, anticlockwise, on the axes `a` and `b`,
-- where `make` turns an outward direction into a normal.
local function sides(points, make)
	local faces = {}
	for index, point in points do
		local next_point = points[index % #points + 1]
		local dx, dy = next_point[1] - point[1], next_point[2] - point[2]
		local length = math.sqrt(dx * dx + dy * dy)
		local nx, ny = dy / length, -dx / length
		table.insert(faces, make(nx, ny, nx * point[1] + ny * point[2]))
	end
	return faces
end

-- A convex outline stood up from z0 to z1.
function kit.prism(material, points, z0, z1)
	local faces = sides(points, function(nx, ny, distance)
		return face(nx, ny, 0, distance)
	end)
	table.insert(faces, face(0, 0, 1, z1))
	table.insert(faces, face(0, 0, -1, -z0))
	return kit.solid(material, faces)
end

-- A convex outline in x and z, running from y0 to y1. Anticlockwise here means
-- seen looking along +y, so x goes right and z goes up.
function kit.prism_y(material, points, y0, y1)
	local faces = sides(points, function(nx, nz, distance)
		return face(nx, 0, nz, distance)
	end)
	table.insert(faces, face(0, 1, 0, y1))
	table.insert(faces, face(0, -1, 0, -y0))
	return kit.solid(material, faces)
end

-- A convex outline in y and z, running from x0 to x1.
function kit.prism_x(material, points, x0, x1)
	local faces = sides(points, function(ny, nz, distance)
		return face(0, ny, nz, distance)
	end)
	table.insert(faces, face(1, 0, 0, x1))
	table.insert(faces, face(-1, 0, 0, -x0))
	return kit.solid(material, faces)
end

-- A convex outline from z0 up to a slope, where its top at (x, y) is
-- `top + slope_x * x + slope_y * y`.
function kit.sloped(material, points, z0, top, slope_x, slope_y)
	local faces = sides(points, function(nx, ny, distance)
		return face(nx, ny, 0, distance)
	end)
	local x, y, z = normalized(-slope_x, -slope_y, 1)
	table.insert(faces, face(x, y, z, top * z))
	table.insert(faces, face(0, 0, -1, -z0))
	return kit.solid(material, faces)
end

-- A square bar between two points, `width` across, for beams, rails and struts.
function kit.beam(material, from, to, width)
	local dx, dy, dz = normalized(to[1] - from[1], to[2] - from[2], to[3] - from[3])
	-- Anything not along the beam gives a first side.
	local ax, ay, az = 0, 0, 1
	if math.abs(dz) > 0.9 then
		ax, ay, az = 1, 0, 0
	end
	local sx, sy, sz = normalized(dy * az - dz * ay, dz * ax - dx * az, dx * ay - dy * ax)
	local ux, uy, uz = dy * sz - dz * sy, dz * sx - dx * sz, dx * sy - dy * sx
	local half = width / 2
	local function along(x, y, z, point)
		return x * point[1] + y * point[2] + z * point[3]
	end
	return kit.solid(material, {
		face(dx, dy, dz, along(dx, dy, dz, to)),
		face(-dx, -dy, -dz, -along(dx, dy, dz, from)),
		face(sx, sy, sz, along(sx, sy, sz, from) + half),
		face(-sx, -sy, -sz, -along(sx, sy, sz, from) + half),
		face(ux, uy, uz, along(ux, uy, uz, from) + half),
		face(-ux, -uy, -uz, -along(ux, uy, uz, from) + half),
	})
end

-- A repeatable stream of numbers from 0 to 1, so a seed gives the same rubble
-- each time.
function kit.random(seed)
	local value = (seed or 1) * 2654435761 % 4294967296
	return function()
		value = (value * 1664525 + 1013904223) % 4294967296
		return value / 4294967296
	end
end

-- Adds everything in `more` to the end of `list`.
function kit.add(list, more)
	for _, item in more do
		table.insert(list, item)
	end
	return list
end

return kit
