-- A spike of rock, rising from the origin, or hanging if `down` is set. params:
-- radius, length, material, seed, down.
return {
	build = function(params)
		local radius = params.radius or 64
		local length = params.length or 256
		local material = params.material or "stone"
		local random = kit.random(params.seed or 1)
		local way = params.down and -1 or 1
		local out = {}
		local rings = 4
		local turn = random() * math.pi
		for index = 0, rings - 1 do
			local share = 1 - index / rings
			local z0, z1 = length * index / rings, length * (index + 1) / rings
			local outline = kit.ring(radius * share, 6, turn + index * 0.4)
			if way < 0 then
				z0, z1 = -z1, -z0
			end
			table.insert(out, kit.prism(material, outline, z0, z1))
		end
		return out
	end,
}
