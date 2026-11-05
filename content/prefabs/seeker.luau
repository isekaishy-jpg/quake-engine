-- An NPC that finds its own way to a goal, as a prefab entity from the map's
-- script. params: goal, a point to go to.
return {
    build = function(params)
        return {
            {
                block = "entity",
                type = "seeker",
                origin = { 0, 0, 24 },
                goal = params.goal or { 0, 0, 24 },
            },
        }
    end,
}
