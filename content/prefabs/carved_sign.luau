-- Letters cut into a wall, with metal showing inside. params: image, inner,
-- normal, size, depth.
return {
	build = function(params)
		return {
			{
				block = "stamp",
				image = params.image or "stamp_letters",
				inner = params.inner or "metal",
				origin = { 0, 0, 0 },
				normal = params.normal or { 0, 1, 0 },
				size = params.size or 180,
				depth = params.depth or 12,
				detail = params.detail or 3,
			},
		}
	end,
}
