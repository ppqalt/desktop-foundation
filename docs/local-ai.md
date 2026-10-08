# Local AI

Odysseus appears in the application launcher when its existing Compose project is configured. Opening it starts stopped Ollama and Docker services, starts existing Odysseus containers, checks local health and the active Ollama model inventory, then opens a new browser window at `http://localhost:7000/`.

```bash
scripts/local-ai
scripts/local-ai plan
scripts/local-ai status
```

`plan` reads configuration and executable availability. `status` reads systemd and container lifecycle metadata. Neither contacts HTTP endpoints. The launcher exits after handing off to the browser; it adds no background supervisor.

## Configuration

Defaults live in `config/local-ai.toml`. Override individual values in `~/.config/desktop-foundation/local-ai.toml`:

```toml
compose_directory = "~/odysseus"
url = "http://localhost:7000/"
ollama_url = "http://localhost:11434/"
startup_timeout_seconds = 90
```

URLs accept HTTP on `localhost`, `127.0.0.1` or `[::1]`, with an explicit port. They must refer to the server root. The timeout accepts 10–120 seconds.

Install Docker, Docker Compose, Ollama and curl, then complete the [upstream Odysseus setup](https://github.com/odysseus-dev/odysseus). The desktop launcher adopts that installation. Docker access must already work for your user. Starting system services may show the normal authorization dialog.

The launcher uses [Docker Compose `start --wait`](https://docs.docker.com/reference/cli/docker/compose/start/) for existing containers. It does not create containers, pull images, build the app, change networking or choose/download a model. Choose your model in Odysseus as usual.

Containerized Odysseus needs a route to Ollama on the host. Keep the existing Docker gateway and firewall rules that provide that route. Binding Ollama exclusively to host loopback would block that connection. The browser UI can remain bound to loopback.

Health checks read only HTTP status lines from Ollama's `/api/version` and Odysseus's `/api/health`. After liveness succeeds, the launcher reads model lifecycle metadata from [Ollama `/api/tags`](https://docs.ollama.com/api/tags), with a 5-second deadline and 1 MiB response limit. An empty active model store produces an actionable error instead of opening an unusable chat. This check does not load a model or run inference. The launcher never reads application logs, chats or model/account settings.

## Which model store is active?

The system Ollama service and a manually started user instance may have separate stores. Models under `~/.ollama/models` do not become available automatically to a system service using `/var/lib/ollama`; the service's `ProtectHome` restriction remains in place.

Import existing local weights through Ollama's supported blob/create API, or pull the model into the active service. Register the exact model identity that Odysseus is configured to request, including its namespace and tag. The launcher checks that the service has an installed model, while Odysseus continues to own model selection.
