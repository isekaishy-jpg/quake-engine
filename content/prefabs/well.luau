-- A round stone rim, with water if it's wanted. params: radius, height, sides,
-- material, depth, liquid.
return {
	build = function(params)
		local radius = params.radius or 64
		local height = params.height or 32
		local sides = params.sides or 12
		local material = params.material or "stone"
		local rim = params.rim or 12
		local out = {}
		for index = 0, sides - 1 do
			local a0 = index / sides * math.pi * 2
			local a1 = (index + 1) / sides * math.pi * 2
			local inner, outer = radius - rim, radius
			table.insert(
				out,
				kit.prism(material, {
					{ math.cos(a0) * inner, math.sin(a0) * inner },
					{ math.cos(a0) * outer, math.sin(a0) * outer },
					{ math.cos(a1) * outer, math.sin(a1) * outer },
					{ math.cos(a1) * inner, math.sin(a1) * inner },
				}, 0, height)
			)
		end
		table.insert(out, kit.prism(material, kit.ring(radius, sides, 0), -8, 0))
		return out
	end,
}
