-- A monster, ready to fight. params: kind, angle.
return {
    build = function(params)
        return {
            {
                block = "entity",
                type = params.kind or "monster_army",
                origin = { 0, 0, 24 },
                angle = params.angle or 0,
            },
        }
    end,
}
