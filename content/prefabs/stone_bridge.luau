-- An arched stone bridge along x from the origin. params: length, width, rise,
-- depth (of its piers), material, parapet, segments.
return {
	build = function(params)
		local length = params.length or 384
		local width = params.width or 128
		local rise = params.rise or 48
		local depth = params.depth or 96
		local material = params.material or "stone"
		local parapet = params.parapet or material
		local segments = params.segments or 8
		local thick = params.thickness or 32
		local wall = params.wall or 12
		local high = params.height or 36
		local out = {}
		local function level(share)
			return math.sin(share * math.pi) * rise
		end
		for index = 0, segments - 1 do
			local s0, s1 = index / segments, (index + 1) / segments
			local x0, x1 = s0 * length, s1 * length
			local z0, z1 = level(s0), level(s1)
			local deck = { { x0, z0 - thick }, { x1, z1 - thick }, { x1, z1 }, { x0, z0 } }
			table.insert(out, kit.prism_y(material, deck, 0, width))
			local rail = { { x0, z0 }, { x1, z1 }, { x1, z1 + high }, { x0, z0 + high } }
			table.insert(out, kit.prism_y(parapet, rail, 0, wall))
			table.insert(out, kit.prism_y(parapet, rail, width - wall, width))
		end
		-- Piers at each end, down into what it crosses.
		table.insert(out, kit.box(material, -32, -8, -depth, 16, width + 8, 0))
		table.insert(out, kit.box(material, length - 16, -8, -depth, length + 32, width + 8, 0))
		return out
	end,
}
