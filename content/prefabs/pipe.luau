-- A pipe between two points, relative to the origin, with collars. params: to
-- (x y z), radius, material, collar, every (how far apart collars are).
return {
	build = function(params)
		local to = params.to or { 256, 0, 0 }
		local radius = params.radius or 16
		local material = params.material or "metal"
		local collar = params.collar or material
		local every = params.every or 128
		local out = {}
		local length = math.sqrt(to[1] * to[1] + to[2] * to[2] + to[3] * to[3])
		table.insert(out, kit.beam(material, { 0, 0, 0 }, to, radius * 2))
		local count = math.floor(length / every)
		for index = 0, count do
			local share = index / math.max(count, 1)
			local width = 6 / length
			local from =
				{ to[1] * (share - width), to[2] * (share - width), to[3] * (share - width) }
			local till =
				{ to[1] * (share + width), to[2] * (share + width), to[3] * (share + width) }
			table.insert(out, kit.beam(collar, from, till, radius * 2.5))
		end
		return out
	end,
}
