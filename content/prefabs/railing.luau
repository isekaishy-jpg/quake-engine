-- Posts and a rail along +x. params: length, height, post_gap, material.
return {
	build = function(params)
		local length = params.length or 256
		local height = params.height or 48
		local gap = params.post_gap or 64
		local material = params.material or "metal"
		local out = {
			{
				block = "brush",
				material = material,
				cuboid = { 0, -2, height - 4, length, 2, height },
			},
		}
		local x = 0
		while x <= length do
			table.insert(out, {
				block = "brush",
				material = material,
				cuboid = { x - 2, -2, 0, x + 2, 2, height - 4 },
			})
			x = x + gap
		end
		return out
	end,
}
