# Programmable Parameter Demo

This demo turns the proposal in `programmable-parameter.md` into runnable code.

It simulates three network functions:

- UDR emits subscription data.
- An intermediate NF forwards data it does not understand.
- AMF runs a hot-swappable WASM applet to verify AI-agent metadata.

## Running the Demo

Since Intermediate NF and AMF are independent background services, you should run them first, then trigger the flow via UDR:

### Step 1: Start the background services (in 2 separate terminals)

1. **Start Intermediate NF (port 8082)**:
   ```bash
   cargo run --bin intermediate_nf
   ```
2. **Start AMF (port 8083)**:
   ```bash
   cargo run --bin amf
   ```

### Step 2: Run the UDR Client Trigger

In a 3rd terminal, run the UDR database process to emit subscription data and trigger the flow:

```bash
cargo run -- --config configs/rel22.yaml
```

The dynamic upgrade verification checks the AI agent ID, trust level, and vendor dynamic parameter, returning the authorization decision `ALLOW` if they match.

## Execution Flow

The sequence of events in the simulation when executing the dynamic upgrade scenario:

```mermaid
sequenceDiagram
    autonumber
    actor User as CLI Runner
    participant Main as Client (main.rs)
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

    WASM->>AMF: Call host function: metadata_matches_ue for vendor
    AMF-->>WASM: Return boolean matches status

    alt Verification fails
        WASM->>AMF: Call host function: mismatch_action()
        AMF-->>WASM: Return configured mismatch policy (LIMIT_ACCESS / REJECT)
    end

    WASM-->>AMF: Return decision code (0 = ALLOW, 1 = LIMIT_ACCESS, 2 = REJECT)
    AMF-->>Main: Return Decision enum mapping
    end

    Main-->>User: Print scenario report and final decision
```
