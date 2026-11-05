-- A barrel that's an entity's model, going back and forth like a door, with its
-- occlusion sidecar shading the floor as it moves. params: name, travel.
return {
    build = function(params)
        return {
            {
                block = "entity",
                name = params.name,
                type = "func_door",
                mesh = "barrel",
                origin = { 0, 0, 0 },
                angle = params.angle or 0,
                lip = -(params.travel or 96),
                wait = 2,
                speed = 48,
            },
        }
    end,
}
