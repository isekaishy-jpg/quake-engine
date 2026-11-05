-- A teleporter pad, with where it goes. params: to (x y z), name, size.
return {
	build = function(params)
		local size = params.size or 64
		local name = params.name or "portal"
		local to = params.to or { 0, 0, 64 }
		return {
			{
				block = "entity",
				type = "trigger_teleport",
				target = name,
				children = {
					{
						block = "brush",
						material = "hidden",
						cuboid = { 0, 0, 0, size, size, 96 },
					},
				},
			},
			{ block = "brush", material = "light_panel", cuboid = { 0, 0, -2, size, size, 0 } },
			{
				block = "entity",
				name = name,
				type = "info_teleport_destination",
				origin = to,
			},
		}
	end,
}
