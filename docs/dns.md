# DNS configuration

DNS management is opt-in. Installation does not change network settings.

```sh
scripts/dns plan
scripts/dns status
scripts/dns verify
scripts/dns journals
scripts/dns apply
scripts/dns apply --connection UUID
scripts/dns restore --connection UUID
```

Without a UUID, apply selects the sole active non-VPN default connection and
otherwise asks for an explicit selection. Find a UUID with `nmcli connection
show --active`. Apply selects an active, non-VPN connection with one device.
It requires NetworkManager configured
to use systemd-resolved and a resolver file pointing exclusively to its stub.
NetworkManager may request authorization. Changes persist with that connection;
repeat the operation explicitly for each additional connection. VPNs and other
connections keep their own DNS and may take precedence for their domains.

The original DNS fields are saved in a private journal before modification. On
failure the operation attempts restoration; if interrupted, run `restore` for
the same UUID. Restore refuses to overwrite subsequent external edits. Keep the
journal until restoration succeeds. Restore currently requires the same connection
to be active for reapplication; it will retain the journal if
that cannot be confirmed.

`plan` reads `config/dns.toml`, followed by alphabetically sorted
`$XDG_CONFIG_HOME/desktop-foundation/dns.d/*.toml` and finally
`$XDG_CONFIG_HOME/desktop-foundation/dns.toml` (defaulting to `~/.config`).
The available providers are `adguard`, `quad9` and `custom`. AdGuard's default
service filters advertisements and trackers. Configuring this computer does not
configure other devices on the network.

```toml
provider = "custom"
transport = "tls"
servers = ["192.0.2.1"]
tls_name = "dns.example.org"
```

TLS requires an authentication hostname and disables plaintext fallback for the
selected connection. DNS-over-HTTPS is not supported by this backend and is
rejected before making changes. Plain DNS is used only when explicitly selected.
A successful lookup in `verify` demonstrates name resolution, not the provider
or transport used. Applying checks the selected link's reported TLS setting and
runs a link-specific, uncached DNS lookup. The installed `resolvectl` must
support `--cache=no`; if it does not, application fails and attempts restoration.
Other applications and VPNs can use their own
resolvers. Browser DNS-over-HTTPS settings are not managed.
