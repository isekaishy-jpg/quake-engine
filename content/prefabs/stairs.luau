-- Steps rising along +x. params: steps, step_rise, step_run, width, material.
return {
    build = function(params)
        local steps = params.steps or 8
        local rise = params.step_rise or 16
        local run = params.step_run or 32
        local width = params.width or 128
        local material = params.material or "stone"
        local out = {}
        for index = 0, steps - 1 do
            table.insert(out, {
                block = "brush",
                material = material,
                cuboid = { index * run, 0, 0, index * run + run, width, (index + 1) * rise },
            })
        end
        return out
    end,
}
