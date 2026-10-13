# Project model contract

This file defines how project understanding moves from model output to the Systems view. First, the small model proposes concepts, sources, and connections. Then the host checks that each reference matches the project. Only after those checks pass does SDE save the model and use it for navigation and focused chat context. The JSON below shows the required shape; it is an example, not evidence from a real project.

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

This is an illustrative shape, not generated evidence. For a generated model, the host rejects nonexistent paths or ranges before saving it.

1. **Build the concept hierarchy.** Include exactly one project root, unique stable IDs, and no hierarchy cycles. IDs use ASCII letters, digits, hyphens, and underscores. Keep an ID when its explanation changes. Concepts should explain purpose, responsibilities, implementation concepts, and functions. They do not need to copy the folder tree, and one file can support more than one concept.
2. **Attach source evidence.** Use project-relative paths. Set `kind` to `folder` or `file`. An optional `symbol` must appear inside the referenced range. Set `start` and `end` together: both are null, or both are 1-based inclusive line numbers. Folders cannot have ranges. The host rejects private or generated paths and paths that leave the project.
3. **Describe relationships.** Each connection has `from`, `to`, `kind`, `description`, and `uncertain`. A connection's endpoints must refer to existing concepts. Use `calls`, `data`, `triggers`, `depends`, or `relates` for `kind`. If the relationship is inferred rather than confirmed, set `uncertain` to `true`; SDE shows that connection with a dashed line.
4. **Let the host validate and save.** The host records source hashes in `revisions` independently of the model's claims. Output is limited to 2,000 concepts, 8,000 connections, and 24 references per concept. The initial prompt asks for a compact graph. During an update, the model receives the previous model, changed paths, and exact validation feedback. If sources change while generation is running, that candidate is invalidated and another pass is scheduled. A failed update keeps the existing saved model.

Annotations, ideas, chats, and layout state stay outside the generated model. This keeps generated explanations separate from user notes and workspace state. Ideas are proposals; they do not prove that a feature is implemented.

In the native UI, begin with one concept and its immediate breakdown. Mental model edges show containment and concept relationships. The workflow view shows directed behavioral connections within the same scope. Descending reveals finer concepts. Descriptions and source links appear in a reserved surface below the canvas, so reading or annotating does not move the graph.

All three modes use the same Systems scene. The graph changes its nodes and edges for the selected mode, while camera gestures, title-bar history, overview, reset, atmosphere, and navigation transitions stay shared. Each mode keeps its own camera and selection. The host manages `inventory` and `analysis_version` for cache freshness; these fields are not generated explanations. While the model runs, the bottom status strip shows progress and elapsed time.

For each background request, SDE first supplies a strict output schema. For source bounds, the schema requires either two null values or two integers. After the model responds, the host checks that files exist and that file types, symbols, and ranges are valid. If correction is needed, the model receives its invalid draft, all detected source errors, and bounded numbered excerpts. Validation details stay internal; the status strip shows a short progress or failure message.
