-- An item to pick up. params: kind.
return {
    build = function(params)
        return {
            {
                block = "entity",
                type = params.kind or "item_health",
                origin = { 0, 0, 8 },
            },
        }
    end,
}
