"""Translate shared data to Niri-native KDL; no runtime visual emulation."""
import json
from pathlib import Path
import subprocess


def data(path):
    # Pure Lua data is shared with the retained Hyprland implementation.
    code = '''local function encode(v)
      if type(v)=="table" then local parts={} for k,x in pairs(v) do
        parts[#parts+1]=string.format("%q",k)..":"..encode(x) end
        return "{"..table.concat(parts,",").."}"
      elseif type(v)=="string" then return string.format("%q",v)
      else return tostring(v) end end
      print(encode(dofile(arg[1])))'''
    return json.loads(subprocess.check_output(['lua', '-e', code, '--', '/dev/null', str(path)], text=True))


def render(root, profile):
    root = Path(root)
    v = data(root / 'config/window-appearance.lua')
    keyboard = data(root / 'config/input.lua')
    color = lambda opacity: v['shadow']['color'] + f'{round(opacity * 255):02x}'
    q = json.dumps
    text = f'''// Generated from repository-managed shared visual/input intent.
include {q(str(root / 'compositor/niri/config.kdl'))}
environment {{
    DF_COMPOSITOR "niri"
    DF_FOUNDATION_ROOT {q(str(root))}
}}
spawn-at-startup {q(str(root / 'scripts/session-start'))}
input {{ keyboard {{ xkb {{ layout {q(keyboard['layout'])}; }} repeat-delay {keyboard['repeatDelay']}; repeat-rate {keyboard['repeatRate']}; }} }}
screenshot-path null
layout {{
    gaps {v['spacing']['betweenWindows']}
    struts {{ left {v['spacing']['desktopEdge']}; right {v['spacing']['desktopEdge']}; top {v['spacing']['desktopEdge']}; bottom {v['spacing']['desktopEdge']}; }}
    center-focused-column "never"
    preset-column-widths {{ proportion 0.33333; proportion 0.5; proportion 0.66667; }}
    default-column-width {{ proportion 0.5; }}
    focus-ring {{ off; }}
    border {{ on; width {v['focus']['borderWidth']}; active-color {q(v['focus']['activeBorder'])}; inactive-color {q(v['focus']['inactiveBorder'])}; }}
    shadow {{ on; softness {v['shadow']['range']}; spread 1; offset x={v['shadow']['offset']['x']} y={v['shadow']['offset']['y']}; color {q(color(v['shadow']['activeOpacity']))}; inactive-color {q(color(v['shadow']['inactiveOpacity']))}; }}
}}
blur {{ passes {v['blur']['qualityPasses']}; offset {max(1, v['blur']['radius'] * v['blur']['strength'] / 3)}; noise {v['blur']['noise']}; saturation {1 + v['blur']['saturationBoost']}; }}
window-rule {{
    geometry-corner-radius {v['rounding']}
    clip-to-geometry true
    draw-border-with-background false
    opacity {v['opacity']['active']}
    background-effect {{ blur true; xray true; }}
    popups {{ background-effect {{ blur false; }}; }}
}}
// Niri applies opacity to toplevel content; client popups keep their own shape.
// Keep terminal glyphs opaque; prefer native background alpha in Kitty.
window-rule {{ match app-id="^kitty$"; opacity 1.0; }}
layer-rule {{ match namespace="^desktop-foundation-(launcher|clipboard|power|bluetooth)$"; background-effect {{ blur true; xray true; }} }}
animations {{
    workspace-switch {{ duration-ms 300; curve "ease-out-cubic"; }}
    horizontal-view-movement {{ duration-ms 300; curve "ease-out-cubic"; }}
    window-movement {{ duration-ms 300; curve "ease-out-cubic"; }}
    window-open {{ duration-ms 160; curve "ease-out-cubic"; }}
    window-close {{ duration-ms 120; curve "ease-out-cubic"; }}
    overview-open-close {{ duration-ms 300; curve "ease-out-cubic"; }}
}}
'''
    host = root / 'profiles' / profile / 'niri.kdl'
    if host.exists():
        text += 'include ' + q(str(host)) + '\n'
    return text.replace("} ", "}; ")
