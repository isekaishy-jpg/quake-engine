-- A round column with a base and a head. params: radius, height, sides, material,
-- trim (for its base and head).
return {
	build = function(params)
		local radius = params.radius or 24
		local height = params.height or 256
		local sides = params.sides or 8
		local stone = params.material or "stone"
		local trim = params.trim or stone
		local wide = kit.ring(radius * 1.45, sides, math.pi / sides)
		local mid = kit.ring(radius * 1.2, sides, math.pi / sides)
		local body = kit.ring(radius, sides, math.pi / sides)
		local foot = math.min(16, height / 8)
		return {
			kit.prism(trim, wide, 0, foot * 0.6),
			kit.prism(trim, mid, foot * 0.6, foot),
			kit.prism(stone, body, foot, height - foot),
			kit.prism(trim, mid, height - foot, height - foot * 0.6),
			kit.prism(trim, wide, height - foot * 0.6, height),
		}
	end,
}
