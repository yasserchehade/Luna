"use client";

import { useMemo, useState } from "react";
import { useRouter } from "next/navigation";

type HouseholdGraphNode = {
  id: string;
  node_type: string;
  display_name: string;
  metadata?: Record<string, unknown>;
};

type EntityRelationship = {
  id: string;
  source_entity_type: string;
  source_entity_id: string;
  relationship_type: string;
  target_entity_type: string;
  target_entity_id: string;
};

type HouseholdGraph = {
  nodes: HouseholdGraphNode[];
  relationships: EntityRelationship[];
};

type OperationState = {
  error: string | null;
  loading: boolean;
  success: string | null;
};

type StructureEditorProps = {
  graph: HouseholdGraph;
};

const RELATIONSHIP_EXAMPLES = ["owns", "lives_at", "supplies", "insured_by", "related_to"];

function apiBaseUrl() {
  return process.env.NEXT_PUBLIC_API_BASE_URL ?? "http://localhost:8000";
}

function normalizedLabel(value: string) {
  return value.replaceAll("_", " ");
}

function metadataToText(metadata?: Record<string, unknown>) {
  if (!metadata || Object.keys(metadata).length === 0) {
    return "";
  }
  return JSON.stringify(metadata, null, 2);
}

function parseMetadata(value: string) {
  if (value.trim().length === 0) {
    return {};
  }

  const parsed = JSON.parse(value) as unknown;
  if (parsed === null || Array.isArray(parsed) || typeof parsed !== "object") {
    throw new Error("Metadata must be a JSON object.");
  }
  return parsed as Record<string, unknown>;
}

export function StructureEditor({ graph }: StructureEditorProps) {
  const router = useRouter();
  const entityNodes = useMemo(
    () => graph.nodes.filter((node) => node.node_type !== "document"),
    [graph.nodes],
  );
  const nodesById = useMemo(
    () => new Map(graph.nodes.map((node) => [node.id, node])),
    [graph.nodes],
  );
  const groupedEntities = useMemo(() => {
    return entityNodes.reduce<Record<string, HouseholdGraphNode[]>>((groups, entity) => {
      const group = groups[entity.node_type] ?? [];
      group.push(entity);
      groups[entity.node_type] = group;
      return groups;
    }, {});
  }, [entityNodes]);
  const entityTypes = Object.keys(groupedEntities).sort();

  const [createMetadata, setCreateMetadata] = useState("");
  const [relationshipType, setRelationshipType] = useState("owns");
  const [state, setState] = useState<OperationState>({
    error: null,
    loading: false,
    success: null,
  });
  const [editingEntityId, setEditingEntityId] = useState<string | null>(null);

  async function request(path: string, options: RequestInit) {
    setState({ error: null, loading: true, success: null });

    const response = await fetch(`${apiBaseUrl()}${path}`, {
      ...options,
      headers: {
        "Content-Type": "application/json",
        ...(options.headers ?? {}),
      },
    });

    if (!response.ok) {
      let detail = "Structure update failed.";
      try {
        const body = (await response.json()) as { detail?: string };
        detail = body.detail ?? detail;
      } catch {
        detail = response.statusText || detail;
      }
      throw new Error(detail);
    }
  }

  async function withState(action: () => Promise<void>, success: string) {
    try {
      await action();
      setState({ error: null, loading: false, success });
      router.refresh();
    } catch (error) {
      setState({
        error: error instanceof Error ? error.message : "Structure update failed.",
        loading: false,
        success: null,
      });
    }
  }

  async function createEntity(formData: FormData) {
    await withState(async () => {
      const metadata = parseMetadata(createMetadata);
      await request("/api/household/entities", {
        body: JSON.stringify({
          display_name: String(formData.get("display_name") ?? ""),
          entity_type: String(formData.get("entity_type") ?? ""),
          metadata,
        }),
        method: "POST",
      });
      setCreateMetadata("");
    }, "Entity created.");
  }

  async function updateEntity(entityId: string, formData: FormData) {
    await withState(async () => {
      const metadata = parseMetadata(String(formData.get("metadata") ?? ""));
      await request(`/api/household/entities/${entityId}`, {
        body: JSON.stringify({
          display_name: String(formData.get("display_name") ?? ""),
          entity_type: String(formData.get("entity_type") ?? ""),
          metadata,
        }),
        method: "PATCH",
      });
      setEditingEntityId(null);
    }, "Entity updated.");
  }

  async function createRelationship(formData: FormData) {
    await withState(async () => {
      await request("/api/household/relationships", {
        body: JSON.stringify({
          relationship_type: String(formData.get("relationship_type") ?? ""),
          source_entity_id: String(formData.get("source_entity_id") ?? ""),
          target_entity_id: String(formData.get("target_entity_id") ?? ""),
        }),
        method: "POST",
      });
    }, "Relationship created.");
  }

  async function deleteRelationship(relationshipId: string) {
    await withState(async () => {
      await request(`/api/household/relationships/${relationshipId}`, {
        method: "DELETE",
      });
    }, "Relationship deleted.");
  }

  return (
    <section className="structureEditor" aria-label="Household structure editor">
      <div className="panel">
        <div className="panelHeader">
          <h2>Add entity</h2>
          <span>node</span>
        </div>
        <form action={createEntity} className="structureForm">
          <label>
            <span>Entity type</span>
            <input
              name="entity_type"
              placeholder="family_trust, property, vehicle"
              required
              type="text"
            />
          </label>
          <label>
            <span>Display name</span>
            <input name="display_name" placeholder="Smith Family Trust" required type="text" />
          </label>
          <label>
            <span>Metadata JSON</span>
            <textarea
              onChange={(event) => setCreateMetadata(event.target.value)}
              placeholder={'{\n  "role": "primary residence"\n}'}
              value={createMetadata}
            />
          </label>
          <button disabled={state.loading} type="submit">
            Create entity
          </button>
        </form>
      </div>

      <div className="panel">
        <div className="panelHeader">
          <h2>Add relationship</h2>
          <span>edge</span>
        </div>
        <form action={createRelationship} className="structureForm">
          <label>
            <span>Source</span>
            <select disabled={entityNodes.length === 0} name="source_entity_id" required>
              <option value="">Select source</option>
              {entityNodes.map((entity) => (
                <option key={entity.id} value={entity.id}>
                  {entity.display_name}
                </option>
              ))}
            </select>
          </label>
          <label>
            <span>Relationship type</span>
            <input
              list="relationship-examples"
              name="relationship_type"
              onChange={(event) => setRelationshipType(event.target.value)}
              required
              type="text"
              value={relationshipType}
            />
            <datalist id="relationship-examples">
              {RELATIONSHIP_EXAMPLES.map((example) => (
                <option key={example} value={example} />
              ))}
            </datalist>
          </label>
          <label>
            <span>Target</span>
            <select disabled={entityNodes.length === 0} name="target_entity_id" required>
              <option value="">Select target</option>
              {entityNodes.map((entity) => (
                <option key={entity.id} value={entity.id}>
                  {entity.display_name}
                </option>
              ))}
            </select>
          </label>
          <div className="exampleRow" aria-label="Relationship examples">
            {RELATIONSHIP_EXAMPLES.map((example) => (
              <button
                disabled={state.loading}
                key={example}
                onClick={() => setRelationshipType(example)}
                type="button"
              >
                {example}
              </button>
            ))}
          </div>
          <button disabled={state.loading || entityNodes.length < 2} type="submit">
            Create relationship
          </button>
        </form>
      </div>

      <div className="panel widePanel">
        <div className="panelHeader">
          <h2>Entities</h2>
          <span>{entityNodes.length} nodes</span>
        </div>
        {entityNodes.length === 0 ? (
          <p className="emptyState">Create the first household entity to start the graph.</p>
        ) : (
          <div className="entityGroups">
            {entityTypes.map((entityType) => (
              <section key={entityType} className="entityGroup">
                <h3>{normalizedLabel(entityType)}</h3>
                <div className="entityList">
                  {groupedEntities[entityType].map((entity) => (
                    <div key={entity.id} className="entityRow editableEntityRow">
                      {editingEntityId === entity.id ? (
                        <form action={(formData) => updateEntity(entity.id, formData)}>
                          <input
                            aria-label="Entity display name"
                            defaultValue={entity.display_name}
                            name="display_name"
                            required
                            type="text"
                          />
                          <input
                            aria-label="Entity type"
                            defaultValue={entity.node_type}
                            name="entity_type"
                            required
                            type="text"
                          />
                          <textarea
                            aria-label="Entity metadata JSON"
                            defaultValue={metadataToText(entity.metadata)}
                            name="metadata"
                          />
                          <div className="rowActions">
                            <button disabled={state.loading} type="submit">
                              Save
                            </button>
                            <button
                              disabled={state.loading}
                              onClick={() => setEditingEntityId(null)}
                              type="button"
                            >
                              Cancel
                            </button>
                          </div>
                        </form>
                      ) : (
                        <>
                          <strong>{entity.display_name}</strong>
                          <span>{normalizedLabel(entity.node_type)}</span>
                          {entity.metadata && Object.keys(entity.metadata).length > 0 ? (
                            <code>{metadataToText(entity.metadata)}</code>
                          ) : null}
                          <div className="rowActions">
                            <button
                              disabled={state.loading}
                              onClick={() => setEditingEntityId(entity.id)}
                              type="button"
                            >
                              Edit
                            </button>
                          </div>
                        </>
                      )}
                    </div>
                  ))}
                </div>
              </section>
            ))}
          </div>
        )}
      </div>

      <div className="panel relationshipPanel">
        <div className="panelHeader">
          <h2>Relationships</h2>
          <span>{graph.relationships.length} edges</span>
        </div>
        {graph.relationships.length === 0 ? (
          <p className="emptyState">Connect entities to describe ownership, suppliers, and obligations.</p>
        ) : (
          <div className="taskList">
            {graph.relationships.map((relationship) => {
              const source = nodesById.get(relationship.source_entity_id);
              const target = nodesById.get(relationship.target_entity_id);
              const sourceName = source?.display_name ?? relationship.source_entity_id;
              const targetName = target?.display_name ?? relationship.target_entity_id;

              return (
                <div key={relationship.id} className="taskRow relationshipRow">
                  <strong>
                    {sourceName} {normalizedLabel(relationship.relationship_type)} {targetName}
                  </strong>
                  <span>
                    {normalizedLabel(relationship.source_entity_type)} to{" "}
                    {normalizedLabel(relationship.target_entity_type)}
                  </span>
                  <div className="rowActions">
                    <button
                      disabled={state.loading}
                      onClick={() => deleteRelationship(relationship.id)}
                      type="button"
                    >
                      Delete
                    </button>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>

      {state.error ? (
        <p className="formMessage errorMessage" role="alert">
          {state.error}
        </p>
      ) : null}
      {state.success ? <p className="formMessage successMessage">{state.success}</p> : null}
    </section>
  );
}
