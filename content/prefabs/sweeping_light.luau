-- A moving light with shadows, going round a loop. params: reach, color, height,
-- period, radius.
return {
	build = function(params)
		local reach = params.reach or 160
		local height = params.height or 96
		return {
			{
				block = "entity",
				type = "live_light",
				origin = { reach, 0, height },
				color = params.color or { 1, 0.8, 0.5 },
				radius = params.radius or 420,
				shadows = true,
				period = params.period or 8,
				path = {
					reach,
					0,
					height,
					0,
					reach,
					height,
					-reach,
					0,
					height,
					0,
					-reach,
					height,
				},
			},
		}
	end,
}
