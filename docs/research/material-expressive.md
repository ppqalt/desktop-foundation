# Control Center: Material 3 Expressive research

Research date: 29 October 2026. Design study, not an implementation specification or a list of shipped features.

Recommended direction: **a quiet Material 3 Expressive control surface, with wallpaper-derived tonal containers, generous controls and a compact overview leading into detailed views.** Keep Desktop Foundation’s graphite base, Rust behavior and QML presentation.

The [visual reference board](material-expressive-references.html) contains original, attributed project images. The catalog below distinguishes visual inspection from documentation review. Reference screenshots describe a particular published design, not necessarily the latest running version.

## What the current panel needs

The supplied screenshot exposes the backend’s structure too directly. Audio devices occupy long technical dropdowns; the same wired connection appears as both a device and a saved profile; Bluetooth power, a paired device and a link to another popup compete at similar emphasis. Almost every action looks like another outlined strip. Thin sliders and small secondary labels make important controls harder to find than their surrounding hardware names.

The wallpaper is also doing too much visual work behind the text. Blur softens detail, but does not produce a predictable surface luminance. A pink active fill, pink outline and keyboard focus need distinct meanings.

These are information hierarchy and component problems. Changing only corner radii, fonts or hue would leave them largely intact.

## What Google actually specifies

The following are Google’s published guidance and measurements. Proposed Desktop Foundation choices appear separately later. Android dp/sp, CSS units and Qt logical pixels are different systems; numeric token values need an explicit platform mapping.

### Purpose and expressive restraint

Google’s [introductory guidance](https://m3.material.io/blog/building-with-m3-expressive) describes seven interacting tactics: shape variation, nuanced color, typographic hierarchy, meaningful containment, natural motion, adaptable components and selective combinations that emphasize key moments. Its particularly relevant instruction is **“Contain content for emphasis.”** It recommends one or two hero moments rather than making everything compete.

This permits a calm desktop. Material’s creative director explicitly says **“Expressive can mean quiet.”** [Google Design interview](https://design.google/library/design-notes-material-3-expressive-liam-spradlin).

The [Google research account](https://design.google/library/expressive-material-design-google-research) covers 46 studies and more than 18,000 participants. It also reports failures when familiar structures or useful labels were removed. That supports clearer grouping and stronger key controls; it does not justify unlabeled decorative buttons everywhere. Reported improvements belong to those studies, not a performance prediction for this panel.

### Shapes: verified scale and nesting

Directly checked on Google’s rendered [corner-radius specification](https://m3.material.io/styles/shape/corner-radius-scale):

| Token suffix | Radius |
| --- | ---: |
| none | 0 dp |
| extra-small | 4 dp |
| small | 8 dp |
| medium | 12 dp |
| large | 16 dp |
| large-increased | 20 dp |
| extra-large | 28 dp |
| extra-large-increased | 32 dp |
| extra-extra-large | 48 dp |
| full | Fully rounded |

Google permits asymmetry and component-level customization. It recommends optical nesting: outer radius minus padding equals inner radius. Large rounding can consume useful space in dense containers; unusual shapes are better reserved for limited visual emphasis. [Shape principles](https://m3.material.io/styles/shape/overview-principles).

Our existing 14-pixel sidebar corner is therefore a product customization, not one of Google’s stock scale values. It can remain while internal controls use a coherent scale. The nesting formula is a visual guideline, not a reason to force a 2-pixel radius onto every independently spaced card inside the current frame.

### Buttons: shape conveys interaction

Google’s current [button specifications](https://m3.material.io/components/buttons/specs) include default/action and toggle/selection variants, five sizes, round or square shapes, and elevated, filled, tonal, outlined or text treatments. Toggles do not have a text-only style.

| Button size | Square resting radius | Pressed radius |
| --- | ---: | ---: |
| XS | 12 dp | 8 dp |
| S | 12 dp | 8 dp |
| M | 16 dp | 12 dp |
| L | 28 dp | 16 dp |
| XL | 28 dp | 16 dp |

Round resting buttons use full rounding. Toggle selection reverses the resting shape family: round to square, or square to round. This provides a second state cue alongside color. An ordinary action button and a persistent radio toggle should consequently not share identical semantics just because both are clickable.

### Sliders: substantial tracks, precise values

Directly checked on [Google’s slider specs](https://m3.material.io/components/sliders/specs):

| Size | Track height | Handle height | Track radius |
| --- | ---: | ---: | ---: |
| XS | 16 dp | 44 dp | 8 dp |
| S | 24 dp | 44 dp | 8 dp |
| M | 40 dp | 52 dp | 12 dp |
| L | 56 dp | 68 dp | 16 dp |
| XL | 96 dp | 108 dp | 28 dp |

Handle width is 4 dp. Standard, centered and range variants exist; orientation, stops and value indicators are configurations. Inset icons apply to suitable larger sizes. Availability differs between platform implementations; these specifications do not mean Qt ships an equivalent component.

For this panel, an S or M treatment is a useful starting point for mockups. Volume remains continuous and immediate. Expressive styling does not require animating the value or making the bar trail keyboard input.

### Color: roles before colors

Google’s [color-role guidance](https://m3.material.io/styles/color/the-color-system) separates surfaces from accent containers and their paired foregrounds. Five surface-container emphasis levels are available: lowest, low, default, high and highest. `onSurface` and `onSurfaceVariant` provide text hierarchy. Accent fills use their matching `on…` roles; `outlineVariant` suits separators, whereas `outline` identifies important boundaries.

For Desktop Foundation, map these relationships through the existing semantic palette. Preserve neutral graphite at the bottom of the hierarchy, allow visibly wallpaper-tinted containers above it, and give selected controls a stronger accent. Do not independently tint every QML file or treat alpha blending as a substitute for tonal roles. Paired colors still require contrast checks after our graphite mapping and compositing.

### Typography

Google’s [current type-scale guidance](https://m3.material.io/styles/typography/type-scale-tokens) defines 15 baseline and 15 emphasized styles. They are intended to coexist; a product should select a useful subset. Emphasis can identify a selected item or important action. Brand and reading typefaces can differ, and the defaults are customizable.

Its [application guidance](https://m3.material.io/styles/typography/applying-type) recommends tabular digits for frequently changing values. Use readable proportional text for device names, compact supporting text for state, and aligned digits for percentages. Monospace and all-capital section labels should not carry the entire hierarchy.

### Motion: current physics system, not guessed easing

Google’s [motion overview](https://m3.material.io/styles/motion/overview/how-it-works) distinguishes spatial springs for geometry from effects springs for color/opacity. Springs preserve continuity through interruptions and retargeting; effects should not overshoot. Expressive and standard schemes allow different levels of bounce.

The rendered [motion specification](https://m3.material.io/styles/motion/overview/specs) gives these **web fallback approximations**, not mandatory durations for every platform:

| Expressive token | Cubic Bézier | Duration |
| --- | --- | ---: |
| fast spatial | .42, 1.67, .21, .90 | 350 ms |
| default spatial | .38, 1.21, .22, 1.00 | 500 ms |
| slow spatial | .39, 1.29, .35, .98 | 650 ms |
| fast effects | .31, .94, .34, 1.00 | 150 ms |
| default effects | .34, .80, .34, 1.00 | 200 ms |
| slow effects | .34, .88, .34, 1.00 | 300 ms |

Google prefers actual springs when possible, especially for interruptible interactions. Our existing velocity-preserving 240/180 ms sidebar motion is a deliberate local design, not an implementation of those tokens. Keep it for the first visual redesign so the two sidebars stay consistent; evaluate a shared motion change separately. Preserve the previously requested 100 ms volume-OSD fades and immediate volume values.

### Lists, targets and keyboard access

Google’s [lists overview](https://m3.material.io/components/lists/overview) recommends concise, consistently structured rows and distinguishes standard from segmented lists. Its updated expressive list supports clearer grouping. Long technical descriptions belong in a secondary line or detailed view.

Google’s [Android accessibility guidance](https://support.google.com/accessibility/android/answer/7101858?hl=en) recommends 48×48 dp touch targets, with 8 dp separation where appropriate. A visual icon can be smaller than its input area. This is a useful accessible starting point, not a claim that every desktop widget must be 48 physical pixels wide.

Use at least 4.5:1 for ordinary text and 3:1 for qualifying large text, following [W3C’s contrast guidance](https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html). A generic 3:1 component-color statement is not sufficient for small labels. Check the actual composited surface, including a bright wallpaper. Keyboard focus must be visible independently of selection, and essential actions must not require hover, a long press or dragging.

## Curated reference catalog

These are sources to learn from, not proposed replacement dependencies. Performance cannot be inferred from screenshots. “Visual” below means a published image or demo was actually inspected; “docs” means the source was reviewed without a hands-on UI assessment.

### Closest shell references

| Reference | Evidence reviewed | Useful lesson for our design | Boundary |
| --- | --- | --- | --- |
| [Caelestia](https://github.com/caelestia-dots/shell) | README/configuration; published video sampled visually | Cohesive shell geometry and morphing transitions; shared primitives instead of individually styled popups | Hyprland-oriented Quickshell shell. A sampled demo is not an animation benchmark or Niri compatibility test. |
| [illogical-impulse / end-4](https://github.com/end-4/dots-hyprland) | Current README and two linked desktop images, visual | Strong wallpaper palette identity and distinct functional surfaces | The linked images include older Material/Windows-inspired work. Do not present these as an exact picture of current Expressive components. |
| [iNiR](https://github.com/snowarch/iNiR) | Current README and iRiS card screenshot, visual; [project map](https://inir.space/docs/project-map/) | Niri-specific shell integration and consistent related surfaces | Current README is iRiS-focused; older indexed “Material ii/Waffle” descriptions differ. A relevant architectural comparison, not a template to copy wholesale. |
| [Dux Shell, formerly the Dank Material Shell repository](https://github.com/duxlabs-dev/dux-shell) | Redirect checked; README; published Control Center screenshot, visual | Two-column control modules, strong icon areas, compact audio controls; UI/backend separation | Quickshell + Go. The inspected screenshot is in edit mode: remove handles and editing chrome are not part of a clean normal-use design. Keep our Rust backend. |
| [NAnDoroid](https://github.com/na-ive/nandoroid-shell) | README and showcase 1/2, visual | Closest direct Android 16-inspired example: substantial sliders, shaped controls, rich wallpaper tones, visually contained groups | Quickshell + Hyprland. Its notch, desktop widgets and large feature set are outside this barless control-center redesign. |
| [Noctalia](https://github.com/noctalia-dev/noctalia) | Current README, [Control Center docs](https://docs.noctalia.dev/noctalia/control-center/), official community desktop image, visual | Home-versus-detailed-view separation; audio/network/Bluetooth are coherent destinations; keyboard navigation is documented | Current project is native Wayland/OpenGL ES, not Quickshell. The older v4 Quickshell shell is archived. Community desktop image alone does not prove current control-center appearance. |
| [Wayle](https://github.com/wayle-rs/wayle) | README and published wide desktop preview, visual | Compact device panels and consistent state presentation; useful Rust system-integration reference | Rust + GTK4/Relm4. Keep QML visuals here; avoid importing another UI toolkit or its always-running shell. |

### Additional custom-desktop references

| Reference | Evidence reviewed | What is worth borrowing | What is less suitable |
| --- | --- | --- | --- |
| [HyprPanel](https://github.com/Jas-SinghFSU/HyprPanel) | README and hp1 screenshot, visual | Clear grouping, separated utilities, consistent icon treatment | README says development moved to Wayle. Historical design reference; its dashboard density and permanent bar do not suit our overview. |
| [Ax-Shell](https://github.com/Axenide/Ax-Shell) | README and screenshots 1/2/4, visual | Rounded detached surfaces and coherent edge treatment | Deprecated in favor of [Ambxst](https://axeni.de/ambxst/). The inspected screenshots mostly show desktop/notch/launcher, not evidence for a detailed audio-control design. |
| [Ambxst](https://axeni.de/ambxst/) | Official project/install page, docs only | Successor worth evaluating for spatial transitions before an animation-focused pass | Not visually ranked here; no performance or Niri claim inferred from the landing page. |
| [adi1090x Eww widgets](https://github.com/adi1090x/widgets) | README and dashboard preview, visual | Strong grid rhythm, different module sizes, comfortable separation; useful custom-WM dashboard reference | Much more decorative and information-heavy than this panel needs; not a Material 3 Expressive implementation. |
| [Yoru](https://github.com/raexera/yoru) | Repository/README, docs only | Historical AwesomeWM composition reference for an organized custom desktop | Archived since October 2024; not a new runtime foundation. AwesomeWM is not i3 or Niri. |
| [SwayNotificationCenter](https://github.com/ErikReider/SwayNotificationCenter) | Repository/README, docs only | Dedicated notification/task surface rather than mixing everything into the control overview | GTK notification daemon, not an Expressive design authority or a replacement for our existing notification system. |
| [HyDE](https://github.com/HyDE-Project/HyDE) | Repository/README, docs only | Broad system-wide theme consistency and coordinated configuration | A full Hyprland environment; not evidence that another bar or theme stack is needed. |

### Operating-system references

| Reference | Primary source | Specific useful pattern | Desktop adaptation |
| --- | --- | --- | --- |
| Google Pixel / Android Expressive | [Official launch and examples](https://blog.google/products-and-platforms/platforms/android/material-3-expressive-android-wearos-launch/), visual | Responsive controls, stronger hierarchy, personalized colors, contextual depth | Primary visual language. Translate touch interactions into explicit mouse and keyboard actions. Watch-specific edge-hugging layouts are not desktop rules. |
| macOS Control Center | [Apple guide](https://support.apple.com/en-gb/guide/mac-help/mchlc9d0e1f2/27/mac/27), official image inspected | Modules of different sizes; essential status first; arrows reveal detail | Borrow grouping and disclosure. Refractive glass and Apple-specific control semantics would dilute the chosen Material identity. |
| iPhone Control Center | [Apple guide](https://support.apple.com/en-gb/guide/iphone/iph59095ec58/27/ios/27), docs | Related connectivity controls stay together; expanded controls expose more options | Keep labels and keyboard-accessible detail buttons. Do not copy hidden long-press requirements or iOS’s distinction between temporary Bluetooth disconnection and radio power. |
| GNOME Quick Settings | [GNOME 43 design introduction](https://release.gnome.org/43/), docs/video source identified | Common settings and audio-device switching can coexist without showing all devices permanently | Excellent information-architecture reference. The cited release is historical, not a claim about the exact current GNOME layout. |
| Windows 11 Quick Settings | [Microsoft guide](https://support.microsoft.com/en-us/windows/experience/personalization/customize-the-taskbar-in-windows), docs | Frequent controls gathered in a single keyboard-accessible destination | Borrow the concise overview, not the taskbar or a separate Windows visual system. |
| Samsung One UI Quick Panel | [Samsung structure guide](https://developer.samsung.com/one-ui/structure/basic.html) and [current support guide](https://www.samsung.com/us/support/answer/ANS10002874/), docs | Distinct clusters and clear separation between settings and notifications | Preserve compact desktop interaction; avoid requiring phone-style swipes or filling space with unavailable controls. |
| KDE Plasma | [Official Plasma overview](https://kde.org/plasma-desktop/), docs | Related device controls share a consistent system interface; inactive items can stay out of the way | Use one design system across our existing shortcuts. Do not add Plasma modules as dependencies. |
| COSMIC | [System76 overview](https://system76.com/cosmic), docs | Modular applets and consistent theming demonstrate that a Rust-centered desktop need not expose raw backend structure | Architectural inspiration; not a reason to rewrite QML or replace Niri. |

## Recommended Desktop Foundation design

This section is a proposal derived from the research, not a Google specification.

### 1. One glanceable overview

Keep the left-attached, full-height 720-logical-pixel frame. Give the body a simple two-column rhythm where space permits, rather than using every control as a full-width strip. Do not fill the available height just because it exists.

Suggested top-to-bottom organization:

1. Quiet header: “Control center”, close control and a short keyboard hint.
2. Connectivity pair: Network and Bluetooth tiles, each showing one useful current status and a separate, obvious detail affordance.
3. A larger Audio group: current output name, mute action, substantial volume slider and percentage; microphone alongside or immediately below using the same grammar.
4. Contextual connected-device summary, such as Nothing Ear (3), only when relevant. Opening its details reuses the existing controls.
5. A restrained footer for relevant settings entry points and errors. No duplicate list of all saved profiles on the overview.

On this machine, the network tile could read “Ethernet · Connected.” The interface identifier, IP details and alternative saved profiles belong in Network detail. A missing Wi-Fi adapter should not occupy a full disabled tile on the everyday overview, but should remain explainable in that detail view. Friendly names must distinguish digital and analog outputs, with the original technical name available when needed.

### 2. Detail views within the same frame

Use a consistent header/back pattern for Audio devices, Bluetooth and Network. Keep navigation inside the sidebar rather than stacking several competing floating menus. On return, restore the initiating control’s keyboard focus and preserve scroll position.

Device/profile identity must remain stable in Rust data; display ordering must not change an item’s action target. Show one connected wired profile rather than two representations of the same connection. Keep advanced configuration in the existing tools until a deliberate feature change is approved.

### 3. Color that visibly follows the wallpaper

Maintain graphite for the outer surface. Use increasingly tinted, more opaque internal containers to establish hierarchy, then reserve stronger wallpaper color for active controls and the slider’s active track. Keep text close to neutral and shadows effectively black.

The existing 75% outer background can remain while nested content gets additional tonal fill. That is different from raising opacity on the entire layer, which would also affect text. The exact fill/alpha values should be chosen in the mockup against both a busy bright wallpaper and a dark one, rather than guessed from a single screenshot.

Add missing surface/paired-text roles to the shared semantic palette only when implementing. Reuse the same roles in the launcher, clipboard, Bluetooth/Nothing controls and session menu so this does not become another isolated theme.

### 4. A small component vocabulary

Build reusable QML primitives for a tonal group, connectivity tile, device row, volume control, back/header and focus treatment. Existing Rust actions remain behind them. Use a restrained 12/16/20/28 radius family internally; avoid putting a border around every group and every child.

Differentiate these states deliberately:

| State | Proposed visual meaning |
| --- | --- |
| Radio on | Filled toggle and explicit on-state |
| Device connected | Connection label and device symbol |
| Current audio output | Selected indicator/checkmark |
| Keyboard focus | Distinct focus outline, independent of fill |
| Request pending | Local busy cue without moving the layout |
| Unavailable/error | Readable explanation; error color only for actual failures |

Use one consistent icon family. Existing Google Sans can continue if its actual installed weights cover the chosen styles; a new variable font is not required just to claim an Expressive appearance. Proposed desktop type sizes should be decided as a small role set during mockups, not scattered magic numbers.

### 5. Motion and interaction

Keep the shared 240/180 ms opening/closing behavior for the initial redesign. Give pressed/selected controls a restrained shape response. Keep spatial movement interruptible, do not animate whole lists when service state updates, and do not restart opening motion on every key repeat.

Retain immediate volume changes. Keyboard operation must cover all tile actions, detail navigation, mute, sliders, device selection and close. The toggle and detail targets of a split control must be separate focus stops with clear accessible names. Arrow navigation may accelerate tile use, but must coexist with predictable Tab/Shift+Tab behavior.

### 6. Preserve the working architecture

The existing [Control Center](../control-center.md) already provides the useful foundation: shared sidebar configuration, QML lazy loading, independent request queues, coalesced audio writes and finite Rust operations. Preserve it.

Keep Super+Z, barless layout, on-demand keyboard focus, and unload-after-close behavior. Do not add another shell, permanent animation helper, polling process, framework or settings daemon. Do not change the right btop sidebar as a side effect of redesigning the left panel. Broader shared motion or typography changes should be explicit follow-up decisions.

## Shortlist for the next design pass

1. **Google’s own Expressive specs and Android examples:** component language and source of truth.
2. **NAnDoroid:** the most directly relevant inspected Android-inspired desktop controls.
3. **DMS/Dux’s Control Center:** practical modular composition and compact audio controls.
4. **Caelestia:** shared geometry and transition cohesion; study its actual component code before borrowing motion details.
5. **macOS and GNOME:** grouping and progressive disclosure, translated into Material components.
6. **Wayle and iNiR:** useful integration comparisons for Rust behavior and Niri respectively.

Produce two static design alternatives next: a restrained tonal design and a more expressive one with larger shaped controls. Keep identical functions and frame dimensions so the comparison is about design. Resolve overview/detail navigation, colors and density before deploying anything. The final choice should feel recognizably Google Expressive across Desktop Foundation, rather than looking like several unrelated references pasted together.

## Scope and evidence

This research changed documentation only. No shell was installed, launched or benchmarked, and no live device controls were exercised. Published screenshots were inspected in a background browser; source documentation was used to identify architecture and project status. Visual conclusions are design judgments, not measured performance claims.

The reference board links original hosted images instead of bundling third-party artwork into the repository. Source pages and image links may change; attribution stays attached to each reference. Google pages that returned only a JavaScript placeholder through text retrieval were read in the rendered browser, including shape, sliders, buttons, typography and motion specifications.
