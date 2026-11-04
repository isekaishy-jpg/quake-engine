--  Copyright (C) 1996-1997  Id Software, Inc.
--
--  This program is free software; you can redistribute it and/or modify
--  it under the terms of the GNU General Public License as published by
--  the Free Software Foundation; either version 2 of the License, or
--  (at your option) any later version.
--
--  This program is distributed in the hope that it will be useful,
--  but WITHOUT ANY WARRANTY; without even the implied warranty of
--  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
--  GNU General Public License for more details.
--
--  See the LICENSE file for details.
--
-- Ported from the level changing part of client.qc. The intermission waits for
-- a HUD.

function changelevel_touch(self, other)
	if other.classname ~= "player" then
		return
	end
	activator = other
	SUB_UseTargets(self)
	self.touch = SUB_Null
	engine.changelevel(self.map)
end

function trigger_changelevel(self)
	if not has_text(self.map) then
		objerror("changelevel trigger doesn't have map")
	end
	InitTrigger(self)
	self.touch = changelevel_touch
end
