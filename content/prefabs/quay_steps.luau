-- Steps down from a quay to the water, along x, descending towards +y. params:
-- width, steps, material.
return {
	build = function(params)
		local width = params.width or 128
		local steps = params.steps or 5
		local material = params.material or "stone"
		local out = {}
		for index = 0, steps - 1 do
			table.insert(
				out,
				kit.box(material, 0, index * 24, -160, width, index * 24 + 24, -index * 16)
			)
		end
		return out
	end,
}
