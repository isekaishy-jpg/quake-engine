-- A thing that says when it's touched, as a prefab entity from the map's script.
-- params: message.
return {
	build = function(params)
		return {
			{
				block = "entity",
				type = "tell_tale",
				origin = { 0, 0, 16 },
				message = params.message or "you found it",
			},
		}
	end,
}
