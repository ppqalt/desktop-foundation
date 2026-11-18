# Commands and entry points

Start here rather than adding another wrapper:

| Purpose | Entry point |
| --- | --- |
| Install or resume installation | `./scripts/install --standard` |
| Runtime commands and configuration | `./scripts/foundation --help` |
| Development workflows | `./scripts/dev --help` |
| Update an installed checkout | `./scripts/df-update` |
| Gallery | `./scripts/component-gallery` |

`foundation` is the single runtime launcher. It locates the owning checkout,
checks that its release backend exists, supplies `--root` and replaces itself
with the Rust executable. It never compiles or installs packages implicitly.

The short historical commands (`record`, `volume`, `clock`, `theme-apply`,
`monitor`, etc.) are **relative symbolic links to `foundation`**, not independent
implementations. Their argument mappings live in its one dispatch table. The
same applies to old developer aliases such as `test`, `format` and `lint`, which
forward to `dev`. Installed links through `~/.local/bin` retain the owning
checkout and original command name. Keep these links: keybindings, service
units, older deployments and external commands can still reference them.

Prefer `foundation COMMAND` in new runtime callers, or an existing alias when
the caller's contract requires it. Add domain logic to its existing Rust module,
not to a new shell launcher. Preserve argument arrays and native exit status.
`tests/test_script_dispatch.py` exercises all 48 links against a private fake
backend, including installed link chains, defaults and failure behavior.

Separate executable scripts remain where they own a real lifecycle: dependency
bootstrap, first-build installation, source updating, protected boot/recovery
writers, process supervision and source reload. These cannot all be folded into
a wrapper that requires the native backend to exist. Existing Python modules
also remain where they are imported by current compatibility paths/tests; see
[Python inventory](../docs/python-inventory.md).

Developer benchmarks, explicit live checks and gallery fixture data belong in
[`tools/`](../tools/README.md). Automated regressions belong in `tests/`.
