-- A wall button. params: target, name, material.
return {
    build = function(params)
        return {
            {
                block = "entity",
                name = params.name,
                type = "func_button",
                target = params.target,
                angle = -2,
                speed = 40,
                wait = 1,
                children = {
                    {
                        block = "brush",
                        material = params.material or "metal",
                        cuboid = { 0, -12, 0, 6, 12, 24 },
                    },
                },
            },
        }
    end,
}
