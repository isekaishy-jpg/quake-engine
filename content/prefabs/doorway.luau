-- A wall along y with an opening. params: width, height, opening, opening_height, thickness, material.
return {
	build = function(params)
		local width = params.width or 512
		local height = params.height or 256
		local opening = params.opening or 96
		local top = params.opening_height or 128
		local thick = params.thickness or 32
		local material = params.material or "brick"
		local half = width / 2
		local gap = opening / 2
		return {
			{ block = "brush", material = material, cuboid = { 0, -half, 0, thick, -gap, height } },
			{ block = "brush", material = material, cuboid = { 0, gap, 0, thick, half, height } },
			{ block = "brush", material = material, cuboid = { 0, -gap, top, thick, gap, height } },
		}
	end,
}
