-- A slab whose surface flows. params: size.
return {
    build = function(params)
        local size = params.size or 192
        return {
            { block = "brush", material = "swirl", cuboid = { 0, 0, 0, size, size, 12 } },
        }
    end,
}
