# Beelzebub

Beelzebub is a process uptime monitor designed for tracking video game usage using a client–server model.

## Usage

### Client

The client is distributed as a single Windows binary. Just download the latest release, create the configuration file and run the client.

### Server

The server is currently only distributed as a Docker image due to the binary being a pain to build in GitHub Actions and the fact that I don't personally have any other needs.

```yaml
version: '3.7'
services:
  beelzebub:
    image: ghcr.io/hamuko/beelzebub-server:latest
    container_name: beelzebub
    environment:
      - RUST_LOG=info
    volumes:
      - /path/to/beelzebub/server.yaml:/root/.config/beelzebub/server.yaml
    ports:
      - "3000:8080"  # Make the server available on 0.0.0.0:3000
    restart: on-failure
```

## Configuration

### Client

The client is configured using a Yaml file in `%AppData%\Hamuko\Beelzebub\config\client.yaml`. The configuration will be hot reloaded if it is changed while the client is running.

```yaml
# Monitoring settings
minimumDuration: 60
monitor:
  - C:\Program Files (x86)\Steam\steamapps\common
  - C:\Program Files (x86)\World of Warcraft
  - C:\Program Files\Epic Games

# Server connection settings
url: http://server.internal:8080
secret: secret-authentication-value  # Optional
```

### Server

The server is configured using a Yaml file in `~/.config/beelzebub/server.yaml`.

```yaml
dbUrl: postgres://username:password@database-server.internal/beelzebub
secret: secret-authentication-value  # Optional
```
