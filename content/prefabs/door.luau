-- A sliding door entity across y. params: width, height, name, speed, material.
return {
	build = function(params)
		local width = params.width or 96
		local height = params.height or 128
		return {
			{
				block = "entity",
				name = params.name,
				type = "func_door",
				angle = -1,
				speed = params.speed or 120,
				wait = 3,
				lip = 8,
				children = {
					{
						block = "brush",
						material = params.material or "metal",
						cuboid = { 0, -width / 2, 0, 16, width / 2, height },
					},
				},
			},
		}
	end,
}
