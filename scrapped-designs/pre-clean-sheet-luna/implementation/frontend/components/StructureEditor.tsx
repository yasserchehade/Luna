"use client";

import { useEffect, useMemo, useRef, useState } from "react";
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
  provenance_document_id?: string | null;
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
  initialMode?: "createEntity" | "createRelationship";
  suggestions: GraphSuggestion[];
};

type PanelMode = "none" | "createEntity" | "createRelationship" | "editEntity";

type PositionedNode = HouseholdGraphNode & {
  x: number;
  y: number;
};

type DetailField = {
  key: string;
  label: string;
  placeholder?: string;
};

const RELATIONSHIP_EXAMPLES = ["owns", "lives_at", "supplies", "insured_by", "related_to"];
const ENTITY_TYPE_PRESETS = [
  "family_member",
  "family_trust",
  "property",
  "vehicle",
  "business",
  "supplier",
  "financial_account",
  "insurance_policy",
  "utility_account",
  "subscription",
];
const TYPE_ORDER = new Map(
  [
    "family_member",
    "family_trust",
    "business",
    "property",
    "vehicle",
    "utility_account",
    "insurance_policy",
    "supplier",
    "financial_account",
    "bank_account",
    "account",
    "subscription",
  ].map((type, index) => [type, index]),
);
const GROUPS = [
  { id: "family", label: "Family", types: ["family_member", "family_trust"] },
  { id: "properties", label: "Properties", types: ["property"] },
  { id: "vehicles", label: "Vehicles", types: ["vehicle"] },
  { id: "businesses", label: "Businesses", types: ["business"] },
  { id: "utilities", label: "Utilities", types: ["utility_account"] },
  { id: "insurance", label: "Insurance", types: ["insurance_policy"] },
  { id: "suppliers", label: "Suppliers", types: ["supplier"] },
  { id: "financial_accounts", label: "Financial Accounts", types: ["financial_account", "bank_account", "account"] },
  { id: "subscriptions", label: "Subscriptions", types: ["subscription"] },
];
const DETAIL_FIELDS: Record<string, DetailField[]> = {
  property: [
    { key: "address", label: "Address" },
    { key: "council", label: "Council" },
    { key: "state", label: "State" },
    { key: "purchase_date", label: "Purchase date" },
    { key: "settlement_date", label: "Settlement date" },
    { key: "notes", label: "Notes" },
  ],
  vehicle: [
    { key: "registration", label: "Registration" },
    { key: "make", label: "Make" },
    { key: "model", label: "Model" },
    { key: "year", label: "Year" },
    { key: "vin", label: "VIN" },
    { key: "notes", label: "Notes" },
  ],
  family_member: [
    { key: "role", label: "Role" },
    { key: "phone", label: "Phone" },
    { key: "email", label: "Email" },
    { key: "notes", label: "Notes" },
  ],
  family_trust: [
    { key: "abn", label: "ABN" },
    { key: "established_date", label: "Established date" },
    { key: "notes", label: "Notes" },
  ],
  supplier: [
    { key: "website", label: "Website" },
    { key: "phone", label: "Phone" },
    { key: "email", label: "Email" },
    { key: "customer_number", label: "Customer number" },
    { key: "abn", label: "ABN" },
    { key: "notes", label: "Notes" },
  ],
  insurance_policy: [
    { key: "policy_number", label: "Policy number" },
    { key: "renewal_date", label: "Renewal date" },
    { key: "coverage", label: "Coverage" },
    { key: "excess", label: "Excess" },
    { key: "notes", label: "Notes" },
  ],
  utility_account: [
    { key: "account_number", label: "Account number" },
    { key: "service_type", label: "Service type" },
    { key: "notes", label: "Notes" },
  ],
  financial_account: [
    { key: "account_number", label: "Account number" },
    { key: "institution", label: "Institution" },
    { key: "notes", label: "Notes" },
  ],
  bank_account: [
    { key: "account_number", label: "Account number" },
    { key: "institution", label: "Institution" },
    { key: "notes", label: "Notes" },
  ],
  account: [
    { key: "account_number", label: "Account number" },
    { key: "institution", label: "Institution" },
    { key: "notes", label: "Notes" },
  ],
  subscription: [
    { key: "account_number", label: "Account number" },
    { key: "renewal_date", label: "Renewal date" },
    { key: "notes", label: "Notes" },
  ],
};
const HIDDEN_DETAIL_KEYS = new Set([
  "company_name",
  "full_name",
  "insurer",
  "owner",
  "property",
  "supplier",
  "trust_name",
  "trustee",
]);
const NAVIGATOR_STORAGE_KEY = "luna.navigator.view";
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

function titleCase(value: string) {
  return normalizedLabel(value).replace(/\b\w/g, (character) => character.toUpperCase());
}

function entityOptionLabel(entity: HouseholdGraphNode) {
  return `${entity.display_name} (${titleCase(entity.node_type)})`;
}

function detailFieldsFor(type: string) {
  return DETAIL_FIELDS[type] ?? [
    { key: "notes", label: "Notes" },
  ];
}

function nameLabelFor(type: string) {
  if (type === "family_member") return "Person name";
  if (type === "property") return "Property name";
  if (type === "vehicle") return "Vehicle name";
  if (type === "family_trust") return "Trust name";
  if (type === "supplier") return "Company name";
  if (type === "insurance_policy") return "Policy name";
  if (type === "utility_account") return "Account name";
  if (type === "financial_account" || type === "bank_account" || type === "account") return "Account name";
  if (type === "subscription") return "Subscription name";
  return "Name";
}

function isEvidenceType(type: string) {
  return type === "document" || type === "bill";
}

function detailsFromForm(formData: FormData, type: string) {
  const details: Record<string, unknown> = {};
  for (const field of detailFieldsFor(type)) {
    const value = String(formData.get(`detail_${field.key}`) ?? "").trim();
    if (value) {
      details[field.key] = value;
    }
  }
  return details;
}

function detailValue(metadata: Record<string, unknown> | undefined, key: string) {
  const value = metadata?.[key];
  return typeof value === "string" || typeof value === "number" ? String(value) : "";
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

function confidenceLabel(confidence: number) {
  if (confidence >= 0.8) return "High confidence";
  if (confidence >= 0.55) return "Medium confidence";
  return "Low confidence";
}

function suggestionTitle(suggestion: GraphSuggestion) {
  const payload = suggestion.action_payload;
  const affectedNames = suggestion.affected_entities
    .map((entity) => entity.display_name)
    .filter(Boolean);
  if (suggestion.suggested_action === "create_entity") {
    const entityType = String(payload.entity_type ?? "item");
    const displayName = String(payload.display_name ?? affectedNames[0] ?? "new item");
    if (entityType === "property") return `Luna found a property: ${displayName}`;
    if (entityType === "vehicle") return `Luna found a vehicle: ${displayName}`;
    if (entityType === "insurance_policy") return `Luna found an insurance policy: ${displayName}`;
    if (entityType === "utility_account") return `Luna found a utility account: ${displayName}`;
    if (entityType === "supplier") return `Luna found a supplier: ${displayName}`;
    return `Luna found ${displayName}`;
  }
  if (suggestion.suggested_action === "attach_document") {
    return affectedNames.length >= 2
      ? `This document appears to belong to ${affectedNames[1]}`
      : "This document appears to belong somewhere in your household";
  }
  if (suggestion.suggested_action === "connect_entities") {
    return affectedNames.length >= 2
      ? `Link ${affectedNames[0]} to ${affectedNames[1]}`
      : "Luna found a useful link";
  }
  if (suggestion.suggested_action === "update_metadata") return "Luna found details to add";
  return "Luna found something useful";
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
    if (!current) break;
    for (const relationship of outgoing.get(current.node.id) ?? []) {
      const target = nodesById.get(relationship.target_entity_id);
      if (!target) continue;
      const nextDepth = Math.max(current.depth + 1, nodeRank(target.node_type));
      const existingDepth = depths.get(target.id);
      if (existingDepth === undefined || nextDepth < existingDepth) {
        depths.set(target.id, nextDepth);
        queue.push({ depth: nextDepth, node: target });
      }
    }
  }

  sortedNodes.forEach((node) => {
    if (!depths.has(node.id)) depths.set(node.id, Math.min(nodeRank(node.node_type), 5));
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

export function StructureEditor({ graph, initialMode, suggestions }: StructureEditorProps) {
  const router = useRouter();
  const canvasRef = useRef<HTMLDivElement | null>(null);
  const nodesById = useMemo(
    () => new Map(graph.nodes.map((node) => [node.id, node])),
    [graph.nodes],
  );
  const [createType, setCreateType] = useState("property");
  const [relationshipType, setRelationshipType] = useState("owns");
  const [state, setState] = useState<OperationState>({
    error: null,
    loading: false,
    success: null,
  });
  const [mode, setMode] = useState<PanelMode>("none");
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const [expandedSuggestionId, setExpandedSuggestionId] = useState<string | null>(null);
  const [selectedGroup, setSelectedGroup] = useState("family");
  const [focusSelected, setFocusSelected] = useState(true);
  const [expandedGroupIds, setExpandedGroupIds] = useState<Set<string>>(() => new Set());
  const [editingRelationshipId, setEditingRelationshipId] = useState<string | null>(null);
  const [hasLoadedViewState, setHasLoadedViewState] = useState(false);

  const selectedNode = selectedNodeId ? nodesById.get(selectedNodeId) : undefined;
  const connectedNodeIds = useMemo(() => {
    if (!selectedNodeId) return new Set<string>();
    const ids = new Set<string>([selectedNodeId]);
    graph.relationships.forEach((relationship) => {
      if (
        relationship.source_entity_id === selectedNodeId &&
        nodesById.has(relationship.target_entity_id)
      ) {
        ids.add(relationship.target_entity_id);
      }
      if (
        relationship.target_entity_id === selectedNodeId &&
        nodesById.has(relationship.source_entity_id)
      ) {
        ids.add(relationship.source_entity_id);
      }
    });
    return ids;
  }, [graph.relationships, nodesById, selectedNodeId]);
  const selectedRelationships = useMemo(() => {
    if (!selectedNodeId) return [];
    return graph.relationships.filter(
      (relationship) =>
        nodesById.has(relationship.source_entity_id) &&
        nodesById.has(relationship.target_entity_id) &&
        (relationship.source_entity_id === selectedNodeId ||
          relationship.target_entity_id === selectedNodeId),
    );
  }, [graph.relationships, nodesById, selectedNodeId]);
  const supportingDocumentCount = useMemo(() => {
    if (!selectedNodeId) return 0;
    const evidenceIds = new Set<string>();
    graph.relationships.forEach((relationship) => {
      const sourceMatches = relationship.source_entity_id === selectedNodeId;
      const targetMatches = relationship.target_entity_id === selectedNodeId;
      if (!sourceMatches && !targetMatches) return;
      if (relationship.provenance_document_id) {
        evidenceIds.add(relationship.provenance_document_id);
      }
      if (sourceMatches && isEvidenceType(relationship.target_entity_type)) {
        evidenceIds.add(relationship.target_entity_id);
      }
      if (targetMatches && isEvidenceType(relationship.source_entity_type)) {
        evidenceIds.add(relationship.source_entity_id);
      }
    });
    return evidenceIds.size;
  }, [graph.relationships, selectedNodeId]);
  const visibleDetails = selectedNode?.metadata
    ? Object.entries(selectedNode.metadata).filter(([key]) => !HIDDEN_DETAIL_KEYS.has(key))
    : [];
  const visibleNodeIds = useMemo(() => {
    const ids = new Set<string>();
    graph.nodes.forEach((node) => {
      const group = GROUPS.find((candidate) => candidate.types.includes(node.node_type));
      if (group && expandedGroupIds.has(group.id)) {
        ids.add(node.id);
      }
    });
    if (selectedNodeId && nodesById.has(selectedNodeId)) {
      ids.add(selectedNodeId);
      if (focusSelected) {
        connectedNodeIds.forEach((id) => ids.add(id));
      }
    }
    return ids;
  }, [connectedNodeIds, expandedGroupIds, focusSelected, graph.nodes, nodesById, selectedNodeId]);
  const visibleGraph = useMemo(() => {
    const nodes = graph.nodes.filter((node) => visibleNodeIds.has(node.id));
    const relationships = graph.relationships.filter(
      (relationship) =>
        visibleNodeIds.has(relationship.source_entity_id) &&
        visibleNodeIds.has(relationship.target_entity_id),
    );
    return { nodes, relationships };
  }, [graph.nodes, graph.relationships, visibleNodeIds]);
  const layout = useMemo(() => layoutGraph(visibleGraph), [visibleGraph]);

  useEffect(() => {
    if (initialMode) {
      setMode(initialMode);
    }
  }, [initialMode]);

  useEffect(() => {
    try {
      const rawState = window.localStorage.getItem(NAVIGATOR_STORAGE_KEY);
      if (rawState) {
        const saved = JSON.parse(rawState) as {
          expandedGroupIds?: string[];
          focusSelected?: boolean;
          selectedGroup?: string;
          selectedNodeId?: string | null;
          scrollLeft?: number;
          scrollTop?: number;
        };
        setExpandedGroupIds(new Set(saved.expandedGroupIds ?? []));
        setFocusSelected(saved.focusSelected ?? true);
        setSelectedGroup(saved.selectedGroup ?? "family");
        if (saved.selectedNodeId && nodesById.has(saved.selectedNodeId)) {
          setSelectedNodeId(saved.selectedNodeId);
        }
        window.requestAnimationFrame(() => {
          if (canvasRef.current) {
            canvasRef.current.scrollLeft = saved.scrollLeft ?? 0;
            canvasRef.current.scrollTop = saved.scrollTop ?? 0;
          }
        });
      }
    } catch {
      setExpandedGroupIds(new Set());
    } finally {
      setHasLoadedViewState(true);
    }
  }, [nodesById]);

  useEffect(() => {
    if (!hasLoadedViewState) return;
    const canvas = canvasRef.current;
    window.localStorage.setItem(
      NAVIGATOR_STORAGE_KEY,
      JSON.stringify({
        expandedGroupIds: Array.from(expandedGroupIds),
        focusSelected,
        selectedGroup,
        selectedNodeId,
        scrollLeft: canvas?.scrollLeft ?? 0,
        scrollTop: canvas?.scrollTop ?? 0,
      }),
    );
  }, [expandedGroupIds, focusSelected, hasLoadedViewState, selectedGroup, selectedNodeId]);

  function focusNode(nodeId: string) {
    setSelectedNodeId(nodeId);
    setMode("none");
    const node = layout.positionedNodes.get(nodeId);
    const canvas = canvasRef.current;
    if (!node || !canvas) return;
    canvas.scrollTo({
      left: Math.max(0, node.x - canvas.clientWidth / 2 + NODE_WIDTH / 2),
      top: Math.max(0, node.y - canvas.clientHeight / 2 + NODE_HEIGHT / 2),
      behavior: "smooth",
    });
  }

  function fitView() {
    const canvas = canvasRef.current;
    if (!canvas) return;
    canvas.scrollTo({ left: 0, top: 0, behavior: "smooth" });
  }

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
      let detail = "Luna could not save this change.";
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
        error: error instanceof Error ? error.message : "Luna could not save this change.",
        loading: false,
        success: null,
      });
    }
  }

  async function createEntity(formData: FormData) {
    await withState(async () => {
      const entityType = String(formData.get("entity_type") ?? createType);
      await request("/api/household/entities", {
        body: JSON.stringify({
          display_name: String(formData.get("display_name") ?? ""),
          entity_type: entityType,
          metadata: detailsFromForm(formData, entityType),
        }),
        method: "POST",
      });
    }, "Added to your household.");
  }

  async function updateEntity(entityId: string, formData: FormData) {
    await withState(async () => {
      const entityType = String(formData.get("entity_type") ?? selectedNode?.node_type ?? "");
      await request(`/api/household/entities/${entityId}`, {
        body: JSON.stringify({
          display_name: String(formData.get("display_name") ?? ""),
          entity_type: entityType,
          metadata: detailsFromForm(formData, entityType),
        }),
        method: "PATCH",
      });
    }, "Details updated.");
  }

  async function createRelationship(formData: FormData) {
    await withState(async () => {
      const sourceEntityId = String(formData.get("source_entity_id") ?? "");
      const targetEntityId = String(formData.get("target_entity_id") ?? "");
      const source = nodesById.get(sourceEntityId);
      const target = nodesById.get(targetEntityId);
      if (sourceEntityId === targetEntityId) throw new Error("Choose two different items to link.");
      if (!source || !target) throw new Error("Choose both items to link.");
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
    }, "Items linked.");
  }

  async function updateRelationship(relationshipId: string, formData: FormData) {
    await withState(async () => {
      const sourceEntityId = String(formData.get("source_entity_id") ?? "");
      const targetEntityId = String(formData.get("target_entity_id") ?? "");
      const source = nodesById.get(sourceEntityId);
      const target = nodesById.get(targetEntityId);
      if (sourceEntityId === targetEntityId) throw new Error("Choose two different items to link.");
      if (!source || !target) throw new Error("Choose both items to link.");
      await request(`/api/household/relationships/${relationshipId}`, {
        body: JSON.stringify({
          relationship_type: String(formData.get("relationship_type") ?? ""),
          source_entity_id: sourceEntityId,
          source_entity_type: source.node_type,
          target_entity_id: targetEntityId,
          target_entity_type: target.node_type,
        }),
        method: "PATCH",
      });
      setEditingRelationshipId(null);
    }, "Link updated.");
  }

  async function deleteRelationship(relationshipId: string) {
    if (!window.confirm("Remove this link from the household?")) return;
    await withState(async () => {
      await request(`/api/household/relationships/${relationshipId}`, { method: "DELETE" });
    }, "Link removed.");
  }

  async function deleteEntity(entityId: string) {
    if (!window.confirm("Delete this household item? Luna will ask you to remove links first if this item is still connected.")) return;
    await withState(async () => {
      await request(`/api/household/entities/${entityId}`, { method: "DELETE" });
      if (selectedNodeId === entityId) {
        setSelectedNodeId(null);
      }
    }, "Household item deleted.");
  }

  async function decideSuggestion(suggestionId: string, decision: "accept" | "reject") {
    await withState(async () => {
      await request(`/api/household/suggestions/${suggestionId}/${decision}`, { method: "POST" });
    }, decision === "accept" ? "Luna updated the household." : "Luna will not suggest that again.");
    if (expandedSuggestionId === suggestionId) setExpandedSuggestionId(null);
  }

  function openRelationshipFromSelected() {
    if (selectedNode) {
      setRelationshipType("related_to");
      setMode("createRelationship");
    }
  }

  function toggleGroup(groupId: string) {
    setSelectedGroup(groupId);
    setExpandedGroupIds((current) => {
      const next = new Set(current);
      if (next.has(groupId)) {
        next.delete(groupId);
      } else {
        next.add(groupId);
      }
      return next;
    });
  }

  function expandAll() {
    setExpandedGroupIds(new Set(GROUPS.map((group) => group.id)));
  }

  function collapseAll() {
    setExpandedGroupIds(new Set());
    setSelectedGroup("family");
    setSelectedNodeId(null);
  }

  return (
    <section className="structureWorkspace" aria-label="Household map">
      <div className="structureActionBar">
        <div>
          <h2>Household map</h2>
          <span>{graph.nodes.length} household items in Luna's memory</span>
        </div>
        <div className="structureToolbar">
          <button type="button" onClick={() => setMode("createEntity")}>
            Add household item
          </button>
          <button disabled={graph.nodes.length < 2} onClick={() => setMode("createRelationship")} type="button">
            Link items
          </button>
          <button type="button" onClick={expandAll}>
            Expand All
          </button>
          <button type="button" onClick={collapseAll}>
            Collapse All
          </button>
          <button type="button" onClick={fitView}>
            Fit view
          </button>
          <button type="button" onClick={() => setFocusSelected(!focusSelected)}>
            {focusSelected ? "Show entire household" : "Focus on selected"}
          </button>
        </div>
      </div>

      <div className="groupFilterBar" aria-label="Household groups">
        {GROUPS.map((group) => {
          const count = graph.nodes.filter((node) => group.types.includes(node.node_type)).length;
          const isExpanded = expandedGroupIds.has(group.id);
          return (
            <button
              className={`${selectedGroup === group.id ? "activeGroupChip" : ""} ${isExpanded ? "expandedGroupChip" : ""}`}
              key={group.id}
              onClick={() => toggleGroup(group.id)}
              type="button"
            >
              {group.label}
              <span>{count}</span>
              <small>{isExpanded ? "expanded" : "collapsed"}</small>
            </button>
          );
        })}
      </div>
      {expandedGroupIds.size === 0 ? (
        <p className="navigatorGroupHint">
          Choose a group to open that part of the household, or use Expand All when you want the full map.
        </p>
      ) : null}

      {mode === "createEntity" ? (
        <form action={createEntity} className="structureQuickForm">
          <label>
            <span>Type</span>
            <select
              name="entity_type"
              onChange={(event) => setCreateType(event.target.value)}
              value={createType}
            >
              {ENTITY_TYPE_PRESETS.map((entityType) => (
                <option key={entityType} value={entityType}>
                  {titleCase(entityType)}
                </option>
              ))}
            </select>
          </label>
          <label>
            <span>{nameLabelFor(createType)}</span>
            <input name="display_name" placeholder="12 Smith Street" required type="text" />
          </label>
          {detailFieldsFor(createType).map((field) => (
            <label key={field.key}>
              <span>{field.label}</span>
              <input name={`detail_${field.key}`} placeholder={field.placeholder} type="text" />
            </label>
          ))}
          <div className="formButtonRow">
            <button disabled={state.loading} type="submit">Add</button>
            <button disabled={state.loading} onClick={() => setMode("none")} type="button">
              Cancel
            </button>
          </div>
        </form>
      ) : null}

      {mode === "createRelationship" ? (
        <form action={createRelationship} className="structureQuickForm">
          <label>
            <span>From</span>
            <select defaultValue={selectedNode?.id ?? ""} disabled={graph.nodes.length === 0} name="source_entity_id" required>
              <option value="">Choose item</option>
              {graph.nodes.map((node) => (
                <option key={node.id} value={node.id}>{entityOptionLabel(node)}</option>
              ))}
            </select>
          </label>
          <label>
            <span>Link</span>
            <input list="relationship-examples" name="relationship_type" onChange={(event) => setRelationshipType(event.target.value)} required type="text" value={relationshipType} />
          </label>
          <label>
            <span>To</span>
            <select disabled={graph.nodes.length === 0} name="target_entity_id" required>
              <option value="">Choose item</option>
              {graph.nodes.map((node) => (
                <option key={node.id} value={node.id}>{entityOptionLabel(node)}</option>
              ))}
            </select>
          </label>
          <div className="formButtonRow">
            <button disabled={state.loading || graph.nodes.length < 2} type="submit">Link</button>
            <button disabled={state.loading} onClick={() => setMode("none")} type="button">Cancel</button>
          </div>
        </form>
      ) : null}

      <datalist id="relationship-examples">
        {RELATIONSHIP_EXAMPLES.map((example) => (
          <option key={example} value={example} />
        ))}
      </datalist>

      {state.error ? <p className="formMessage errorMessage" role="alert">{state.error}</p> : null}
      {state.success ? <p className="formMessage successMessage">{state.success}</p> : null}

      <div className="structureTreeLayout">
        <div className="treeCanvas" aria-label="Household map" ref={canvasRef}>
          {graph.nodes.length === 0 ? (
            <div className="treeEmptyState">
              <strong>Add your first item to start building the household map.</strong>
              <span>Luna can also find people, properties, vehicles, suppliers, and policies from documents you upload.</span>
            </div>
          ) : visibleGraph.nodes.length === 0 ? (
            <div className="treeEmptyState">
              <strong>Your household is grouped and minimized.</strong>
              <span>Open Family, Properties, Vehicles, Businesses, Utilities, Insurance, Suppliers, or Documents to reveal items.</span>
            </div>
          ) : (
            <div className="treeViewport" style={{ height: layout.height, width: layout.width }}>
              <svg aria-hidden="true" className="relationshipLayer" height={layout.height} viewBox={`0 0 ${layout.width} ${layout.height}`} width={layout.width}>
                <defs>
                  <marker id="tree-arrow" markerHeight="8" markerWidth="8" orient="auto" refX="7" refY="4" viewBox="0 0 8 8">
                    <path d="M0,0 L8,4 L0,8 Z" fill="#9aa9a3" />
                  </marker>
                </defs>
                {visibleGraph.relationships.map((relationship) => {
                  const source = layout.positionedNodes.get(relationship.source_entity_id);
                  const target = layout.positionedNodes.get(relationship.target_entity_id);
                  if (!source || !target) return null;
                  const startX = source.x + NODE_WIDTH / 2;
                  const startY = source.y + NODE_HEIGHT;
                  const endX = target.x + NODE_WIDTH / 2;
                  const endY = target.y;
                  const midY = startY + (endY - startY) / 2;
                  const labelX = startX + (endX - startX) / 2;
                  const labelY = midY - 8;
                  const path = `M ${startX} ${startY} C ${startX} ${midY}, ${endX} ${midY}, ${endX} ${endY}`;
                  const dimmed =
                    focusSelected && selectedNodeId && !connectedNodeIds.has(source.id) && !connectedNodeIds.has(target.id);
                  return (
                    <g className={dimmed ? "dimmedRelationship" : ""} key={relationship.id}>
                      <path className="relationshipPath" d={path} markerEnd="url(#tree-arrow)" />
                      <foreignObject height="30" width="140" x={labelX - 70} y={labelY}>
                        <div className="relationshipLabel">{normalizedLabel(relationship.relationship_type)}</div>
                      </foreignObject>
                    </g>
                  );
                })}
              </svg>

              {Array.from(layout.positionedNodes.values()).map((node) => {
                const dimmed = focusSelected && selectedNodeId && !connectedNodeIds.has(node.id);
                return (
                  <button
                    aria-pressed={selectedNodeId === node.id}
                    className={`treeNode type-${node.node_type} ${selectedNodeId === node.id ? "selectedTreeNode" : ""} ${dimmed ? "dimmedTreeNode" : ""}`}
                    key={node.id}
                    onClick={() => focusNode(node.id)}
                    style={{ height: NODE_HEIGHT, left: node.x, top: node.y, width: NODE_WIDTH }}
                    type="button"
                  >
                    <span className="nodeIcon">{nodeInitial(node.node_type)}</span>
                    <span className="nodeText">
                      <strong>{node.display_name}</strong>
                      <span>{titleCase(node.node_type)}</span>
                    </span>
                  </button>
                );
              })}
            </div>
          )}
        </div>

        <aside className="entityDetailsPanel" aria-label="Household item details">
          <section className="suggestionsPanel" aria-label="Luna prepared suggestions">
            <div className="suggestionsHeader">
              <div>
                <h3>Luna prepared</h3>
                <span>{suggestions.length} awaiting confirmation</span>
              </div>
            </div>
            {suggestions.length === 0 ? (
              <p className="emptyState">Upload documents and Luna will suggest household links and items here.</p>
            ) : (
              <div className="suggestionList">
                {suggestions.map((suggestion) => (
                  <article className="suggestionCard" key={suggestion.id}>
                    <div>
                      <strong>{suggestionTitle(suggestion)}</strong>
                      <span>{confidenceLabel(suggestion.confidence)}</span>
                    </div>
                    <p>{suggestion.reasoning}</p>
                    {expandedSuggestionId === suggestion.id ? (
                      <details className="advancedDetails" open>
                        <summary>Advanced details</summary>
                        <code>{JSON.stringify(suggestion.action_payload, null, 2)}</code>
                      </details>
                    ) : null}
                    <div className="suggestionActions">
                      <button disabled={state.loading} onClick={() => decideSuggestion(suggestion.id, "accept")} type="button">Accept</button>
                      <button disabled={state.loading} onClick={() => decideSuggestion(suggestion.id, "reject")} type="button">Reject</button>
                      <button disabled={state.loading} onClick={() => setExpandedSuggestionId(expandedSuggestionId === suggestion.id ? null : suggestion.id)} type="button">Details</button>
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
                  <span>{titleCase(selectedNode.node_type)}</span>
                </div>
              </div>

              <div className="detailsActions">
                {selectedNode.node_type !== "document" && selectedNode.node_type !== "bill" ? (
                  <>
                    <button type="button" onClick={() => setMode("editEntity")}>Edit item</button>
                    <button className="dangerButton" disabled={state.loading} onClick={() => deleteEntity(selectedNode.id)} type="button">Delete item</button>
                  </>
                ) : null}
                <button type="button" onClick={openRelationshipFromSelected}>Link another item</button>
              </div>

              {mode === "editEntity" && selectedNode.node_type !== "document" && selectedNode.node_type !== "bill" ? (
                <form action={(formData) => updateEntity(selectedNode.id, formData)} className="detailsEditForm">
                  <label>
                    <span>Type</span>
                    <select defaultValue={selectedNode.node_type} name="entity_type">
                      {ENTITY_TYPE_PRESETS.map((entityType) => (
                        <option key={entityType} value={entityType}>{titleCase(entityType)}</option>
                      ))}
                    </select>
                  </label>
                  <label>
                    <span>{nameLabelFor(selectedNode.node_type)}</span>
                    <input defaultValue={selectedNode.display_name} name="display_name" required type="text" />
                  </label>
                  {detailFieldsFor(selectedNode.node_type).map((field) => (
                    <label key={field.key}>
                      <span>{field.label}</span>
                      <input defaultValue={detailValue(selectedNode.metadata, field.key)} name={`detail_${field.key}`} type="text" />
                    </label>
                  ))}
                  <div className="formButtonRow">
                    <button disabled={state.loading} type="submit">Save</button>
                    <button disabled={state.loading} onClick={() => setMode("none")} type="button">Cancel</button>
                  </div>
                </form>
              ) : null}

              <section className="detailSection">
                <h3>Details</h3>
                {visibleDetails.length > 0 ? (
                  <div className="detailsList">
                    {visibleDetails.map(([key, value]) => (
                      <div key={key}>
                        <span>{titleCase(key)}</span>
                        <strong>{typeof value === "object" ? JSON.stringify(value) : String(value)}</strong>
                      </div>
                    ))}
                  </div>
                ) : (
                  <p>No details yet.</p>
                )}
              </section>

              <section className="detailSection">
                <h3>Linked items</h3>
                {selectedRelationships.length === 0 ? (
                  <p>No linked items yet.</p>
                ) : (
                  <div className="detailRelationshipList">
                    {selectedRelationships.map((relationship) => {
                      const source = nodesById.get(relationship.source_entity_id);
                      const target = nodesById.get(relationship.target_entity_id);
                      return (
                        <div key={relationship.id} className="detailRelationshipRow">
                          {editingRelationshipId === relationship.id ? (
                            <form action={(formData) => updateRelationship(relationship.id, formData)} className="linkEditForm">
                              <label>
                                <span>From</span>
                                <select defaultValue={relationship.source_entity_id} name="source_entity_id" required>
                                  {graph.nodes.map((node) => (
                                    <option key={node.id} value={node.id}>{entityOptionLabel(node)}</option>
                                  ))}
                                </select>
                              </label>
                              <label>
                                <span>Link</span>
                                <input defaultValue={relationship.relationship_type} list="relationship-examples" name="relationship_type" required type="text" />
                              </label>
                              <label>
                                <span>To</span>
                                <select defaultValue={relationship.target_entity_id} name="target_entity_id" required>
                                  {graph.nodes.map((node) => (
                                    <option key={node.id} value={node.id}>{entityOptionLabel(node)}</option>
                                  ))}
                                </select>
                              </label>
                              <div className="formButtonRow">
                                <button disabled={state.loading} type="submit">Save link</button>
                                <button disabled={state.loading} onClick={() => setEditingRelationshipId(null)} type="button">Cancel</button>
                              </div>
                            </form>
                          ) : (
                            <>
                              <span>
                                <strong>{source?.display_name ?? relationship.source_entity_type}</strong>{" "}
                                {normalizedLabel(relationship.relationship_type)}{" "}
                                <strong>{target?.display_name ?? relationship.target_entity_type}</strong>
                              </span>
                              <div className="linkActionButtons">
                                <button disabled={state.loading} onClick={() => setEditingRelationshipId(relationship.id)} type="button">Edit link</button>
                                <button disabled={state.loading} onClick={() => deleteRelationship(relationship.id)} type="button">Remove link</button>
                              </div>
                            </>
                          )}
                        </div>
                      );
                    })}
                  </div>
                )}
              </section>

              <section className="detailSection">
                <h3>Supporting records</h3>
                {supportingDocumentCount === 0 ? (
                  <p>No linked records yet.</p>
                ) : (
                  <div className="linkedRecordList">
                    <span>
                      {supportingDocumentCount} supporting document
                      {supportingDocumentCount === 1 ? "" : "s"} connected through evidence.
                    </span>
                  </div>
                )}
              </section>
            </>
          ) : (
            <p className="emptyState">Select an item to see details, links, and documents.</p>
          )}
        </aside>
      </div>
    </section>
  );
}
