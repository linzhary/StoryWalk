import { invoke } from "@tauri-apps/api/core";
import type { Story, StorySession, StoryMessage, StoryCard, StoryMaterials } from "./mock-data";

export interface BreadcrumbItem {
  label: string;
  href: string;
}

// ---- Stories ----

export async function getStories(): Promise<Story[]> {
  return invoke("get_stories");
}

export async function getStory(id: string): Promise<Story> {
  return invoke("get_story", { storyId: id });
}

export async function createStory(data: { title: string; description?: string; style?: "modern" | "ancient"; mode?: "card" | "chat" }): Promise<Story> {
  return invoke("create_story", { title: data.title, description: data.description || "", style: data.style || "modern", mode: data.mode || "card" });
}

export async function updateStory(id: string, data: Partial<{ title: string; description: string }>): Promise<Story> {
  return invoke("update_story", { storyId: id, title: data.title, description: data.description });
}

export async function deleteStory(id: string): Promise<void> {
  return invoke("delete_story", { storyId: id });
}

// ---- Sessions ----

export async function getSessions(storyId: string): Promise<StorySession[]> {
  return invoke("get_sessions", { storyId });
}

export async function createSession(storyId: string, data: { title?: string; mode?: string; model?: string }): Promise<StorySession> {
  return invoke("create_session", { storyId, title: data.title, mode: data.mode, model: data.model });
}

export async function updateSession(id: string, data: Partial<{ title: string; model: string }>): Promise<StorySession> {
  return invoke("update_session", { sessionId: id, title: data.title, model: data.model });
}

export async function deleteSession(id: string): Promise<void> {
  return invoke("delete_session", { sessionId: id });
}

export async function chat(sessionId: string, message: string, model: string): Promise<void> {
  return invoke("chat", { sessionId, message, model });
}

export async function summarizeSession(id: string): Promise<string> {
  return invoke("summarize_session", { sessionId: id });
}

// ---- Materials ----

export async function readMaterials(storyId: string): Promise<StoryMaterials> {
  // Read both MD files
  const [referenceMd, guidelinesMd] = await Promise.all([
    invoke<string>("read_story_materials", { storyId, file: "reference" }),
    invoke<string>("read_story_materials", { storyId, file: "guidelines" }),
  ]);
  return { referenceMd, guidelinesMd };
}

export async function updateMaterials(storyId: string, file: string, content: string): Promise<void> {
  return invoke("update_story_materials", { storyId, file, content });
}

/** 纯聊模式手动触发素材沉淀（写卡模式由回复完成后自动触发，无需前端调用） */
export async function triggerMaterialExtraction(storyId: string, sessionId: string): Promise<void> {
  return invoke("trigger_material_extraction", { storyId, sessionId });
}

// ---- Story Cards ----

export async function getStoryCards(storyId: string): Promise<StoryCard[]> {
  return invoke("get_story_cards", { storyId });
}

export async function saveStoryCard(storyId: string, sessionId: string, content: string): Promise<StoryCard> {
  return invoke("save_story_card", { storyId, sessionId, content });
}

export async function updateStoryCard(cardId: string, content: string): Promise<StoryCard> {
  return invoke("update_story_card", { cardId, content });
}

export async function deleteStoryCard(cardId: string): Promise<void> {
  return invoke("delete_story_card", { cardId });
}

// ---- Messages ----

export async function getMessages(sessionId: string): Promise<StoryMessage[]> {
  return invoke("get_messages", { sessionId });
}

export async function getMessageCount(sessionId: string): Promise<number> {
  return invoke("get_message_count", { sessionId });
}

export async function getMessagesPaginated(sessionId: string, beforeId?: string, limit?: number): Promise<StoryMessage[]> {
  return invoke("get_messages_paginated", { sessionId, beforeId: beforeId || null, limit: limit || null });
}

export async function saveMessage(sessionId: string, data: { role: string; content: string; reasoning?: string; toolCallId?: string; phase?: string }): Promise<StoryMessage> {
  return invoke("save_message", {
    sessionId,
    role: data.role,
    content: data.content,
    reasoning: data.reasoning || null,
    toolCallId: data.toolCallId || null,
    phase: data.phase || null,
  });
}

export async function rollbackMessages(sessionId: string, messageId: string): Promise<StoryMessage[]> {
  return invoke("rollback_messages", { sessionId, messageId });
}

export async function deleteMessage(id: string): Promise<void> {
  return invoke("delete_message", { messageId: id });
}

// ---- Breadcrumbs ----

export async function getBreadcrumbStory(id: string): Promise<BreadcrumbItem> {
  return invoke("get_breadcrumb_story", { storyId: id });
}
