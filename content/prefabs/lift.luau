-- A platform that rises. params: size, height, name, material.
return {
	build = function(params)
		local size = params.size or 128
		local height = params.height or 192
		return {
			{
				block = "entity",
				name = params.name,
				type = "func_plat",
				height = height,
				speed = 150,
				children = {
					{
						block = "brush",
						material = params.material or "metal",
						cuboid = { 0, 0, height - 8, size, size, height },
					},
				},
			},
		}
	end,
}
