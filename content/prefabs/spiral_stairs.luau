-- Stairs winding up round a post, from the origin. params: radius, rise (in all),
-- step_rise, steps_per_turn, core, material, start (angle in degrees), clockwise.
return {
	build = function(params)
		local radius = params.radius or 128
		local rise = params.rise or 256
		local step_rise = params.step_rise or 16
		local per_turn = params.steps_per_turn or 16
		local core = params.core or 24
		local material = params.material or "stone"
		local post = params.post or material
		local start = math.rad(params.start or 0)
		local turn = params.clockwise and -1 or 1
		local out = { kit.prism(post, kit.ring(core, 8, 0), 0, rise) }
		local steps = math.floor(rise / step_rise)
		local slice = math.pi * 2 / per_turn
		for index = 0, steps - 1 do
			local a0 = start + turn * index * slice
			local a1 = a0 + turn * slice * 1.1
			if turn < 0 then
				a0, a1 = a1, a0
			end
			local outline = {
				{ math.cos(a0) * core, math.sin(a0) * core },
				{ math.cos(a0) * radius, math.sin(a0) * radius },
				{ math.cos(a1) * radius, math.sin(a1) * radius },
				{ math.cos(a1) * core, math.sin(a1) * core },
			}
			local top = (index + 1) * step_rise
			table.insert(out, kit.prism(material, outline, top - 12, top))
		end
		return out
	end,
}
