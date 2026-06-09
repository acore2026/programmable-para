# Programmable Parameter Demo

This demo turns the proposal in `programmable-parameter.md` into runnable code.

It simulates three network functions:

- UDR emits subscription data.
- An intermediate NF forwards data it does not understand.
- AMF runs a hot-swappable WASM applet to verify AI-agent metadata.

## Running the Demo

The easiest way to run the entire simulation is using the provided orchestrator script:

```bash
./run_demo.sh
```

This will compile all binaries, start the `intermediate_nf` and `amf` servers in the background, run the UDR trigger client, print the results, and automatically clean up all background processes.

### Running manually in separate terminals

If you prefer to run the components manually:

1. **Terminal 1**: Start Intermediate NF (port 8082):
   ```bash
   cargo run --bin intermediate_nf
   ```
2. **Terminal 2**: Start AMF (port 8083):
   ```bash
   cargo run --bin amf
   ```
3. **Terminal 3**: Run the UDR client trigger:
   ```bash
   cargo run
   ```

The dynamic upgrade verification checks the AI agent ID, trust level, and vendor dynamic parameter, returning the authorization decision `ALLOW` if they match.

## Execution Flow

The sequence of events in the simulation when executing the dynamic upgrade scenario:

```mermaid
sequenceDiagram
    autonumber
    actor UE as User Equipment (UE)
    participant UDR as UDR
    participant INF as Intermediate NF
    
    box LightGray Co-located (In-Process / Same Pod)
    participant AMF as AMF
    participant WASM as WASM
    end

    rect rgb(240, 248, 255)
    Note over UDR, AMF: Phase 1: Rule Provisioning (Push)
    UDR->>INF: Push subscriber data & verification rules (WASM code)
    Note over INF: Reads slice and subscriber ID, but forwards metadata container opaque and unchanged.
    INF->>AMF: Forward subscriber data & verification rules (WASM code)
    AMF-->>INF: Acknowledge (OK)
    INF-->>UDR: Acknowledge (OK)
    end

    rect rgb(255, 245, 245)
    Note over UE, WASM: Phase 2: UE Registration & Local Verification
    UE->>AMF: Request registration (with claims)
    AMF->>AMF: Compile WAT file to WebAssembly module
    AMF->>AMF: Link host functions
    AMF->>WASM: Execute verify() entrypoint
    
    WASM->>AMF: Call host functions (metadata check)
    AMF-->>WASM: Return check values
    
    WASM-->>AMF: Return Decision Code (ALLOW / LIMIT_ACCESS / REJECT)
    Note over AMF: Enforces decision locally (e.g. establishes UE connection)
    end
```
