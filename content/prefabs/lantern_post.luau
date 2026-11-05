-- A post with a lit lantern on an arm. params: height, reach, material, intensity,
-- color, style.
return {
	build = function(params)
		local height = params.height or 144
		local reach = params.reach or 28
		local material = params.material or "metal"
		local glow = params.glow or "light_panel"
		return {
			kit.prism(material, kit.ring(5, 6, 0), 0, height),
			kit.prism(material, kit.ring(10, 6, 0), 0, 8),
			kit.box(material, 0, -2, height - 4, reach, 2, height),
			kit.box(material, reach - 6, -6, height - 22, reach + 6, 6, height - 20),
			kit.box(glow, reach - 4, -4, height - 20, reach + 4, 4, height - 8),
			kit.box(material, reach - 6, -6, height - 8, reach + 6, 6, height - 4),
			{
				block = "entity",
				type = "light",
				origin = { reach, 0, height - 26 },
				intensity = params.intensity or 200,
				color = params.color or { 1, 0.78, 0.5 },
				style = params.style,
			},
		}
	end,
}
