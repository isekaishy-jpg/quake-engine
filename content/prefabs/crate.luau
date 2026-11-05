-- A pushable-looking box of a given size. params: size, material.
return {
    build = function(params)
        local size = params.size or 48
        return {
            {
                block = "brush",
                material = params.material or "metal",
                cuboid = { 0, 0, 0, size, size, size },
            },
        }
    end,
}
