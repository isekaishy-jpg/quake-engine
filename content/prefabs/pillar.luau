-- A square pillar. params: size, height, material.
return {
    build = function(params)
        local size = (params.size or 64) / 2
        local height = params.height or 256
        return {
            {
                block = "brush",
                material = params.material or "stone",
                cuboid = { -size, -size, 0, size, size, height },
            },
        }
    end,
}
