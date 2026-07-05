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

type GraphSuggestion = {
  id: string;
  confidence: number;
  suggested_action: string;
  reasoning: string;
  affected_entities: { display_name?: string; entity_id?: string; entity_type?: string }[];
  status: string;
  action_payload: Record<string, unknown>;
  source_document_id?: string | null;
  source_bill_id?: string | null;
};

type OperationState = {
  error: string | null;
  loading: boolean;
  success: string | null;
};

type StructureEditorProps = {
  graph: HouseholdGraph;
  suggestions: GraphSuggestion[];
};

type PanelMode = "none" | "createEntity" | "createRelationship" | "editEntity";

type PositionedNode = HouseholdGraphNode & {
  x: number;
  y: number;
};

const RELATIONSHIP_EXAMPLES = ["owns", "lives_at", "supplies", "insured_by", "related_to"];
const ENTITY_TYPE_PRESETS = [
  "family_member",
  "family_trust",
  "property",
  "vehicle",
  "business",
  "supplier",
  "account",
  "utility_account",
  "subscription",
];
const TYPE_ORDER = new Map(
  [
    "family_trust",
    "family_member",
    "business",
    "property",
    "vehicle",
    "supplier",
    "account",
    "utility_account",
    "subscription",
    "document",
    "bill",
  ].map((type, index) => [type, index]),
);
const NODE_WIDTH = 220;
const NODE_HEIGHT = 78;
const HORIZONTAL_GAP = 70;
const VERTICAL_GAP = 92;
const CANVAS_PADDING = 36;

function apiBaseUrl() {
  return process.env.NEXT_PUBLIC_API_BASE_URL ?? "http://localhost:8000";
}

function normalizedLabel(value: string) {
  return value.replaceAll("_", " ");
}

function entityOptionLabel(entity: HouseholdGraphNode) {
  return `${entity.display_name} (${normalizedLabel(entity.node_type)})`;
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

function nodeRank(nodeType: string) {
  return TYPE_ORDER.get(nodeType) ?? 99;
}

function nodeInitial(nodeType: string) {
  const parts = normalizedLabel(nodeType).split(" ");
  return parts
    .map((part) => part[0])
    .join("")
    .slice(0, 2)
    .toUpperCase();
}

function suggestionActionLabel(action: string) {
  return normalizedLabel(action).replace(/^./, (character) => character.toUpperCase());
}

function suggestionSummary(suggestion: GraphSuggestion) {
  const names = suggestion.affected_entities
    .map((entity) => entity.display_name)
    .filter(Boolean)
    .join(", ");
  return names || suggestion.reasoning;
}

function layoutGraph(graph: HouseholdGraph) {
  const nodesById = new Map(graph.nodes.map((node) => [node.id, node]));
  const incomingCount = new Map(graph.nodes.map((node) => [node.id, 0]));
  const outgoing = new Map<string, EntityRelationship[]>();

  graph.relationships.forEach((relationship) => {
    if (!nodesById.has(relationship.source_entity_id) || !nodesById.has(relationship.target_entity_id)) {
      return;
    }
    incomingCount.set(
      relationship.target_entity_id,
      (incomingCount.get(relationship.target_entity_id) ?? 0) + 1,
    );
    outgoing.set(relationship.source_entity_id, [
      ...(outgoing.get(relationship.source_entity_id) ?? []),
      relationship,
    ]);
  });

  const sortedNodes = [...graph.nodes].sort(
    (a, b) =>
      nodeRank(a.node_type) - nodeRank(b.node_type) ||
      a.display_name.localeCompare(b.display_name),
  );
  const roots = sortedNodes.filter((node) => (incomingCount.get(node.id) ?? 0) === 0);
  const queue = (roots.length > 0 ? roots : sortedNodes).map((node) => ({
    depth: nodeRank(node.node_type) === 99 ? 0 : Math.min(nodeRank(node.node_type), 4),
    node,
  }));
  const depths = new Map<string, number>();

  queue.forEach(({ depth, node }) => depths.set(node.id, depth));
  while (queue.length > 0) {
    const current = queue.shift();
    if (!current) {
      break;
    }

    for (const relationship of outgoing.get(current.node.id) ?? []) {
      const target = nodesById.get(relationship.target_entity_id);
      if (!target) {
        continue;
      }
      const nextDepth = Math.max(current.depth + 1, nodeRank(target.node_type));
      const existingDepth = depths.get(target.id);
      if (existingDepth === undefined || nextDepth < existingDepth) {
        depths.set(target.id, nextDepth);
        queue.push({ depth: nextDepth, node: target });
      }
    }
  }

  sortedNodes.forEach((node) => {
    if (!depths.has(node.id)) {
      depths.set(node.id, Math.min(nodeRank(node.node_type), 5));
    }
  });

  const levels = new Map<number, HouseholdGraphNode[]>();
  sortedNodes.forEach((node) => {
    const depth = depths.get(node.id) ?? 0;
    levels.set(depth, [...(levels.get(depth) ?? []), node]);
  });

  const maxLevelSize = Math.max(1, ...Array.from(levels.values()).map((nodes) => nodes.length));
  const width = Math.max(760, CANVAS_PADDING * 2 + maxLevelSize * NODE_WIDTH + (maxLevelSize - 1) * HORIZONTAL_GAP);
  const levelNumbers = Array.from(levels.keys()).sort((a, b) => a - b);
  const height = Math.max(460, CANVAS_PADDING * 2 + levelNumbers.length * NODE_HEIGHT + (levelNumbers.length - 1) * VERTICAL_GAP);
  const levelIndex = new Map(levelNumbers.map((depth, index) => [depth, index]));
  const positionedNodes = new Map<string, PositionedNode>();

  levelNumbers.forEach((depth) => {
    const nodes = [...(levels.get(depth) ?? [])].sort(
      (a, b) =>
        nodeRank(a.node_type) - nodeRank(b.node_type) ||
        a.display_name.localeCompare(b.display_name),
    );
    const rowWidth = nodes.length * NODE_WIDTH + Math.max(0, nodes.length - 1) * HORIZONTAL_GAP;
    const rowStart = (width - rowWidth) / 2;
    const rowIndex = levelIndex.get(depth) ?? 0;

    nodes.forEach((node, index) => {
      positionedNodes.set(node.id, {
        ...node,
        x: rowStart + index * (NODE_WIDTH + HORIZONTAL_GAP),
        y: CANVAS_PADDING + rowIndex * (NODE_HEIGHT + VERTICAL_GAP),
      });
    });
  });

  return { height, positionedNodes, width };
}

export function StructureEditor({ graph, suggestions }: StructureEditorProps) {
  const router = useRouter();
  const nodesById = useMemo(
    () => new Map(graph.nodes.map((node) => [node.id, node])),
    [graph.nodes],
  );
  const layout = useMemo(() => layoutGraph(graph), [graph]);
  const [createMetadata, setCreateMetadata] = useState("");
  const [relationshipType, setRelationshipType] = useState("owns");
  const [state, setState] = useState<OperationState>({
    error: null,
    loading: false,
    success: null,
  });
  const [mode, setMode] = useState<PanelMode>("none");
  const [selectedNodeId, setSelectedNodeId] = useState(graph.nodes[0]?.id ?? null);
  const [expandedSuggestionId, setExpandedSuggestionId] = useState<string | null>(null);

  const selectedNode = selectedNodeId ? nodesById.get(selectedNodeId) : undefined;
  const selectedRelationships = useMemo(() => {
    if (!selectedNodeId) {
      return [];
    }
    return graph.relationships.filter(
      (relationship) =>
        relationship.source_entity_id === selectedNodeId ||
        relationship.target_entity_id === selectedNodeId,
    );
  }, [graph.relationships, selectedNodeId]);
  const linkedRecords = selectedRelationships
    .map((relationship) =>
      relationship.source_entity_id === selectedNodeId
        ? nodesById.get(relationship.target_entity_id)
        : nodesById.get(relationship.source_entity_id),
    )
    .filter(
      (node): node is HouseholdGraphNode =>
        !!node && (node.node_type === "document" || node.node_type === "bill"),
    );

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
      setMode("none");
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
    }, "Entity updated.");
  }

  async function createRelationship(formData: FormData) {
    await withState(async () => {
      const sourceEntityId = String(formData.get("source_entity_id") ?? "");
      const targetEntityId = String(formData.get("target_entity_id") ?? "");
      const source = nodesById.get(sourceEntityId);
      const target = nodesById.get(targetEntityId);

      if (sourceEntityId === targetEntityId) {
        throw new Error("Choose two different nodes for a relationship.");
      }
      if (!source || !target) {
        throw new Error("Choose a source and target node.");
      }

      await request("/api/household/relationships", {
        body: JSON.stringify({
          relationship_type: String(formData.get("relationship_type") ?? ""),
          source_entity_id: sourceEntityId,
          source_entity_type: source.node_type,
          target_entity_id: targetEntityId,
          target_entity_type: target.node_type,
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

  async function decideSuggestion(suggestionId: string, decision: "accept" | "reject") {
    await withState(async () => {
      await request(`/api/household/suggestions/${suggestionId}/${decision}`, {
        method: "POST",
      });
    }, decision === "accept" ? "Suggestion accepted." : "Suggestion rejected.");
    if (expandedSuggestionId === suggestionId) {
      setExpandedSuggestionId(null);
    }
  }

  function openRelationshipFromSelected() {
    if (selectedNode) {
      setRelationshipType("related_to");
      setMode("createRelationship");
    }
  }

  return (
    <section className="structureWorkspace" aria-label="Household structure editor">
      <div className="structureActionBar">
        <div>
          <h2>Household structure</h2>
          <span>{graph.nodes.length} nodes, {graph.relationships.length} relationships</span>
        </div>
        <div className="structureToolbar">
          <button type="button" onClick={() => setMode("createEntity")}>
            Add entity
          </button>
          <button
            disabled={graph.nodes.length < 2}
            onClick={() => setMode("createRelationship")}
            type="button"
          >
            Add relationship
          </button>
        </div>
      </div>

      {mode === "createEntity" ? (
        <form action={createEntity} className="structureQuickForm">
          <label>
            <span>Entity type</span>
            <input
              list="entity-type-presets"
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
          <label className="metadataField">
            <span>Metadata JSON</span>
            <textarea
              onChange={(event) => setCreateMetadata(event.target.value)}
              placeholder={'{\n  "role": "primary residence"\n}'}
              value={createMetadata}
            />
          </label>
          <div className="formButtonRow">
            <button disabled={state.loading} type="submit">Create</button>
            <button disabled={state.loading} onClick={() => setMode("none")} type="button">
              Cancel
            </button>
          </div>
        </form>
      ) : null}

      {mode === "createRelationship" ? (
        <form action={createRelationship} className="structureQuickForm">
          <label>
            <span>Source</span>
            <select
              defaultValue={selectedNode?.id ?? ""}
              disabled={graph.nodes.length === 0}
              name="source_entity_id"
              required
            >
              <option value="">Select source</option>
              {graph.nodes.map((node) => (
                <option key={node.id} value={node.id}>
                  {entityOptionLabel(node)}
                </option>
              ))}
            </select>
          </label>
          <label>
            <span>Relationship</span>
            <input
              list="relationship-examples"
              name="relationship_type"
              onChange={(event) => setRelationshipType(event.target.value)}
              required
              type="text"
              value={relationshipType}
            />
          </label>
          <label>
            <span>Target</span>
            <select disabled={graph.nodes.length === 0} name="target_entity_id" required>
              <option value="">Select target</option>
              {graph.nodes.map((node) => (
                <option key={node.id} value={node.id}>
                  {entityOptionLabel(node)}
                </option>
              ))}
            </select>
          </label>
          <div className="formButtonRow">
            <button disabled={state.loading || graph.nodes.length < 2} type="submit">
              Connect
            </button>
            <button disabled={state.loading} onClick={() => setMode("none")} type="button">
              Cancel
            </button>
          </div>
        </form>
      ) : null}

      <datalist id="entity-type-presets">
        {ENTITY_TYPE_PRESETS.map((entityType) => (
          <option key={entityType} value={entityType} />
        ))}
      </datalist>
      <datalist id="relationship-examples">
        {RELATIONSHIP_EXAMPLES.map((example) => (
          <option key={example} value={example} />
        ))}
      </datalist>

      {state.error ? (
        <p className="formMessage errorMessage" role="alert">
          {state.error}
        </p>
      ) : null}
      {state.success ? <p className="formMessage successMessage">{state.success}</p> : null}

      <div className="structureTreeLayout">
        <div className="treeCanvas" aria-label="Household knowledge graph">
          {graph.nodes.length === 0 ? (
            <div className="treeEmptyState">
              <strong>Add your first entity to start building your household knowledge graph.</strong>
              <span>Start with a family member, family trust, property, then supplier.</span>
            </div>
          ) : (
            <div
              className="treeViewport"
              style={{ height: layout.height, width: layout.width }}
            >
              <svg
                aria-hidden="true"
                className="relationshipLayer"
                height={layout.height}
                viewBox={`0 0 ${layout.width} ${layout.height}`}
                width={layout.width}
              >
                <defs>
                  <marker
                    id="tree-arrow"
                    markerHeight="8"
                    markerWidth="8"
                    orient="auto"
                    refX="7"
                    refY="4"
                    viewBox="0 0 8 8"
                  >
                    <path d="M0,0 L8,4 L0,8 Z" fill="#9aa9a3" />
                  </marker>
                </defs>
                {graph.relationships.map((relationship) => {
                  const source = layout.positionedNodes.get(relationship.source_entity_id);
                  const target = layout.positionedNodes.get(relationship.target_entity_id);
                  if (!source || !target) {
                    return null;
                  }

                  const startX = source.x + NODE_WIDTH / 2;
                  const startY = source.y + NODE_HEIGHT;
                  const endX = target.x + NODE_WIDTH / 2;
                  const endY = target.y;
                  const midY = startY + (endY - startY) / 2;
                  const labelX = startX + (endX - startX) / 2;
                  const labelY = midY - 8;
                  const path = `M ${startX} ${startY} C ${startX} ${midY}, ${endX} ${midY}, ${endX} ${endY}`;

                  return (
                    <g key={relationship.id}>
                      <path
                        className="relationshipPath"
                        d={path}
                        markerEnd="url(#tree-arrow)"
                      />
                      <foreignObject
                        height="30"
                        width="140"
                        x={labelX - 70}
                        y={labelY}
                      >
                        <div className="relationshipLabel">
                          {normalizedLabel(relationship.relationship_type)}
                        </div>
                      </foreignObject>
                    </g>
                  );
                })}
              </svg>

              {Array.from(layout.positionedNodes.values()).map((node) => (
                <button
                  aria-pressed={selectedNodeId === node.id}
                  className={`treeNode type-${node.node_type} ${
                    selectedNodeId === node.id ? "selectedTreeNode" : ""
                  }`}
                  key={node.id}
                  onClick={() => {
                    setSelectedNodeId(node.id);
                    setMode("none");
                  }}
                  style={{
                    height: NODE_HEIGHT,
                    left: node.x,
                    top: node.y,
                    width: NODE_WIDTH,
                  }}
                  type="button"
                >
                  <span className="nodeIcon">{nodeInitial(node.node_type)}</span>
                  <span className="nodeText">
                    <strong>{node.display_name}</strong>
                    <span>{normalizedLabel(node.node_type)}</span>
                  </span>
                </button>
              ))}
            </div>
          )}
        </div>

        <aside className="entityDetailsPanel" aria-label="Entity details">
          <section className="suggestionsPanel" aria-label="Graph suggestions">
            <div className="suggestionsHeader">
              <div>
                <h3>Suggestions</h3>
                <span>{suggestions.length} pending</span>
              </div>
            </div>
            {suggestions.length === 0 ? (
              <p className="emptyState">No graph suggestions yet. New document and bill patterns will appear here.</p>
            ) : (
              <div className="suggestionList">
                {suggestions.map((suggestion) => (
                  <article className="suggestionCard" key={suggestion.id}>
                    <div>
                      <strong>{suggestionActionLabel(suggestion.suggested_action)}</strong>
                      <span>{suggestionSummary(suggestion)}</span>
                    </div>
                    <p>{suggestion.reasoning}</p>
                    <div className="suggestionMeta">
                      <span>{Math.round(suggestion.confidence * 100)}% confidence</span>
                      <span>{normalizedLabel(suggestion.suggested_action)}</span>
                    </div>
                    {expandedSuggestionId === suggestion.id ? (
                      <code>{JSON.stringify(suggestion.action_payload, null, 2)}</code>
                    ) : null}
                    <div className="suggestionActions">
                      <button
                        disabled={state.loading}
                        onClick={() => decideSuggestion(suggestion.id, "accept")}
                        type="button"
                      >
                        Accept
                      </button>
                      <button
                        disabled={state.loading}
                        onClick={() => decideSuggestion(suggestion.id, "reject")}
                        type="button"
                      >
                        Reject
                      </button>
                      <button
                        disabled={state.loading}
                        onClick={() =>
                          setExpandedSuggestionId(
                            expandedSuggestionId === suggestion.id ? null : suggestion.id,
                          )
                        }
                        type="button"
                      >
                        View details
                      </button>
                    </div>
                  </article>
                ))}
              </div>
            )}
          </section>

          {selectedNode ? (
            <>
              <div className={`detailsHero type-${selectedNode.node_type}`}>
                <span className="nodeIcon">{nodeInitial(selectedNode.node_type)}</span>
                <div>
                  <strong>{selectedNode.display_name}</strong>
                  <span>{normalizedLabel(selectedNode.node_type)}</span>
                </div>
              </div>

              <div className="detailsActions">
                {selectedNode.node_type !== "document" && selectedNode.node_type !== "bill" ? (
                  <button type="button" onClick={() => setMode("editEntity")}>
                    Edit entity
                  </button>
                ) : null}
                <button type="button" onClick={openRelationshipFromSelected}>
                  Create relationship
                </button>
              </div>

              {mode === "editEntity" &&
              selectedNode.node_type !== "document" &&
              selectedNode.node_type !== "bill" ? (
                <form
                  action={(formData) => updateEntity(selectedNode.id, formData)}
                  className="detailsEditForm"
                >
                  <label>
                    <span>Name</span>
                    <input
                      defaultValue={selectedNode.display_name}
                      name="display_name"
                      required
                      type="text"
                    />
                  </label>
                  <label>
                    <span>Type</span>
                    <input
                      defaultValue={selectedNode.node_type}
                      list="entity-type-presets"
                      name="entity_type"
                      required
                      type="text"
                    />
                  </label>
                  <label>
                    <span>Metadata JSON</span>
                    <textarea
                      defaultValue={metadataToText(selectedNode.metadata)}
                      name="metadata"
                    />
                  </label>
                  <div className="formButtonRow">
                    <button disabled={state.loading} type="submit">Save</button>
                    <button disabled={state.loading} onClick={() => setMode("none")} type="button">
                      Cancel
                    </button>
                  </div>
                </form>
              ) : null}

              <section className="detailSection">
                <h3>Metadata</h3>
                {selectedNode.metadata && Object.keys(selectedNode.metadata).length > 0 ? (
                  <code>{metadataToText(selectedNode.metadata)}</code>
                ) : (
                  <p>No metadata yet.</p>
                )}
              </section>

              <section className="detailSection">
                <h3>Relationships</h3>
                {selectedRelationships.length === 0 ? (
                  <p>No relationships yet.</p>
                ) : (
                  <div className="detailRelationshipList">
                    {selectedRelationships.map((relationship) => {
                      const source = nodesById.get(relationship.source_entity_id);
                      const target = nodesById.get(relationship.target_entity_id);
                      return (
                        <div key={relationship.id} className="detailRelationshipRow">
                          <span>
                            <strong>{source?.display_name ?? relationship.source_entity_type}</strong>{" "}
                            {normalizedLabel(relationship.relationship_type)}{" "}
                            <strong>{target?.display_name ?? relationship.target_entity_type}</strong>
                          </span>
                          <button
                            disabled={state.loading}
                            onClick={() => deleteRelationship(relationship.id)}
                            type="button"
                          >
                            Delete
                          </button>
                        </div>
                      );
                    })}
                  </div>
                )}
              </section>

              <section className="detailSection">
                <h3>Linked documents and bills</h3>
                {linkedRecords.length === 0 ? (
                  <p>No linked documents or bills yet.</p>
                ) : (
                  <div className="linkedRecordList">
                    {linkedRecords.map((record) => (
                      <span key={record.id}>
                        {record.display_name} ({normalizedLabel(record.node_type)})
                      </span>
                    ))}
                  </div>
                )}
              </section>
            </>
          ) : (
            <p className="emptyState">Select a node to see details, relationships, and linked records.</p>
          )}
        </aside>
      </div>
    </section>
  );
}
