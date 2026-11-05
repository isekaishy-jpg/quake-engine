-- A polished slab. params: size, height.
return {
	build = function(params)
		local size = params.size or 256
		return {
			{
				block = "brush",
				material = "polished",
				cuboid = { 0, 0, 0, size, size, params.height or 4 },
			},
		}
	end,
}
