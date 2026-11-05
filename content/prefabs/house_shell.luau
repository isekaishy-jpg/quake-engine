-- A ruined house from its corner: four broken walls and what's left of a floor.
-- params: width, depth, height, material, door (which wall has a gap: north,
-- south, east or west), seed.
return {
	build = function(params)
		local width = params.width or 320
		local depth = params.depth or 256
		local height = params.height or 224
		local material = params.material or "stone"
		local door = params.door or "south"
		local seed = params.seed or 1
		local t = params.thickness or 24
		local gap = params.gap or 80
		local out = {}
		local function wall(x, y, length, across, part)
			local random = kit.random(seed * 13 + part)
			local spans = math.max(2, math.floor(length / 48))
			local step = length / spans
			local top = height - random() * height * 0.25
			for index = 0, spans - 1 do
				local next_top = height - random() * height * 0.55
				local a, b = index * step, (index + 1) * step
				local slope = (next_top - top) / step
				if across then
					table.insert(
						out,
						kit.sloped(
							material,
							{ { x, y + a }, { x + t, y + a }, { x + t, y + b }, { x, y + b } },
							0,
							top - slope * (y + a),
							0,
							slope
						)
					)
				else
					table.insert(
						out,
						kit.sloped(
							material,
							{ { x + a, y }, { x + b, y }, { x + b, y + t }, { x + a, y + t } },
							0,
							top - slope * (x + a),
							slope,
							0
						)
					)
				end
				top = next_top
			end
		end
		local mid_x, mid_y = (width - gap) / 2, (depth - gap) / 2
		if door == "south" then
			wall(0, 0, mid_x, false, 1)
			wall(mid_x + gap, 0, width - mid_x - gap, false, 2)
		else
			wall(0, 0, width, false, 1)
		end
		if door == "north" then
			wall(0, depth - t, mid_x, false, 3)
			wall(mid_x + gap, depth - t, width - mid_x - gap, false, 4)
		else
			wall(0, depth - t, width, false, 3)
		end
		if door == "west" then
			wall(0, t, mid_y - t, true, 5)
			wall(0, mid_y + gap, depth - mid_y - gap - t, true, 6)
		else
			wall(0, t, depth - t * 2, true, 5)
		end
		if door == "east" then
			wall(width - t, t, mid_y - t, true, 7)
			wall(width - t, mid_y + gap, depth - mid_y - gap - t, true, 8)
		else
			wall(width - t, t, depth - t * 2, true, 7)
		end
		-- A floor that's half fallen in, if there's height for one.
		if height > 160 and params.upper ~= false then
			table.insert(
				out,
				kit.box(params.beams or "wood", t, t, 128, width * 0.55, depth - t, 136)
			)
		end
		return out
	end,
}
