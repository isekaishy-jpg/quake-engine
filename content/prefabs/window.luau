-- A wall along y with a stained glass window. params: width, height, sill, top, thickness, material.
return {
    build = function(params)
        local width = params.width or 512
        local height = params.height or 320
        local sill = params.sill or 64
        local top = params.top or 256
        local opening = params.opening or 192
        local thick = params.thickness or 32
        local material = params.material or "brick"
        local half = width / 2
        local gap = opening / 2
        return {
            { block = "brush", material = material, cuboid = { 0, -half, 0, thick, -gap, height } },
            { block = "brush", material = material, cuboid = { 0, gap, 0, thick, half, height } },
            { block = "brush", material = material, cuboid = { 0, -gap, 0, thick, gap, sill } },
            { block = "brush", material = material, cuboid = { 0, -gap, top, thick, gap, height } },
            { block = "brush", material = "stained_glass", cuboid = { 12, -gap, sill, 20, gap, top } },
        }
    end,
}
