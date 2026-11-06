-- A town house from its corner, with a pitched roof, a door and windows on its
-- -y side. params: width, depth, height, material, roof, door, window, trim, rise.
return {
	build = function(params)
		local width = params.width or 256
		local depth = params.depth or 224
		local height = params.height or 224
		local stone = params.material or "stone"
		local roof = params.roof or "wood"
		local door = params.door or "metal"
		local window = params.window or "metal"
		local trim = params.trim or stone
		local rise = params.rise or width * 0.4
		local out = {
			kit.box(stone, 0, 0, 0, width, depth, height),
			kit.box(trim, -6, -6, height - 8, width + 6, depth + 6, height),
			kit.box(trim, -4, -4, 96, width + 4, depth + 4, 104),
		}
		-- The gable ends, then the roof over them.
		local half = width / 2
		table.insert(
			out,
			kit.prism_y(
				stone,
				{ { 0, height }, { width, height }, { half, height + rise } },
				4,
				depth - 4
			)
		)
		for _, part in
			{
				{
					{ -16, height - 4 },
					{ -16, height + 4 },
					{ half, height + rise + 8 },
					{ half, height + rise },
				},
				{
					{ half, height + rise },
					{ half, height + rise + 8 },
					{ width + 16, height + 4 },
					{ width + 16, height - 4 },
				},
			}
		do
			table.insert(out, kit.prism_y(roof, part, -12, depth + 12))
		end
		-- The front.
		local door_x = half - 32
		table.insert(out, kit.box(door, door_x, -4, 0, door_x + 64, 0, 96))
		for _, x in { 24, width - 72 } do
			table.insert(out, kit.box(window, x, -3, 128, x + 48, 0, 188))
		end
		return out
	end,
}
