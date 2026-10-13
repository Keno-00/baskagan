# Project model contract

The host owns `.dado/systems.json`. The background small model returns JSON only, using this versioned shape:

```json
{
  "version": 1,
  "concepts": [
    {
      "id": "project",
      "name": "Ironmanhands",
      "description": "Control a PC using hand gestures.",
      "parent": null,
      "sources": [{"path": ".", "kind": "folder", "symbol": null, "start": null, "end": null}]
    },
    {
      "id": "camera-capture",
      "name": "Capture frames",
      "description": "Read camera frames for hand detection.",
      "parent": "project",
      "sources": [{"path": "camera.py", "kind": "file", "symbol": "capture", "start": 23, "end": 90}]
    }
  ],
  "connections": []
}
```

This is an illustrative shape, not generated evidence. The host rejects nonexistent paths or ranges.

- Exactly one project root, unique stable IDs, no hierarchy cycles. IDs use ASCII letters, digits, hyphens and underscores. Preserve IDs when explanations change.
- Concepts describe purpose, responsibilities, implementation concepts and functions. They need not mirror file hierarchy. A file may support multiple concepts.
- Sources use project-relative paths. `kind` is `folder` or `file`; optional `symbol` must occur inside the referenced range. `start` and `end` are both null or both 1-based inclusive lines. Folders cannot have ranges. Private/generated paths and paths escaping the project are rejected.
- Connections contain `from`, `to`, `kind`, `description`, and `uncertain`. Kinds are `calls`, `data`, `triggers`, `depends`, or `relates`. Endpoints must exist. Inferred connections use `uncertain: true` and render with a dashed line.
- The host records source hashes in `revisions`, independently of model claims. Output bounds: 2,000 concepts, 8,000 connections, 24 references per concept. The initial prompt requests a compact graph.
- Model output never owns annotations, ideas, chats or layout state. Those remain separate. Ideas are proposals, not evidence of implemented functionality.
- Updates receive the previous model, changed paths and exact validation feedback. Changes during generation invalidate that candidate and schedule another pass. Failures preserve the existing artifact.

The native UI shows one concept and its immediate breakdown. Mental model edges express containment and relationships; the workflow view shows directed behavioral connections within that scope. Descent reveals finer concepts. Descriptions and source links live in a reserved surface below the canvas, preserving the graph while reading or annotating.

All three modes use the existing systems scene. Concept graphs adapt to its nodes and edges; camera gestures, title-bar history, overview, reset, atmosphere, and navigation transitions remain shared. Each mode retains its camera and selection. Host-managed `inventory` and `analysis_version` fields support cache freshness and are not generated explanations. Background progress and elapsed time appear in the bottom status strip.

The background request supplies a strict runtime output schema (including paired null/integer source bounds), then validates file existence, type, symbols, and ranges locally. Corrections receive the invalid draft, all detected source errors, and bounded numbered excerpts. Validation details stay internal; the status strip shows concise progress or failure.
