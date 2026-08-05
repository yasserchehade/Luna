import {
  TodayServiceError,
  type AttachmentResult,
  type ConversationInput,
  type ConversationResult,
  type FactCorrectionInput,
  type HouseholdWorkView,
  type MutationResult,
  type TodayBriefing,
  type TodayService,
} from "./contracts";

const MAX_SOURCE_BYTES = 5 * 1024 * 1024;
const SUPPORTED_SOURCE_TYPES = new Set(["application/pdf", "image/jpeg", "image/png"]);

type HttpTodayServiceOptions = {
  baseUrl?: string;
};

type ErrorEnvelope = {
  error?: { category?: string; message?: string };
};

function sizeLabel(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${Math.ceil(bytes / 1024)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

async function responseJson<T>(response: Response): Promise<T> {
  const body = await response.json().catch(() => ({})) as T & ErrorEnvelope;
  if (response.ok) return body;
  const category = body.error?.category;
  const code = category === "notFound"
    ? "notFound"
    : category === "unsupportedSource" || category === "sourceTooLarge" || category === "invalidInput"
      ? "invalidAttachment"
      : response.status >= 500
        ? "unavailable"
        : "mutationFailed";
  const fallback = code === "unavailable"
    ? "Luna is temporarily unavailable. Your Household Work is safe."
    : "Luna could not safely save that change.";
  throw new TodayServiceError(code, body.error?.message ?? fallback);
}

export function createHttpTodayService(options: HttpTodayServiceOptions = {}): TodayService {
  const baseUrl = (options.baseUrl ?? "/api/luna").replace(/\/$/, "");
  const request = async <T>(path: string, init?: RequestInit): Promise<T> => {
    try {
      return await responseJson<T>(await fetch(`${baseUrl}${path}`, init));
    } catch (error) {
      if (error instanceof TodayServiceError) throw error;
      throw new TodayServiceError("unavailable", "Luna is temporarily unavailable. Your Household Work is safe.");
    }
  };

  return {
    getBriefing: () => request<TodayBriefing>("/today"),
    getWorkItem: (id: string) => request<HouseholdWorkView>(`/household-work/${encodeURIComponent(id)}`),
    sendMessage: (input: ConversationInput) => request<ConversationResult>("/conversation", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        message: input.message,
        contextualWorkIds: input.contextualWorkIds ?? [],
        ...(input.attachmentId ? { sourceId: input.attachmentId } : {}),
      }),
    }),
    approveAction: (workId: string, actionId: string) => request<MutationResult>(
      `/household-work/${encodeURIComponent(workId)}/approve/${encodeURIComponent(actionId)}`,
      { method: "POST" },
    ),
    dismissWork: (workId: string) => request<MutationResult>(
      `/household-work/${encodeURIComponent(workId)}/dismiss`,
      { method: "POST" },
    ),
    completeWork: (workId: string) => request<MutationResult>(
      `/household-work/${encodeURIComponent(workId)}/complete`,
      { method: "POST" },
    ),
    correctFact: (input: FactCorrectionInput) => request<MutationResult>(
      `/household-work/${encodeURIComponent(input.workId)}/facts`,
      {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ factKey: input.factKey, value: input.value }),
      },
    ),
    async attachSource(file: File): Promise<AttachmentResult> {
      if (!SUPPORTED_SOURCE_TYPES.has(file.type)) {
        throw new TodayServiceError("invalidAttachment", "Choose a PDF, JPG or PNG household document.");
      }
      if (file.size > MAX_SOURCE_BYTES) {
        throw new TodayServiceError("invalidAttachment", "That document is larger than the 5 MB source limit.");
      }
      const form = new FormData();
      form.append("source", file, file.name);
      const uploaded = await request<{ sourceId: string; displayName: string; sizeBytes: number }>("/sources", {
        method: "POST",
        body: form,
      });
      return {
        attachmentId: uploaded.sourceId,
        displayName: uploaded.displayName,
        sizeLabel: sizeLabel(uploaded.sizeBytes),
        persisted: true,
      };
    },
  };
}
