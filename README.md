# Programmable Parameter Demo

This demo turns the proposal in `programmable-parameter.md` into runnable code.

It simulates three network functions:

- UDR emits subscription data.
- An intermediate NF forwards data it does not understand.
- AMF runs a hot-swappable WASM applet to verify AI-agent metadata.

Run:

```bash
cargo run -- --scenario rel22-vendor-pass --config configs/rel22.yaml
```

Useful scenarios:

```bash
cargo run -- --scenario strict-breaks
cargo run -- --scenario rel21-pass --config configs/rel21.yaml
cargo run -- --scenario rel22-vendor-pass --config configs/rel22.yaml
cargo run -- --scenario vendor-mismatch --config configs/rel22.yaml
```

The important contrast is:

- `strict-breaks`: a new inline parameter forces intermediate NF schema changes.
- `rel22-vendor-pass`: the same new parameter survives as metadata, and only AMF logic changes through a WASM applet.

## Execution Flow

The sequence of events in the simulation when executing a scenario:

```mermaid
sequenceDiagram
    autonumber
    actor User as CLI Runner
    participant Main as CLI/Simulation (main.rs)
    participant UDR as UDR (Database)
    participant INF as Intermediate NF
    participant AMF as AMF (Access and Mobility)
    participant WASM as WASM VM (Applet Host)

    User->>Main: Runs simulation scenario
    Main->>Main: Load yaml config file from configs directory
    Main->>UDR: Request registration payload
    UDR-->>Main: Return Subscription Data (with metadata) & UE claims

    rect rgb(240, 248, 255)
    Note over Main, INF: 1. Intermediate NF Forwarding
    Main->>INF: Send Subscription Data
    Note over INF: Reads slice and subscriber ID, but forwards metadata container opaque and unchecked.
    INF-->>Main: Forwarded Subscription Data
    end

    rect rgb(255, 245, 245)
    Note over Main, WASM: 2. AMF Applet Verification
    Main->>AMF: Request verification (amf_verify)
    AMF->>AMF: Compile WAT file to WebAssembly module
    AMF->>AMF: Link host functions: metadata_matches_ue, metadata_is, mismatch_action
    AMF->>WASM: Execute verify() entrypoint
    
    WASM->>AMF: Call host function: metadata_matches_ue for aiAgentId
    AMF-->>WASM: Return boolean matches status
    
    WASM->>AMF: Call host function: metadata_is for trustLevel
    AMF-->>WASM: Return boolean status

    opt Rel-22 Applet Version Only
        WASM->>AMF: Call host function: metadata_matches_ue for vendor
        AMF-->>WASM: Return boolean matches status
    end

    alt Verification fails
        WASM->>AMF: Call host function: mismatch_action()
        AMF-->>WASM: Return configured mismatch policy (LIMIT_ACCESS / REJECT)
    end

    WASM-->>AMF: Return decision code (0 = ALLOW, 1 = LIMIT_ACCESS, 2 = REJECT)
    AMF-->>Main: Return Decision enum mapping
    end

    Main-->>User: Print scenario report and final decision
```
