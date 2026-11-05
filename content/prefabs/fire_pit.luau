-- A low stone ring with a flame and its light. params: size, material.
return {
	build = function(params)
		local size = params.size or 32
		local material = params.material or "stone"
		local s = size / 2
		return {
			{ block = "brush", material = material, cuboid = { -s, -s, 0, s, s, 12 } },
			{ block = "entity", type = "flame", origin = { 0, 0, 12 }, size = size, sparks = 6 },
			{
				block = "entity",
				type = "light",
				origin = { 0, 0, 40 },
				intensity = params.intensity or 260,
				color = { 1, 0.65, 0.35 },
				style = 1,
				size = 6,
			},
		}
	end,
}
