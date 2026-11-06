-- A square masonry tower from its middle, with a walkable top and battlement.
-- params: width, height, material, trim, cap, windows.
return {
	build = function(params)
		local width = params.width or 192
		local height = params.height or 320
		local stone = params.material or "stone"
		local trim = params.trim or stone
		local cap = params.cap or trim
		local half = width / 2
		local out = {
			kit.box(stone, -half, -half, 0, half, half, height - 16),
			kit.box(trim, -half - 12, -half - 12, height - 16, half + 12, half + 12, height),
		}
		-- A band every so often, and dark slits between them.
		local band = 128
		local z = band
		while z < height - 48 do
			table.insert(out, kit.box(trim, -half - 4, -half - 4, z, half + 4, half + 4, z + 8))
			for _, side in { -1, 1 } do
				table.insert(
					out,
					kit.box(cap, side * half - 2, -12, z + 32, side * half + 2, 12, z + 88)
				)
				table.insert(
					out,
					kit.box(cap, -12, side * half - 2, z + 32, 12, side * half + 2, z + 88)
				)
			end
			z += band
		end
		return out
	end,
}
