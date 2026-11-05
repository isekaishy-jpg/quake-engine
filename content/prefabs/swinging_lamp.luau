-- A baked light moving through keyframes, blended by time. params: length, period.
return {
    build = function(params)
        local length = params.length or 96
        return {
            {
                block = "entity",
                type = "light",
                intensity = params.intensity or 320,
                color = params.color or { 0.7, 0.8, 1 },
                period = params.period or 4,
                origin = { 0, 0, 0 },
                keyframes = { -length, 0, 0, 0, 0, -16, length, 0, 0, 0, 0, -16 },
            },
        }
    end,
}
