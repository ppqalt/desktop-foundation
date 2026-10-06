# Nothing / CMF controls

Open **Super+B**, select a connected supported device, then choose **Controls**.
The helper identifies the model through the device protocol; controls appear
according to that model's feature map and valid query responses.

Up/Down, Tab/Shift+Tab and scrolling select rows; Home/End and Page Up/Down reach
the rest of the page. Left/Right adjust a value, with Off/On for ordinary switches.
Enter/Space toggles a switch, cycles a preset or activates an action. Escape/Back
returns to paired devices or cancels loading. Adjustment keys apply only to settings.

The card reserves six rows while loading and reveals a complete settings snapshot
together. Switches and value badges retain reported state during requests; the
active row shows Saving, Testing or Ringing. A short accent confirms a setting
only after matching device readback. Errors retain returned values. Reopening
performs fresh queries. The control channel accepts one client at a time; close
other earbud-control applications before opening this page.

## Controls

| Control | Behavior |
|---|---|
| Noise control | Off, transparency and model-specific ANC levels |
| Equalizer / listening preset | Reported standard presets and Custom; CMF listening modes use their own command |
| Custom equalizer | Bass/Mid/Treble from −6 to +6 dB; saving bands and selecting Custom are separate actions |
| Bass Enhance | Enable/disable and levels 1–5 |
| In-ear detection / low latency | Toggles when supported query responses are available |
| Gestures | Reported left/right double, triple, hold and double-hold slots with constrained actions |
| Dual connection | Ear (3), B173: connect two devices |
| Personal sound profile | Ear (3), B173: enable an existing hearing profile |
| Super Mic / auto-transparency | Ear (3), B173: case microphone and transparency during calls |
| Spatial audio | Ear (3), B173: Off or Fixed |
| Audio quality | Ear (3), B173: AAC/LDAC firmware preference and available host SBC/SBC XQ choices |
| Ear-tip fit test | Ear (3), B173: separate left/right seal results |
| Find earbuds | Three-second sound for an available side; excludes B181 |
| Information / refresh / disconnect | Device model, firmware and address; explicit battery refresh; Bluetooth disconnect |

Battery components are shown only when reported, including case/headphone values.
Invalid or missing readings are unavailable. Reports expire after two minutes;
unsolicited reports update the page, and **Refresh battery** requests new values.

**Remove the earbuds from your ears before using Find.** It sends a three-second
ring followed by stop; Back or normal close also requests stop. Find is an action
with no device-state readback. Its side choices require valid battery reports.

For the fit test, wear both earbuds. The test plays sound for about ten seconds;
results mean good seal, adjust tip or check worn state. The helper waits up to
20 seconds for the result. A timeout leaves the result unavailable.

AAC/LDAC preference changes restart the earbuds. The popup discards the old control
state, waits for the reboot and attempts reconnection with bounded retries.
Successful reconnection attempts playback setup and closes the popup. A notification
reports any audio-routing warning. Reopen Controls to read the new firmware preference. PipeWire supplies the actual playback codec.
SBC/SBC XQ instead switch this computer's advertised A2DP profile and confirm the
matching sink codec. These choices apply to the connection; reconnect uses the
normal LDAC/AAC preference order.

## Backend and protocol

`native/nothing` builds `foundation-nothing`, a JSON-lines stdio helper launched
by `scripts/nothing-backend`. Discovery reads paired devices' advertised service
UUIDs when the Bluetooth list opens. Controls lazily registers a BlueZ
ProfileManager1 client for `aeac4a03-dff5-498f-843a-34487cf133eb`, requests
Device1.ConnectProfile and uses the returned RFCOMM descriptor. BlueZ resolves
the channel; the requested device must be paired, connected and advertise the
service. Unknown models retain generic Bluetooth controls.

Packets contain `55 60 01`, little-endian command/length, operation ID, payload and
CRC16/Modbus. Parsing handles fragmented/coalesced data and corrupt-frame recovery
with a 4096-byte payload bound. Responses correlate command and operation ID;
requests use an absolute two-second read deadline. Battery and ANC notifications
can update state during queries. Setting writes are followed by the corresponding
read query and related EQ/Bass/listening queries where needed.

BlueZ events or RFCOMM EOF discard control state on disconnect/restart. Closing
the page closes its helper; a 1.5-second fallback forces shutdown if needed.

```sh
scripts/build-nothing
scripts/nothing-backend --discover
scripts/nothing-backend /org/bluez/hci0/dev_XX_XX_XX_XX_XX_XX
```

Close the UI controls before opening the diagnostic helper. It accepts JSON
lines such as `{"action":"refresh"}`, `{"setting":"anc","value":7}` and
`{"action":"close"}`. Replies contain device state and readback confirmation.
Normal [installation](installation.md) builds both Rust backends.

The model database and protocol codecs credit Daan Hessen's
[earctl](https://github.com/DaanHessen/earctl) and
[ear-web](https://github.com/radiance-project/ear-web). Source revisions and
provenance are recorded in [NOTICE](../native/nothing/NOTICE.md). Distribution of
the helper must include its complete source, lockfile and
[GNU AGPL license](../native/nothing/LICENSE).
