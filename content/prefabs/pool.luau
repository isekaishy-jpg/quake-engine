-- A sunken pool. params: size, depth, liquid (water, slime or lava), material.
return {
	build = function(params)
		local size = params.size or 256
		local depth = params.depth or 96
		local liquid = params.liquid or "water"
		local material = params.material or "stone"
		local t = 16
		return {
			{
				block = "brush",
				material = material,
				cuboid = { -t, -t, -depth - t, size + t, size + t, -depth },
			},
			{ block = "brush", material = material, cuboid = { -t, -t, -depth, 0, size + t, 0 } },
			{
				block = "brush",
				material = material,
				cuboid = { size, -t, -depth, size + t, size + t, 0 },
			},
			{ block = "brush", material = material, cuboid = { 0, -t, -depth, size, 0, 0 } },
			{
				block = "brush",
				material = material,
				cuboid = { 0, size, -depth, size, size + t, 0 },
			},
			{
				block = "brush",
				contents = liquid,
				material = liquid,
				cuboid = { 0, 0, -depth, size, size, -8 },
			},
		}
	end,
}
