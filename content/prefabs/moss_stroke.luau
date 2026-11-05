-- A slab of paintable stone with a stroke of moss across it. params: size.
return {
	build = function(params)
		local size = params.size or 192
		return {
			{
				block = "brush",
				material = "painted_stone",
				cuboid = { 0, 0, 0, size, size, 8 },
			},
			{
				block = "paint",
				layer = 0,
				radius = 40,
				points = { 0, size / 3, 8, size / 2, size / 2, 8, size, size / 3, 8 },
			},
		}
	end,
}
