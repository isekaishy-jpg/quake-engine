-- A wall along y with stained glass and rays through it. params: width, height,
-- sill, top, opening, material, sun (a direction), strength.
return {
	build = function(params)
		local width = params.width or 512
		local height = params.height or 320
		local sill = params.sill or 64
		local top = params.top or 256
		local opening = params.opening or 192
		local thick = params.thickness or 32
		local material = params.material or "brick"
		local half = width / 2
		local gap = opening / 2
		return {
			{ block = "brush", material = material, cuboid = { 0, -half, 0, thick, -gap, height } },
			{ block = "brush", material = material, cuboid = { 0, gap, 0, thick, half, height } },
			{ block = "brush", material = material, cuboid = { 0, -gap, 0, thick, gap, sill } },
			{ block = "brush", material = material, cuboid = { 0, -gap, top, thick, gap, height } },
			{
				block = "brush",
				material = "stained_glass",
				cuboid = { 12, -gap, sill, 20, gap, top },
			},
			{
				block = "entity",
				type = "ray_stencil",
				image = "glass_rays",
				marks = "glass_marks",
				corner = { 16, -gap, top },
				across = { 0, opening, 0 },
				down = { 0, 0, sill - top },
				light = params.sun or { -0.75, 0.1, 0.65 },
				sun = true,
				strength = params.strength or 1.5,
				reach = params.reach or 900,
			},
		}
	end,
}
