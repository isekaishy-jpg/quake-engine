-- A flat wall stamped into bricks. params: normal, size, depth.
return {
	build = function(params)
		return {
			{
				block = "stamp",
				image = "stamp_bricks",
				origin = { 0, 0, 0 },
				normal = params.normal or { 0, 1, 0 },
				size = params.size or { 192, 160 },
				depth = params.depth or 6,
				detail = 4,
			},
		}
	end,
}
