-- A heap of fallen blocks. params: radius, count, size, material, seed.
return {
	build = function(params)
		local radius = params.radius or 96
		local count = params.count or 10
		local size = params.size or 40
		local material = params.material or "stone"
		local random = kit.random(params.seed or 1)
		local out = {}
		for _ = 1, count do
			local angle = random() * math.pi * 2
			local reach = math.sqrt(random()) * radius
			local x, y = math.cos(angle) * reach, math.sin(angle) * reach
			local across = size * (0.45 + random() * 0.7)
			local high = (1 - reach / radius) * size * 0.9
			local turn = random() * math.pi
			local tilt_x, tilt_y = (random() - 0.5) * 0.7, (random() - 0.5) * 0.7
			local outline = kit.ring(across, 4 + math.floor(random() * 3), turn)
			for _, point in outline do
				point[1] += x
				point[2] += y
			end
			local top = high + across * (0.5 + random() * 0.6)
			table.insert(
				out,
				kit.sloped(material, outline, -8, top - tilt_x * x - tilt_y * y, tilt_x, tilt_y)
			)
		end
		return out
	end,
}
