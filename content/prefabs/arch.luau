-- A wall with a round-headed opening, along x, centred on the origin. params:
-- width, spring (where the curve starts), length, height, thickness, material,
-- stone (for the ring round the opening), segments.
return {
	build = function(params)
		local width = params.width or 128
		local spring = params.spring or 96
		local length = params.length or width + 128
		local height = params.height or spring + width / 2 + 64
		local thick = params.thickness or 32
		local wall = params.material or "stone"
		local stone = params.stone or wall
		local segments = params.segments or 8
		local radius = width / 2
		local band = params.band or 16
		local half = thick / 2
		local out = {}
		local side = length / 2
		-- Either side of the opening, and over it.
		table.insert(out, kit.box(wall, -side, -half, 0, -radius - band, half, height))
		table.insert(out, kit.box(wall, radius + band, -half, 0, side, half, height))
		local crown = spring + radius + band
		if height > crown then
			table.insert(
				out,
				kit.box(wall, -radius - band, -half, crown, radius + band, half, height)
			)
		end
		-- Jambs, up to where the curve starts.
		table.insert(out, kit.box(stone, -radius - band, -half - 2, 0, -radius, half + 2, spring))
		table.insert(out, kit.box(stone, radius, -half - 2, 0, radius + band, half + 2, spring))
		for index = 0, segments - 1 do
			local a0 = math.pi - index / segments * math.pi
			local a1 = math.pi - (index + 1) / segments * math.pi
			local function at(angle, reach)
				return { math.cos(angle) * reach, spring + math.sin(angle) * reach }
			end
			-- The ring of stones.
			table.insert(
				out,
				kit.prism_y(
					stone,
					{ at(a0, radius + band), at(a0, radius), at(a1, radius), at(a1, radius + band) },
					-half - 2,
					half + 2
				)
			)
			-- What fills the corner over it.
			local low0, low1 = at(a0, radius + band), at(a1, radius + band)
			table.insert(
				out,
				kit.prism_y(
					wall,
					{ low0, low1, { low1[1], crown }, { low0[1], crown } },
					-half,
					half
				)
			)
		end
		return out
	end,
}
