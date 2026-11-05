-- A decal on a wall. params: material, normal, size, angle.
return {
    build = function(params)
        return {
            {
                block = "decal",
                material = params.material or "lettering",
                origin = { 0, 0, 0 },
                normal = params.normal or { 0, 1, 0 },
                size = params.size or 96,
                angle = params.angle or 0,
            },
        }
    end,
}
