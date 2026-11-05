# DNS configuration

DNS management is not enabled by installation. The DNS commands currently provide
read-only configuration planning and diagnostics; applying and restoring DNS
settings are not yet implemented.

```sh
scripts/dns plan
scripts/dns status
scripts/dns verify
```

`plan` reads `config/dns.toml`, followed by alphabetically sorted
`$XDG_CONFIG_HOME/desktop-foundation/dns.d/*.toml` and finally
`$XDG_CONFIG_HOME/desktop-foundation/dns.toml` (defaulting to `~/.config`).
The available providers are `adguard`, `quad9` and `custom`. AdGuard's default
service filters advertisements and trackers. Configuring this computer would not
configure other devices on the network.

```toml
provider = "custom"
transport = "tls"
servers = ["192.0.2.1"]
tls_name = "dns.example.org"
```

A TLS hostname is required for authenticated DNS-over-TLS. Supported
configuration transport names are `tls`, `https` and `plaintext`; selecting a
transport in a plan does not enable or guarantee it. HTTPS application is not
implemented. A successful `verify` lookup demonstrates only name resolution,
not which provider or transport handled the request. VPN and split-DNS routing
must be respected by any future apply implementation.
