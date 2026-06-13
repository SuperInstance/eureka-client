# Netflix Eureka Service Discovery Client

**eureka-client** is an async Rust client for Netflix Eureka, the service registry that enables dynamic service registration, heartbeat-based liveness, and lookup in microservice architectures. It speaks the Eureka REST API natively and handles the full registration lifecycle.

## Why It Matters

Eureka is the service discovery backbone of Netflix, Spring Cloud, and many AWS deployments. In a microservice mesh, every service instance must register itself on startup, send periodic heartbeats to stay alive, and discover downstream services by name — all without hard-coded addresses. This client brings that capability to Rust services, enabling zero-config service-to-service communication in Eureka-based infrastructure. Any Rust microservice deployed to Spring Cloud or NetflixOSS environments needs this to participate in the registry.

## How It Works

Eureka operates on a **pull model with push heartbeats**:

1. **Registration** — On startup, a service `POST`s its `InstanceInfo` (IP, port, status) to `/eureka/apps/{appName}`. Eureka adds the instance to its registry.
2. **Heartbeat** — Every 30 seconds (configurable via `renewal_interval`), the instance sends a `PUT` to `/eureka/apps/{appName}/{instanceId}`. If Eureka doesn't receive a heartbeat within 90 seconds (3 intervals), it evicts the instance.
3. **Discovery** — Clients `GET /eureka/apps/{appName}` (or `/eureka/apps` for all) to fetch the current instance list. Clients poll every `fetch_interval` (default 30s).
4. **Deregistration** — On graceful shutdown, the instance sends `DELETE /eureka/apps/{appName}/{instanceId}`.

**Registration is idempotent**: re-registering the same `instanceId` updates metadata rather than creating duplicates.

**Load balancing** happens client-side: the caller fetches all instances for a service, then selects one (round-robin, random, or zone-affinity). This client provides the raw instance list; the selection strategy is the caller's responsibility.

The `InstanceStatus` state machine is: `STARTING → UP → (OUT_OF_SERVICE | DOWN)`. Eureka only routes traffic to `UP` instances.

## Quick Start

```rust
use eureka_client::{EurekaClient, EurekaConfig, InstanceInfo, InstanceStatus, PortInfo};

#[tokio::main]
async fn main() -> eureka_client::Result<()> {
    let config = EurekaConfig {
        server_url: "http://eureka:8761".into(),
        app_name: "rust-service".into(),
        instance_id: "rust-service-001".into(),
        ip_address: "10.0.1.5".into(),
        port: 8080,
        ..Default::default()
    };
    let client = EurekaClient::new(config)?;

    // Register this instance
    let instance = InstanceInfo {
        instance_id: "rust-service-001".into(),
        app_name: "rust-service".into(),
        ip_addr: "10.0.1.5".into(),
        port: PortInfo { port: 8080, enabled: true },
        status: InstanceStatus::UP,
        metadata: None,
    };
    client.register(&instance).await?;

    // Discover another service
    let app = client.get_application("downstream-service").await?;
    for inst in &app.instance {
        println!("Found instance: {}:{} ({:?})", inst.ip_addr, inst.port.port, inst.status);
    }

    // Heartbeat loop (in production, run in a background task)
    client.heartbeat("rust-service", "rust-service-001").await?;

    Ok(())
}
```

## API

### Core Types

- **`EurekaClient`** — HTTP client wrapping `reqwest`. Owns connection pool.
- **`EurekaConfig`** — Server URL, instance identity, renewal/fetch intervals.
- **`InstanceInfo`** — Serializable instance descriptor (`instance_id`, `app_name`, `ip_addr`, `port`, `status`, `metadata`).
- **`Application`** — A named service with its list of registered `InstanceInfo` instances.
- **`InstanceStatus`** — Enum: `UP`, `DOWN`, `STARTING`, `OUT_OF_SERVICE`.

### Methods

| Method | HTTP | Description |
|--------|------|-------------|
| `register` | `POST /eureka/apps/{app}` | Register an instance |
| `deregister` | `DELETE /eureka/apps/{app}/{id}` | Remove an instance |
| `heartbeat` | `PUT /eureka/apps/{app}/{id}` | Renew lease |
| `get_application` | `GET /eureka/apps/{app}` | Fetch instances for one service |
| `get_all_applications` | `GET /eureka/apps` | Fetch entire registry |
| `health` | `GET /eureka/status` | Check Eureka server health |

### Errors

**`EurekaError`** variants: `Http(reqwest::Error)`, `ServiceNotFound(String)`, `RegistrationFailed(String)`.

## Architecture Notes

In the SuperInstance ecosystem, this crate provides the service discovery plane. Nodes register with Eureka on boot, discover peers through the registry, and use heartbeats as a liveness signal feeding into the γ + η = C consensus layer. Eureka's eventual-consistency model complements etcd's strong consistency: discovery uses Eureka for speed, while coordination falls back to etcd for correctness.

See [ARCHITECTURE.md](https://github.com/SuperInstance/SuperInstance/blob/main/ARCHITECTURE.md) for the full design.

## References

- Netflix. *Eureka at a Glance*. [github.com/Netflix/eureka](https://github.com/Netflix/eureka/wiki/Eureka-at-a-glance)
- Eisenbach, S. (2015). *Spring Cloud Netflix: Service Discovery with Eureka*. Spring Blog.
- Beyer, B., et al. (2016). *Site Reliability Engineering*. O'Reilly. Chapter 6 on service communication.

## License

MIT
