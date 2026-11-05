-- A wall of panels whose hotspots fit themselves. params: length, height.
return {
    build = function(params)
        local length = params.length or 384
        local height = params.height or 64
        return {
            {
                block = "brush",
                material = "panel_sheet",
                cuboid = { 0, 0, 0, length, 16, height },
            },
            {
                block = "brush",
                material = "panel_sheet",
                cuboid = { 0, 0, height, length / 3, 16, height + 128 },
            },
        }
    end,
}
