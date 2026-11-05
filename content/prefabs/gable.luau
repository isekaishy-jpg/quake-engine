-- A pitched roof along y, centred on the origin at its eaves. params: width,
-- length, rise, thickness, material.
return {
	build = function(params)
		local width = params.width or 256
		local length = params.length or 256
		local rise = params.rise or 96
		local thick = params.thickness or 12
		local material = params.material or "wood"
		local half = width / 2
		local y = length / 2
		return {
			kit.prism_y(
				material,
				{ { -half, 0 }, { -half, thick }, { 0, rise + thick }, { 0, rise } },
				-y,
				y
			),
			kit.prism_y(
				material,
				{ { 0, rise }, { 0, rise + thick }, { half, thick }, { half, 0 } },
				-y,
				y
			),
		}
	end,
}
