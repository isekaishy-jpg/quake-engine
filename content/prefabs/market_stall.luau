-- A stall with a sloped awning, facing -y. params: width, depth, height, counter,
-- post, cloth (materials).
return {
	build = function(params)
		local width = params.width or 128
		local depth = params.depth or 80
		local height = params.height or 104
		local post = params.post or "wood"
		local counter = params.counter or post
		local cloth = params.cloth or "metal"
		local out = {
			kit.box(counter, 8, 8, 0, width - 8, 28, 40),
			kit.box(post, 4, 4, 40, width - 4, 32, 44),
		}
		for _, x in { 6, width - 6 } do
			table.insert(out, kit.beam(post, { x, 6, 0 }, { x, 6, height - 16 }, 6))
			table.insert(out, kit.beam(post, { x, depth - 6, 0 }, { x, depth - 6, height + 12 }, 6))
		end
		table.insert(
			out,
			kit.prism_x(cloth, {
				{ -8, height - 22 },
				{ depth + 8, height + 16 },
				{ depth + 8, height + 22 },
				{ -8, height - 16 },
			}, -8, width + 8)
		)
		return out
	end,
}
