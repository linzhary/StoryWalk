import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface ChatEvent {
  type: "reasoning" | "text" | "tool_call_start" | "tool_execute_start" | "tool_call_end" | "done";
  content: string;
  phase?: "materials" | "creation";
}

export interface ToolCallStartPayload {
  name: string;
  args: Record<string, unknown>;
}

export interface ToolCallEndPayload {
  name: string;
  result: Record<string, unknown>;
}

export type ChatEventCallback = (event: {
  type: ChatEvent["type"];
  content: string;
  phase?: "materials" | "creation";
  toolCall?: ToolCallStartPayload | ToolCallEndPayload;
}) => void;

export function listenChatEvents(callback: ChatEventCallback): Promise<UnlistenFn> {
  return listen<ChatEvent>("chat-event", (event) => {
    const { type, content, phase } = event.payload;

    if (type === "tool_call_start" || type === "tool_execute_start" || type === "tool_call_end") {
      try {
        const parsed = JSON.parse(content);
        callback({ type, content: "", phase, toolCall: parsed });
      } catch {
        callback({ type, content, phase, toolCall: undefined });
      }
    } else {
      callback({ type, content, phase, toolCall: undefined });
    }
  });
}

export function listenMaterialUpdated(callback: () => void): Promise<UnlistenFn> {
  return listen("material_updated", () => callback());
}

export interface MaterialExtractionStatus {
  storyId: string;
  status: "start" | "done" | "failed";
}

/** 后台素材提取状态（写作回复完成后触发） */
export function listenMaterialExtraction(callback: (payload: MaterialExtractionStatus) => void): Promise<UnlistenFn> {
  return listen<MaterialExtractionStatus>("material_extraction", (event) => callback(event.payload));
}

export function listenCardSaved(callback: () => void): Promise<UnlistenFn> {
  return listen("card_saved", () => callback());
}
