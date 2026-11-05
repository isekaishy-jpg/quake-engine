-- A closed room with its corner at the origin. params: size (x y z), wall, floor, ceiling.
return {
	build = function(params)
		local size = params.size or { 512, 512, 256 }
		local x, y, z = size[1], size[2], size[3]
		local wall = params.wall or "brick"
		local floor = params.floor or "stone"
		local ceiling = params.ceiling or "metal"
		local t = 32
		return {
			{ block = "brush", material = floor, cuboid = { -t, -t, -t, x + t, y + t, 0 } },
			{ block = "brush", material = ceiling, cuboid = { -t, -t, z, x + t, y + t, z + t } },
			{ block = "brush", material = wall, cuboid = { -t, -t, 0, 0, y + t, z } },
			{ block = "brush", material = wall, cuboid = { x, -t, 0, x + t, y + t, z } },
			{ block = "brush", material = wall, cuboid = { 0, -t, 0, x, 0, z } },
			{ block = "brush", material = wall, cuboid = { 0, y, 0, x, y + t, z } },
		}
	end,
}
