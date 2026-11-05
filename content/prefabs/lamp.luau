-- A hanging lamp. params: intensity, color, drop.
return {
    build = function(params)
        local drop = params.drop or 32
        return {
            { block = "brush", material = "metal", cuboid = { -1, -1, -drop, 1, 1, 0 } },
            { block = "brush", material = "light_panel", cuboid = { -8, -8, -drop - 6, 8, 8, -drop } },
            {
                block = "entity",
                type = "light",
                origin = { 0, 0, -drop - 16 },
                intensity = params.intensity or 320,
                color = params.color or { 1, 0.95, 0.85 },
            },
        }
    end,
}
