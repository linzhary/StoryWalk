// TypeScript interfaces shared between components and API

export interface Story {
  id: string;
  title: string;
  description: string;
  /** 故事模式：card（写卡，正文沉淀为剧情卡片）/ chat（纯聊，正文直接回复在聊天框） */
  mode: "card" | "chat";
  createdAt: string;
  updatedAt: string;
}

export interface StorySession {
  id: string;
  storyId: string;
  title: string;
  mode: "creation" | "settings";
  model: "deepseek-v4-flash" | "deepseek-v4-pro";
  summary: string;
  createdAt: string;
}

export interface StoryMessage {
  id: string;
  sessionId: string;
  role: "user" | "assistant" | "tool";
  content: string;
  reasoning?: string;
  toolCallId?: string | null;
  phase?: "materials" | "creation";
  createdAt: string;
}

export interface StoryCard {
  id: string;
  storyId: string;
  sessionId: string;
  content: string;
  roundNumber: number;
  createdAt: string;
}

export interface StoryMaterials {
  referenceMd: string;
  guidelinesMd: string;
}
