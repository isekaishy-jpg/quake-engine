-- A slope rising along +x. params: length, height, width, material.
return {
    build = function(params)
        local length = params.length or 256
        local height = params.height or 128
        local width = params.width or 128
        local material = params.material or "stone"
        local slope = math.sqrt(length * length + height * height)
        local nx, nz = -height / slope, length / slope
        return {
            {
                block = "brush",
                material = material,
                children = {
                    { block = "face", normal = { nx, 0, nz }, distance = 0 },
                    { block = "face", normal = { 1, 0, 0 }, distance = length },
                    { block = "face", normal = { 0, 0, -1 }, distance = 0 },
                    { block = "face", normal = { 0, 1, 0 }, distance = width },
                    { block = "face", normal = { 0, -1, 0 }, distance = 0 },
                },
            },
        }
    end,
}
