# Overview-only Ctrl+wheel

`overview-ctrl-scroll.patch` adds an opt-in setting to Niri's native overview
wheel handler. It was prepared against upstream commit `8ed0da4`, the source
revision reported by the installed Niri 26.04 executable. It has not been built,
applied to an installed package, or tested.

The patch adds this compositor configuration:

```kdl
overview {
    ctrl-scroll-workspaces true
}
```

With overview open, vertical wheel events with exactly Ctrl use Niri's existing
`FocusWorkspaceUp` and `FocusWorkspaceDown` actions on the focused output. They
retain the native workspace animations and leave overview open. Ctrl scrolling
has no additional cooldown and uses Niri's wheel-detent accumulator. Ordinary
wheel, horizontal wheel, touchpad gestures and other modifier combinations keep
their existing behavior. The existing exclusion for layer surfaces is preserved.

Outside overview the new branch is unreachable. There are no global bindings,
input grabs, replayed events, IPC listeners or background processes.

## Integration

Apply the patch to the Niri source used by the compositor package, retaining any
existing distribution patches. Review overlapping hunks when updating Niri.
Build, configuration validation and behavioral acceptance are still required
before replacing the installed compositor. Installing a changed executable does
not update an already running compositor; the new binary needs a later session.
No package or session changes are made by Desktop Foundation for this patch.

Once the patched compositor is installed and running, enable it through the
normal layered Foundation input settings:

```toml
# $XDG_CONFIG_HOME/desktop-foundation/input.toml
overview_ctrl_scroll = true
```

The default is false so stock Niri keeps accepting generated configuration.
When true, Foundation emits the scoped `overview` option above. Stock Niri
rejects that unknown option during the existing deployment validation, rather
than substituting a global Ctrl+wheel binding. Profile input settings and sorted
`input.d` fragments support the same field; `config input` includes its effective
value and the existing source-file list.

## Deferred acceptance

No tests or builds were run for this change. Before activation, verify source
compatibility and configuration reload, up/down wheel detents, rapid repeated
scrolling, workspace limits and empty workspaces, focused-output behavior on
multiple monitors, and unchanged browser Ctrl+wheel outside overview. Touchpad
Ctrl scrolling is deliberately unchanged; the requested navigation is for mouse
wheel events. Ordinary touchpad overview gestures remain native.

## Source and license

The patch modifies [Niri's native input handler](https://github.com/niri-wm/niri/blob/8ed0da4/src/input/mod.rs)
and its configuration parser. Niri is licensed under GPL-3.0-or-later; upstream
copyright and license notices apply to the patched source.
