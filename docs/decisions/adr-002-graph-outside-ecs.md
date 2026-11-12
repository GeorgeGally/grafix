# ADR-002: The project graph lives outside Bevy ECS

Status: accepted; boundary established in Gate A, graph built in Gate B

## Decision

The creative node graph (types, IDs, validation, evaluation, serialization)
will live in engine-independent crates (`graph-core`, `graph-nodes`,
`graph-runtime`) that compile and test without Bevy or Svelte. Bevy entities,
handles and resources are execution artefacts compiled from the graph and are
never saved.

Gate A sets the boundary the graph will use: hosts drive the engine with typed
`EngineCommand`s through a `CommandQueue` and read a `TelemetrySnapshot`.
`graphics-bevy` contains no graph logic, only the scene, rendering strategies
and that command/telemetry surface.

## Consequences

- Gate B adds graph commands (`LoadGraph`, `UpsertNodes`, `Connect`, `SetParam`,
  ...) to the same queue rather than letting JS or the graph mutate the ECS.
- High-cardinality data (clones, particles) stays in GPU buffers addressed by
  handles, as the custom instancing path already does, not as ECS entities.
