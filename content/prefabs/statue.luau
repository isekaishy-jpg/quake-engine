-- A statue, as a mesh in the world. params: scale, angle.
return {
	build = function(params)
		return {
			{
				block = "mesh",
				source = params.mesh or "statue",
				origin = { 0, 0, 0 },
				angles = { 0, params.angle or 0, 0 },
				scale = params.scale or 1,
			},
		}
	end,
}
