-- A wall along x from the origin, its top broken away. params: length, height,
-- thickness, material, seed, ragged (how far down the breaks go), holes (window
-- openings along it, as many as fit).
return {
	build = function(params)
		local length = params.length or 256
		local height = params.height or 192
		local thick = params.thickness or 24
		local material = params.material or "stone"
		local ragged = params.ragged or height * 0.5
		local holes = params.holes or 0
		local random = kit.random(params.seed or 1)
		local out = {}
		local spans = math.max(2, math.floor(length / 48))
		local step = length / spans
		local top = height - random() * ragged * 0.5
		for index = 0, spans - 1 do
			local x0, x1 = index * step, (index + 1) * step
			local next_top = height - random() * ragged
			if index == spans - 1 and params.falls then
				next_top = height * 0.2
			end
			local slope = (next_top - top) / step
			local opening = holes > 0
				and index % math.max(2, math.floor(spans / holes)) == 1
				and math.min(top, next_top) > 128
			if opening then
				local sill, lintel = 56, math.min(128, math.min(top, next_top) - 16)
				table.insert(out, kit.box(material, x0, 0, 0, x1, thick, sill))
				table.insert(
					out,
					kit.sloped(
						material,
						{ { x0, 0 }, { x1, 0 }, { x1, thick }, { x0, thick } },
						lintel,
						top - slope * x0,
						slope,
						0
					)
				)
			else
				table.insert(
					out,
					kit.sloped(
						material,
						{ { x0, 0 }, { x1, 0 }, { x1, thick }, { x0, thick } },
						0,
						top - slope * x0,
						slope,
						0
					)
				)
			end
			top = next_top
		end
		return out
	end,
}
