-- A guard walking a spline, as a prefab entity from the map's script. params:
-- name, size.
return {
    build = function(params)
        local name = params.name or "patrol_route"
        local size = params.size or 128
        return {
            {
                block = "spline",
                name = name,
                closed = true,
                points = {
                    0, 0, 24,
                    size, 0, 24,
                    size, size, 24,
                    0, size, 24,
                },
            },
            {
                block = "entity",
                type = "patrol_walker",
                origin = { 0, 0, 24 },
                route = name,
            },
        }
    end,
}
