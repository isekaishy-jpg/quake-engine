-- A wall torch: a bracket, a baked flickering light and a flame.
-- params: intensity, style, color.
return {
    build = function(params)
        return {
            { block = "brush", material = "metal", cuboid = { -4, -4, 0, 4, 4, 24 } },
            {
                block = "entity",
                type = "light",
                origin = { 0, 0, 40 },
                intensity = params.intensity or 220,
                color = params.color or { 1, 0.7, 0.4 },
                style = params.style or 1,
                size = 4,
            },
            {
                block = "entity",
                type = "flame",
                origin = { 0, 0, 36 },
            },
        }
    end,
}
