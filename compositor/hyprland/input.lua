-- Shared user input, translated only here into Hyprland options.
local input = dofile(FOUNDATION_ROOT .. "/config/input.lua")
hl.config({ input = { kb_layout = input.layout, repeat_rate = input.repeatRate, repeat_delay = input.repeatDelay, resolve_binds_by_sym = true } })
