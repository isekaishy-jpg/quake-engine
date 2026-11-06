-- A timber deck along x from the origin, with rails. params: length, width, deck,
-- rail, post (materials), height (of the rails), posts (how far apart), legs
-- (how far down supports go, or nothing to hang free), rails (false for none).
return {
	build = function(params)
		local length = params.length or 256
		local width = params.width or 96
		local deck = params.deck or "wood"
		local rail = params.rail or "metal"
		local post = params.post or rail
		local high = params.height or 40
		local spacing = params.posts or 96
		local legs = params.legs
		local out = { kit.box(deck, 0, 0, -8, length, width, 0) }
		-- Two beams run under the planks.
		table.insert(out, kit.box(post, 0, 8, -20, length, 20, -8))
		table.insert(out, kit.box(post, 0, width - 20, -20, length, width - 8, -8))
		local railed = params.rails ~= false
		for _, y in { 2, width - 2 } do
			if railed then
				table.insert(out, kit.beam(rail, { 0, y, high }, { length, y, high }, 4))
				table.insert(
					out,
					kit.beam(rail, { 0, y, high * 0.55 }, { length, y, high * 0.55 }, 3)
				)
			end
		end
		local count = math.max(1, math.floor(length / spacing))
		for index = 0, count do
			local x = length * index / count
			for _, y in { 2, width - 2 } do
				if railed then
					table.insert(out, kit.beam(post, { x, y, 0 }, { x, y, high + 2 }, 5))
				end
				if legs then
					table.insert(out, kit.beam(post, { x, y, -20 }, { x, y, -legs }, 8))
				end
			end
		end
		return out
	end,
}
