-- A gate that lifts out of a frame across y, opened by a target. params: width,
-- height, name (what opens it), material, frame.
return {
	build = function(params)
		local width = params.width or 192
		local height = params.height or 160
		local frame = params.frame or "metal"
		local half = width / 2
		return {
			kit.box(frame, -24, -half - 24, 0, 24, -half, height + 96),
			kit.box(frame, -24, half, 0, 24, half + 24, height + 96),
			kit.box(frame, -24, -half - 24, height + 96, 24, half + 24, height + 128),
			{
				block = "entity",
				type = "func_door",
				targetname = params.name,
				angle = -1,
				speed = params.speed or 60,
				wait = -1,
				lip = 8,
				children = {
					{
						block = "brush",
						material = params.material or "metal",
						cuboid = { -10, -half, 0, 10, half, height },
					},
				},
			},
		}
	end,
}
