-- A rough face of rock along x from the origin, standing proud of what's behind
-- it, to break up a straight cave wall. params: length, height, depth, material,
-- seed.
return {
	build = function(params)
		local length = params.length or 512
		local height = params.height or 512
		local depth = params.depth or 96
		local material = params.material or "stone"
		local random = kit.random(params.seed or 1)
		local out = {}
		local x = 0
		while x < length do
			local width = 96 + random() * 192
			local reach = depth * (0.35 + random() * 0.65)
			local top = height * (0.45 + random() * 0.55)
			local lean = (random() - 0.5) * 0.5
			local outline = {
				{ x, 0 },
				{ x + width, 0 },
				{ x + width * 0.8, reach },
				{ x + width * 0.2, reach * (0.7 + random() * 0.3) },
			}
			table.insert(
				out,
				kit.sloped(material, outline, -16, top - lean * x, lean, -top / (depth * 6))
			)
			x += width * 0.8
		end
		return out
	end,
}
