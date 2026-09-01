import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useParams } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { Loader2, RotateCcw, ChevronRight, Archive } from "lucide-react";

function CollapsibleThinking({ content, isStreaming }: { content: string; isStreaming?: boolean }) {
  const [open, setOpen] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  useEffect(() => { setOpen(!!isStreaming); }, [isStreaming]);
  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [content]);
  const label = isStreaming ? "思考中..." : "思考过程";
  return (
    <div className="my-2">
      <button onClick={() => setOpen(!open)} className="flex items-center gap-1.5 text-[11px] text-muted-foreground hover:text-foreground transition-colors select-none">
        <ChevronRight className={`h-3 w-3 transition-transform duration-200 ${open ? "rotate-90" : ""}`} />
        <span>{label}</span>
        {isStreaming && <span className="w-1.5 h-1.5 rounded-full bg-primary animate-pulse" />}
      </button>
      <div className="grid" style={{ gridTemplateRows: open ? "1fr" : "0fr" }}>
        <div className="overflow-hidden">
          <div ref={scrollRef} className="mt-1.5 text-xs text-muted-foreground leading-relaxed whitespace-pre-wrap max-h-[300px] overflow-y-auto">{content}</div>
        </div>
      </div>
    </div>
  );
}

function ToolCallCard({ name, target, args, status, error }: { name: string; target?: string; args?: { file?: string }; status: "generating" | "running" | "done" | "error"; error?: string }) {
  const toolMap: Record<string, { label: string; color: string }> = {
    read_story_md: { label: "读取", color: "text-muted-foreground/60" },
    patch_story_md: { label: "编辑", color: "text-[#60a5fa]" },
    update_story_md: { label: "重写", color: "text-[#52ad5a]" },
    save_story_card: { label: "剧情卡片", color: "text-[#c9983c]" },
    read_story_cards: { label: "阅读剧情", color: "text-muted-foreground/60" },
    read_story_card: { label: "读取卡片", color: "text-muted-foreground/60" },
    update_story_card: { label: "更新卡片", color: "text-[#60a5fa]" },
  };
  const isError = status === "error";
  const isDone = status === "done";
  const info = toolMap[name] || { label: name, color: "text-muted-foreground/60" };
  const rawTarget = target || args?.file;
  const fileLabelMap: Record<string, string> = { reference: "参考资料", guidelines: "创作准则" };
  const displayTarget = rawTarget ? (fileLabelMap[rawTarget] || rawTarget) : undefined;
  const borderClass = isError
    ? "border-[#e5484d]/40 hover:border-[#e5484d]/60"
    : isDone
      ? "border-[#52ad5a]/40 hover:border-[#52ad5a]/60"
      : "";
  return (
    <div className={`group flex items-stretch my-[6px] rounded-lg border bg-card transition-colors duration-180 overflow-hidden ${borderClass}`}>
      <span className={`relative flex items-center justify-center shrink-0 w-[34px] ${isError ? "text-[#e5484d]" : info.color}`}>
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
          {name === "patch_story_md" && <path d="M12 2L14 4L10 8V10H12L16 6L14 4L10 8V10H12Z" />}
          {name === "update_story_card" && <path d="M12 2L14 4L10 8V10H12L16 6L14 4L10 8V10H12Z" />}
          {(name === "update_story_md" || !toolMap[name]) && <><rect x="3" y="4" width="10" height="9" rx="1.5" /><path d="M5 2V4M11 2V4" /></>}
          {(name === "read_story_md" || name === "read_story_card") && <><circle cx="6" cy="6" r="1.5" /><circle cx="6" cy="10" r="1.5" /><circle cx="6" cy="14" r="1.5" /></>}
          {name === "read_story_cards" && <><rect x="3" y="3" width="10" height="10" rx="1.5" /><path d="M6 6.5L7.5 8L10 5.5" /></>}
          {name === "save_story_card" && <><rect x="3" y="4" width="10" height="9" rx="1.5" /><path d="M5 2V4M11 2V4" /><path d="M5.5 8.5L7.5 10.5L11 6.5" /></>}
        </svg>
        <span className="absolute right-0 top-[7px] bottom-[7px] w-px bg-border" />
      </span>
      <div className="flex-1 flex items-center gap-1.5 min-w-0 px-3 py-[7px] text-[13px] leading-[1.45] font-medium">
        <span className="text-foreground/80 shrink-0">{info.label}</span>
        {isError && error ? (
          <span className="text-[#e5484d]/90 truncate" title={error}>失败：{error}</span>
        ) : displayTarget ? (
          <span className="text-muted-foreground/70 truncate">{displayTarget}</span>
        ) : null}
      </div>
      <span className="flex items-center justify-center shrink-0 w-[34px] border-l border-border">
        {status === "generating" ? (
          <span className="flex items-center gap-[3px]" style={{ color: "#c9983c" }}>
            <span className="w-[3px] h-[3px] rounded-full bg-current animate-pulse" />
            <span className="w-[3px] h-[3px] rounded-full bg-current animate-pulse" style={{ animationDelay: "0.2s" }} />
            <span className="w-[3px] h-[3px] rounded-full bg-current animate-pulse" style={{ animationDelay: "0.4s" }} />
          </span>
        ) : status === "running" ? (
          <Loader2 className="h-3.5 w-3.5 animate-spin" style={{ color: "#2e5e8a" }} />
        ) : isError ? (
          <span className="text-[#e5484d]"><svg width="14" height="14" viewBox="0 0 14 14" fill="none"><path d="M3.5 3.5L10.5 10.5M10.5 3.5L3.5 10.5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" /></svg></span>
        ) : (
          <span className="text-[#52ad5a]"><svg width="14" height="14" viewBox="0 0 14 14" fill="none"><path d="M3 7L6 10L11 4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" /></svg></span>
        )}
      </span>
    </div>
  );
}

import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import ChatInput from "@/components/chat-input";
import ScrollToBottom from "@/components/scroll-to-bottom";
import StoryCardsPanel from "@/components/story-cards-panel";
import { TaggedText, renderCardTags } from "@/components/card-tag-text";
import { Virtuoso, type Components } from "react-virtuoso";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { listenChatEvents, listenCardSaved, listenMaterialExtraction } from "@/lib/tauri-events";
import type { StoryCard } from "@/lib/mock-data";
import { getStory, getSessions, createSession, chat, updateSession, getMessagesPaginated, getMessageCount, saveMessage, rollbackMessages, getStoryCards, triggerMaterialExtraction } from "@/lib/api";

// Virtuoso list edge spacing (mirrors bitfun's message-list-header/footer approach)
const MESSAGE_LIST_COMPONENTS: Components = {
  Header: () => <div aria-hidden style={{ height: 16, flexShrink: 0 }} />,
  Footer: () => <div aria-hidden style={{ height: 28, flexShrink: 0 }} />,
};

const MD_COMPONENTS = {
  p: ({ children }: any) => <p className="mb-[0.28em] last:mb-0">{renderCardTags(children)}</p>,
  code: ({ className, children, ...props }: any) => {
    const isInline = !className;
    return isInline
      ? <code className="font-mono text-[0.88em] font-medium border rounded-[5px] px-[0.28em] py-[0.08em] mx-[0.03em] whitespace-nowrap transition-colors duration-150 [box-decoration-break:clone]" style={{ background: "var(--md-inline-code-bg)", borderColor: "var(--md-inline-code-border)" }} {...props}>{children}</code>
      : (
        <div className="my-[0.72em] border border-border/30 rounded-lg overflow-hidden">
          <div className="flex items-center justify-between min-h-[1.86rem] px-[0.62rem] py-[0.26rem] border-b border-border/30" style={{ background: "var(--md-code-toolbar-bg)" }}>
            <span className="text-[10px] font-semibold text-muted-foreground/60 uppercase tracking-wider select-none">{className?.replace("language-", "") || "code"}</span>
          </div>
          <pre className="m-0 p-[0.78rem_0.86rem] overflow-x-auto text-[13px] leading-[1.52] font-mono" style={{ background: "var(--md-code-bg)" }}><code>{children}</code></pre>
        </div>
      );
  },
  blockquote: ({ children }: any) => <blockquote className="my-[0.7em] pl-[0.72rem] pr-[0.72rem] py-[0.58rem] border-l-[3px] rounded-r-[6px] text-muted-foreground" style={{ background: "var(--md-blockquote-bg)", borderLeftColor: "var(--md-blockquote-border)" }}>{renderCardTags(children)}</blockquote>,
  a: ({ href, children }: any) => <a href={href} className="text-primary underline decoration-1 underline-offset-2 hover:text-[#234a6d] hover:decoration-[1.5px]" target="_blank" rel="noreferrer">{children}</a>,
  ul: ({ children }: any) => <ul className="list-disc pl-[1.35rem] my-0 [&>li]:mt-[0.28em] [&>li]:leading-[1.58] first:mt-0">{children}</ul>,
  ol: ({ children }: any) => <ol className="list-decimal pl-[1.35rem] my-0 [&>li]:mt-[0.28em] [&>li]:leading-[1.58] first:mt-0">{children}</ol>,
  li: ({ children }: any) => <li>{renderCardTags(children)}</li>,
  table: ({ children }: any) => <div className="my-[0.86em] border rounded-lg overflow-auto" style={{ borderColor: "var(--md-table-border)", background: "var(--md-table-surface)", boxShadow: "var(--md-table-shadow)" }}><table className="w-full border-collapse border-spacing-0 text-[13px] leading-[1.52] [&_tbody_tr:nth-child(even)]:bg-[var(--md-table-row-stripe)] [&_tbody>tr:hover]:bg-[var(--md-table-row-hover)]">{children}</table></div>,
  thead: ({ children }: any) => <thead>{children}</thead>,
  th: ({ children }: any) => <th className="px-[0.78rem] py-[0.48rem] text-left text-[13px] font-semibold border-b first:border-l-0 border-l" style={{ background: "var(--md-table-header-bg)", color: "var(--md-table-header-fg)", borderBottomColor: "var(--md-table-cell-border)", borderLeftColor: "var(--md-table-col-divider)" }}>{children}</th>,
  td: ({ children }: any) => <td className="px-[0.78rem] py-[0.48rem] text-left align-top border-b first:border-l-0 border-l" style={{ borderBottomColor: "var(--md-table-cell-border)", borderLeftColor: "var(--md-table-col-divider)" }}>{renderCardTags(children)}</td>,
  tr: ({ children }: any) => <tr className="[&:last-child_td]:border-b-0">{children}</tr>,
  hr: () => <hr className="h-px my-[1.04em] border-0 rounded bg-border" />,
  strong: ({ children }: any) => <strong className="font-semibold">{children}</strong>,
  em: ({ children }: any) => <em className="text-muted-foreground italic">{children}</em>,
  h1: ({ children }: any) => <h1 className="text-[16px] font-semibold leading-[1.45] my-[0.86em_0_0.36em]">{renderCardTags(children)}</h1>,
  h2: ({ children }: any) => <h2 className="text-[15px] font-semibold leading-[1.45] my-[0.86em_0_0.36em]">{renderCardTags(children)}</h2>,
  h3: ({ children }: any) => <h3 className="text-[14px] font-semibold leading-[1.45] my-[0.86em_0_0.36em]">{renderCardTags(children)}</h3>,
  h4: ({ children }: any) => <h4 className="text-[14px] font-semibold leading-[1.45] text-muted-foreground my-[0.86em_0_0.36em]">{renderCardTags(children)}</h4>,
};

function summarizeToolResult(name: string, result: any): string {
  if (!result) return "";
  switch (name) {
    case "read_story_md": return result.file || "";
    case "patch_story_md": return result.file || "";
    case "update_story_md": return result.file || "";
    case "save_story_card": return result.roundNumber !== undefined ? `已保存（第 ${result.roundNumber} 轮）` : "已保存";
    case "read_story_cards": return result.count !== undefined ? `共 ${result.count} 张卡片` : "已读取";
    case "read_story_card":
      if (result.round === undefined) return "已读取";
      if (result.start !== undefined && result.end !== undefined) return `第 ${result.round} 轮 ${result.start}-${result.end} 字`;
      return `第 ${result.round} 轮`;
    case "update_story_card": return result.roundNumber !== undefined ? `已更新（第 ${result.roundNumber} 轮）` : "已更新";
    default: return JSON.stringify(result).substring(0, 80);
  }
}

interface ToolCallInfo { callId?: number; index?: number; name: string; args: any; status: "generating" | "running" | "done" | "error"; result?: any; resultSummary?: string; error?: string; }
interface TimelineEvent { type: "reasoning" | "tool_call" | "text" | "divider"; content: string; toolCall?: ToolCallInfo; }
interface ChatMessage { id: string; role: "user" | "assistant" | "tool"; content: string; reasoning?: string; confirmed?: boolean; }

export default function StoryPage() {
  const { id: storyId } = useParams<{ id: string }>();
  const [, setStory] = useState<{ id: string; title: string } | null>(null);
  const [currentSessionId, setCurrentSessionId] = useState<string>("");
  const [model, setModel] = useState<"deepseek-v4-flash" | "deepseek-v4-pro">("deepseek-v4-flash");
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [streaming, setStreaming] = useState(false);
  const [streamContent, setStreamContent] = useState("");
  const [streamReasoning, setStreamReasoning] = useState("");
  const [streamTimeline, setStreamTimeline] = useState<TimelineEvent[]>([]);
  const [cards, setCards] = useState<StoryCard[]>([]);
  const [loading, setLoading] = useState(true);
  const [rollbackTargetId, setRollbackTargetId] = useState<string | null>(null);
  const [expandedUserMsgId, setExpandedUserMsgId] = useState<string | null>(null);
  const [prefill, setPrefill] = useState("");
  const [prefillNonce, setPrefillNonce] = useState(0);
  const [extractionStatus, setExtractionStatus] = useState<"idle" | "running" | "done" | "failed">("idle");
  const extractionTimerRef = useRef<number | null>(null);
  const [storyMode, setStoryMode] = useState<"card" | "chat">("card");
  const [showScrollBtn, setShowScrollBtn] = useState(true);
  const [firstItemIndex, setFirstItemIndex] = useState(0);
  const [hasMoreMessages, setHasMoreMessages] = useState(false);
  const virtuosoRef = useRef<any>(null);
  const stopRef = useRef(false);
  const stopUnlistenRef = useRef<(() => void) | null>(null);
  const isLoadingMoreRef = useRef(false);
  const PAGE_LIMIT = 50;

  const handleModelChange = useCallback((newModel: "deepseek-v4-flash" | "deepseek-v4-pro") => {
    setModel(newModel);
    if (currentSessionId) { updateSession(currentSessionId, { model: newModel }).catch(console.error); }
  }, [currentSessionId]);

  const loadSessionMessages = useCallback(async (sessionId: string) => {
    const [totalCount, msgs] = await Promise.all([getMessageCount(sessionId), getMessagesPaginated(sessionId)]);
    const mapped = msgs.map((m: any) => ({ id: m.id, role: m.role, content: m.content, reasoning: m.reasoning || undefined, confirmed: true }));
    setMessages(mapped);
    setHasMoreMessages(msgs.length >= PAGE_LIMIT);
    setFirstItemIndex(Math.max(0, totalCount - msgs.length));
  }, []);

  const loadCards = useCallback(async (sid: string) => {
    try { setCards(await getStoryCards(sid)); } catch (e) { console.error(e); }
  }, []);

  useEffect(() => {
    if (!storyId) return;
    (async () => {
      try {
        const s = await getStory(storyId); setStory(s);
        setStoryMode(s.mode === "chat" ? "chat" : "card");
        const sessList = await getSessions(storyId);
        // Use the writing session (creation mode); create one if missing
        const writing = sessList.find(s => s.mode === "creation");
        if (writing) {
          setCurrentSessionId(writing.id);
          setModel((writing.model || "deepseek-v4-flash") as any);
          await Promise.all([loadSessionMessages(writing.id), loadCards(storyId)]);
        } else {
          const ns = await createSession(storyId, { title: "写作会话", mode: "creation", model: "deepseek-v4-flash" });
          setCurrentSessionId(ns.id); await loadCards(storyId);
        }
      } catch (e) { console.error(e); }
      setLoading(false);
    })();
  }, [storyId, loadCards, loadSessionMessages]);

  // Refresh story cards when the AI saves a new card via save_story_card
  useEffect(() => {
    if (!storyId) return;
    let unlisten: (() => void) | null = null;
    let cancelled = false;
    listenCardSaved(() => { if (!cancelled) loadCards(storyId); })
      .then(fn => { if (cancelled) fn(); else unlisten = fn; })
      .catch(console.error);
    return () => { cancelled = true; unlisten?.(); };
  }, [storyId, loadCards]);

  // Background material extraction status: show "素材沉淀中..." while running, then fade out
  useEffect(() => {
    if (!storyId) return;
    let unlisten: (() => void) | null = null;
    let cancelled = false;
    listenMaterialExtraction((payload) => {
      if (cancelled || payload.storyId !== storyId) return;
      if (payload.status === "start") {
        if (extractionTimerRef.current) window.clearTimeout(extractionTimerRef.current);
        setExtractionStatus("running");
      } else {
        setExtractionStatus(payload.status === "done" ? "done" : "failed");
        extractionTimerRef.current = window.setTimeout(() => setExtractionStatus("idle"), 3000);
      }
    })
      .then(fn => { if (cancelled) fn(); else unlisten = fn; })
      .catch(console.error);
    return () => {
      cancelled = true;
      unlisten?.();
      if (extractionTimerRef.current) window.clearTimeout(extractionTimerRef.current);
    };
  }, [storyId]);

  const virtuosoItems = useMemo(() => {
    const items: { type: string; key: string; [k: string]: any }[] = [];
    // Use round-based grouping (original settings mode style)
    let currentRound: ChatMessage[] = [];
    for (const msg of messages) {
      if (msg.role === "user") {
        if (currentRound.length > 0) {
          items.push({ type: "round", key: currentRound[0].id, msgs: [...currentRound] });
          currentRound = [];
        }
        items.push({ type: "user", key: msg.id, msg, isStreaming: streaming });
      } else {
        currentRound.push(msg);
      }
    }
    if (currentRound.length > 0) {
      items.push({ type: "round", key: currentRound[0].id, msgs: [...currentRound] });
    }
    if (streaming) {
      const lastMsg = messages[messages.length - 1];
      const alreadySaved = lastMsg?.role === "assistant" && lastMsg?.content === streamContent;
      if (!alreadySaved) {
        items.push({ type: "streaming", key: "__streaming__", reasoning: streamReasoning, content: streamContent, timelineLen: streamTimeline.length });
      }
    }
    return items;
  }, [messages, streaming, streamContent, streamReasoning, streamTimeline]);

  const handleRollbackConfirm = useCallback(async () => {
    if (!rollbackTargetId || !currentSessionId) return;
    const target = messages.find(m => m.id === rollbackTargetId);
    setRollbackTargetId(null);
    const prefillContent = target?.content || "";
    try { await rollbackMessages(currentSessionId, rollbackTargetId); await loadSessionMessages(currentSessionId); setPrefill(prefillContent); setPrefillNonce(n => n + 1); }
    catch (e) { console.error(e); }
  }, [rollbackTargetId, currentSessionId, loadSessionMessages]);

  // Quote a whole story card into the chat input as a compact tag: [第N轮]
  const handleQuoteCard = useCallback((roundNumber: number) => {
    setPrefill(`[第${roundNumber}轮]`);
    setPrefillNonce(n => n + 1);
  }, []);

  // Quote a fragment (char range) of a story card into the chat input: [第N轮:S-E]
  const handleQuoteFragment = useCallback((roundNumber: number, start: number, end: number) => {
    setPrefill(`[第${roundNumber}轮:${start}-${end}]`);
    setPrefillNonce(n => n + 1);
  }, []);

  // 纯聊模式：用户手动触发素材沉淀（状态经 material_extraction 事件展示「素材沉淀中...」）
  const handleManualExtraction = useCallback(async () => {
    if (!storyId || !currentSessionId) return;
    try {
      await triggerMaterialExtraction(storyId, currentSessionId);
    } catch (e) {
      console.error(e);
    }
  }, [storyId, currentSessionId]);

  const handleStop = useCallback(() => {
    stopRef.current = true;
    // Tell the backend to cancel the ongoing request
    invoke("stop_chat").catch(() => {});
    // Immediately detach the event listener so no more streaming events are processed
    if (stopUnlistenRef.current) {
      stopUnlistenRef.current();
      stopUnlistenRef.current = null;
    }
    setStreaming(false);
    setStreamContent("");
    setStreamReasoning("");
    setStreamTimeline([]);
  }, []);

  const handleSend = useCallback(async (message: string) => {
    if (!currentSessionId || !storyId) return;
    setPrefill(""); stopRef.current = false;
    const saved = await saveMessage(currentSessionId, { role: "user", content: message });
    setMessages(prev => [...prev, { id: saved.id, role: "user", content: message }]);
    setStreaming(true); setStreamContent(""); setStreamReasoning(""); setStreamTimeline([]);
    // 发送后主动滚动到底（追加消息后不依赖 followOutput）
    requestAnimationFrame(() => {
      virtuosoRef.current?.scrollToIndex({ index: "LAST", align: "end", behavior: "auto" });
    });
    let fullContent = ""; let fullReasoning = "";
    const toolCallsAcc: ToolCallInfo[] = [];
    let nextCallId = 0;

    const unlisten = await listenChatEvents((event) => {
      if (stopRef.current) return;
      if (event.type === "reasoning") {
        fullReasoning += event.content;
        setStreamReasoning(fullReasoning);
        setStreamTimeline(prev => {
          const last = prev[prev.length - 1];
          if (last?.type === "reasoning") {
            const updated = [...prev];
            updated[updated.length - 1] = { ...last, content: last.content + event.content };
            return updated;
          }
          return [...prev, { type: "reasoning", content: event.content }];
        });
      } else if (event.type === "text") {
        fullContent += event.content;
        setStreamContent(fullContent);
        setStreamTimeline(prev => {
          const last = prev[prev.length - 1];
          if (last?.type === "text") {
            const updated = [...prev];
            updated[updated.length - 1] = { ...last, content: last.content + event.content };
            return updated;
          }
          return [...prev, { type: "text", content: event.content }];
        });
      } else if (event.type === "tool_call_start") {
        const info = event.toolCall as any;
        const tc: ToolCallInfo = { callId: nextCallId++, index: info.index, name: info.name, args: info.args, status: "generating" };
        toolCallsAcc.push(tc);
        setStreamTimeline(prev => [...prev, { type: "tool_call", content: "", toolCall: tc }]);
      } else if (event.type === "tool_execute_start") {
        const info = event.toolCall as any;
        // Tool-call index restarts each round, so match by arrival order (status) instead of index
        const ex = toolCallsAcc.find(t => t.status === "generating");
        if (ex) {
          ex.status = "running"; ex.args = info.args;
          const cid = ex.callId;
          setStreamTimeline(prev => prev.map(ev => ev.type === "tool_call" && ev.toolCall?.callId === cid ? { ...ev, toolCall: { ...ev.toolCall!, status: "running" as const, args: info.args } } : ev));
        }
      } else if (event.type === "tool_call_end") {
        const info = event.toolCall as any;
        const idx = toolCallsAcc.findIndex(t => t.status === "running");
        const hasError = !!(info.result && typeof info.result === "object" && info.result.error !== undefined);
        const s = hasError ? undefined : summarizeToolResult(info.name, info.result);
        const errText = hasError ? String(info.result.error) : undefined;
        let cid: number | undefined;
        if (idx >= 0) {
          const ex = toolCallsAcc[idx];
          cid = ex.callId;
          ex.status = hasError ? "error" : "done";
          ex.result = info.result;
          ex.resultSummary = s;
          ex.error = errText;
          toolCallsAcc.splice(idx, 1);
        }
        setStreamTimeline(prev => {
          const u = prev.map(ev => ev.type === "tool_call" && cid !== undefined && ev.toolCall?.callId === cid ? { ...ev, toolCall: { ...ev.toolCall!, status: (hasError ? "error" : "done") as "error" | "done", result: info.result, resultSummary: s, error: errText } } : ev);
          return [...u, { type: "divider" as const, content: "" }];
        });
      }
    });
    stopUnlistenRef.current = unlisten;

    try { await chat(currentSessionId, message, model); } catch (e) { console.error(e); }
    // Detach listener (may already be detached by handleStop)
    if (stopUnlistenRef.current) {
      unlisten();
      stopUnlistenRef.current = null;
    }

    if (stopRef.current) {
      // User cancelled — don't load the completed backend response
      setStreaming(false); setStreamContent(""); setStreamReasoning(""); setStreamTimeline([]);
      return;
    }

    // Reload messages from DB to get all persisted messages (including tool calls)
    await loadSessionMessages(currentSessionId);
    setStreaming(false); setStreamContent(""); setStreamReasoning(""); setStreamTimeline([]);
    // 回复完成后滚动到底（数据替换后不依赖 followOutput）
    requestAnimationFrame(() => {
      virtuosoRef.current?.scrollToIndex({ index: "LAST", align: "end", behavior: "auto" });
    });
  }, [currentSessionId, model, loadSessionMessages]);

  const scrollToBottom = useCallback(() => { virtuosoRef.current?.scrollToIndex({ index: virtuosoItems.length - 1, align: "end", behavior: "smooth" }); }, [virtuosoItems]);

  const loadMoreMessages = useCallback(async () => {
    if (isLoadingMoreRef.current || !hasMoreMessages || !messages.length) return;
    isLoadingMoreRef.current = true;
    try {
      const older = await getMessagesPaginated(currentSessionId, messages[0].id, PAGE_LIMIT);
      if (!older.length) { setHasMoreMessages(false); return; }
      const mapped = older.map((m: any) => ({ id: m.id, role: m.role, content: m.content, reasoning: m.reasoning || undefined, confirmed: true }));
      setMessages(prev => [...mapped, ...prev]);
      setFirstItemIndex(p => Math.max(0, p - mapped.length));
      setHasMoreMessages(older.length >= PAGE_LIMIT);
    } catch (e) { console.error(e); }
    finally { isLoadingMoreRef.current = false; }
  }, [hasMoreMessages, messages, currentSessionId]);

  const renderVirtuosoItem = useCallback((_i: number, item: any) => {
    if (item.type === "user") {
      const isExpanded = expandedUserMsgId === item.msg.id;
      return (
        <div className="flex flex-col pb-3 px-4">
          <div className="w-full relative group">
            <div
              onClick={() => setExpandedUserMsgId(isExpanded ? null : item.msg.id)}
              className={`bg-primary/[0.14] border border-border/30 text-foreground/85 text-sm px-4 py-2.5 rounded-lg shadow-xs pr-10 font-medium selection:bg-transparent cursor-pointer transition-colors hover:border-primary/30 ${isExpanded ? "whitespace-pre-wrap break-words" : "truncate"}`}
              title={isExpanded ? "点击收起" : "点击查看完整消息"}
            >
              <span className="[&::selection]:bg-primary/15"><TaggedText text={item.msg.content} /></span>
            </div>
            <div className="absolute inset-y-0 right-2 flex items-center gap-0.5 opacity-0 group-hover:opacity-100 transition-opacity duration-150">
              <button
                onClick={() => setRollbackTargetId(item.msg.id)}
                disabled={item.isStreaming}
                className="p-1 rounded text-muted-foreground/40 hover:text-foreground hover:bg-muted/60 transition-colors disabled:opacity-20"
                title="回滚到此消息"
              >
                <RotateCcw className="h-3.5 w-3.5" />
              </button>
            </div>
          </div>
        </div>
      );
    }
    if (item.type === "round") {
      return (
        <div className="pb-3 px-4">
          <div className="flex items-end gap-1.5">
            <div className="w-full">
              <div className="text-sm leading-relaxed">
                {item.msgs.map((m: ChatMessage) => {
                  if (m.role === "assistant") {
                    // Render reasoning and content together, in message order, matching the streaming timeline
                    return (
                      <div key={m.id}>
                        {m.reasoning && <CollapsibleThinking content={m.reasoning} />}
                        {m.content && <ReactMarkdown remarkPlugins={[remarkGfm]} components={MD_COMPONENTS}>{m.content}</ReactMarkdown>}
                      </div>
                    );
                  }
                  if (m.role === "tool") {
                    const info = (() => { try { return JSON.parse(m.content); } catch { return { name: m.content, result: null }; } })();
                    const result = info.result;
                    const hasError = !!(result && typeof result === "object" && result.error !== undefined);
                    const summary = hasError ? undefined : summarizeToolResult(info.name || m.content, result);
                    const errText = hasError ? String(result.error) : undefined;
                    return (
                      <div key={m.id}>
                        <ToolCallCard name={info.name || m.content} target={summary} status={hasError ? "error" : "done"} error={errText} />
                        <div className="border-t border-border/20 my-2" />
                      </div>
                    );
                  }
                  return null;
                })}
              </div>
            </div>
          </div>
        </div>
      );
    }
    if (item.type === "streaming") {
      return (
        <div className="pb-3 px-4">
          {streamTimeline.length > 0 ? (
            <div className="text-sm leading-relaxed">
              {streamTimeline.map((ev, i) => {
                if (ev.type === "reasoning") {
                  // Only the last reasoning block stays open (still thinking); earlier ones collapse
                  const hasLaterContent = streamTimeline.slice(i + 1).some(e => e.type !== "reasoning");
                  return <CollapsibleThinking key={i} content={ev.content} isStreaming={!hasLaterContent} />;
                }
                if (ev.type === "text") {
                  return <ReactMarkdown key={i} remarkPlugins={[remarkGfm]} components={MD_COMPONENTS}>{ev.content}</ReactMarkdown>;
                }
                if (ev.type === "tool_call" && ev.toolCall) {
                  return <ToolCallCard key={i} name={ev.toolCall.name} args={ev.toolCall.args} target={ev.toolCall.resultSummary} error={ev.toolCall.error} status={ev.toolCall.status} />;
                }
                if (ev.type === "divider") {
                  return <div key={i} className="border-t border-border/20 my-2" />;
                }
                return null;
              })}
            </div>
          ) : (
            <div className="flex items-center gap-2 text-sm text-muted-foreground/60 py-2"><span className="w-1.5 h-1.5 rounded-full bg-primary animate-pulse" /><span>AI 思考中...</span></div>
          )}
        </div>
      );
    }
    return null;
  }, [streaming, streamReasoning, streamContent, streamTimeline, expandedUserMsgId]);

  if (loading) {
    return <div className="flex-1 flex items-center justify-center"><Loader2 className="h-5 w-5 animate-spin text-primary/60" /></div>;
  }

  return (
    <div className="flex-1 flex flex-col min-h-0">
      <div className="flex-1 flex min-h-0 overflow-hidden">
        {/* Left: Story Cards (写卡模式；纯聊模式隐藏，聊天占满宽度) */}
        {storyMode === "card" && (
          <div className="flex-1 min-w-0 bg-card overflow-hidden">
            <StoryCardsPanel storyId={storyId!} cards={cards} onCardsChanged={() => loadCards(storyId!)} onQuoteCard={handleQuoteCard} onQuoteFragment={handleQuoteFragment} />
          </div>
        )}

        {/* Right: Chat (写卡模式固定 35% 窗口宽度，纯聊模式占满) */}
        <div style={{ width: storyMode === "chat" ? "100%" : "35%" }} className="shrink-0 min-w-0 bg-card flex flex-col overflow-hidden border-l border-border">
          <div className="flex-1 relative">
            <Virtuoso
              key={currentSessionId}
              ref={virtuosoRef}
              data={virtuosoItems}
              itemContent={renderVirtuosoItem}
              computeItemKey={(_i: number, item: any) => item.key}
              firstItemIndex={firstItemIndex}
              startReached={loadMoreMessages}
              initialTopMostItemIndex={virtuosoItems.length > 0 ? virtuosoItems.length - 1 : 0}
              atBottomThreshold={50}
              atBottomStateChange={(atBottom: boolean) => setShowScrollBtn(!atBottom)}
              followOutput="smooth"
              components={MESSAGE_LIST_COMPONENTS}
              style={{ position: "absolute", inset: 0 }}
            />
            {showScrollBtn && <ScrollToBottom visible={showScrollBtn} onClick={scrollToBottom} />}
          </div>
          {currentSessionId && (
            <div className="shrink-0 border-t px-4 py-3">
              {extractionStatus !== "idle" && (
                <div className="flex items-center gap-2 text-xs text-muted-foreground mb-2 select-none">
                  {extractionStatus === "running" && (
                    <><Loader2 className="h-3.5 w-3.5 animate-spin text-primary" /><span>素材沉淀中...</span></>
                  )}
                  {extractionStatus === "done" && <span className="text-[#52ad5a]">素材沉淀完成</span>}
                  {extractionStatus === "failed" && <span className="text-[#e5484d]">素材沉淀失败</span>}
                </div>
              )}
              {storyMode === "chat" && (
                <div className="flex items-center gap-2 mb-2">
                  <Button
                    size="sm"
                    variant="outline"
                    className="h-7 text-[12px] px-3"
                    onClick={handleManualExtraction}
                    disabled={streaming || extractionStatus === "running"}
                  >
                    <Archive className="h-3.5 w-3.5 mr-1" />
                    素材沉淀
                  </Button>
                  <span className="text-[11px] text-muted-foreground/50">将当前剧情手动沉淀到素材（含当前剧情概览）</span>
                </div>
              )}
              <ChatInput
                onSend={handleSend}
                disabled={streaming}
                sendDisabled={extractionStatus === "running"}
                streaming={streaming}
                onStop={handleStop}
                prefill={prefill}
                prefillNonce={prefillNonce}
                model={model}
                onModelChange={() => handleModelChange(model === "deepseek-v4-flash" ? "deepseek-v4-pro" : "deepseek-v4-flash")}
                placeholder={storyMode === "chat" ? "输入你的行动或台词..." : "输入剧情简介或方向..."}
              />
            </div>
          )}
        </div>
      </div>

      {/* Rollback Dialog */}
      <Dialog open={!!rollbackTargetId} onOpenChange={() => setRollbackTargetId(null)}>
        <DialogContent className="sm:max-w-[360px] p-6" showCloseButton={false}>
          <DialogHeader><DialogTitle className="text-[16px]">确认回滚</DialogTitle></DialogHeader>
          <p className="text-sm text-muted-foreground/70 pt-1 leading-relaxed">回滚后将清除此消息及之后的所有内容，确定要继续吗？</p>
          <div className="flex justify-end gap-2 pt-5 border-t mt-5">
            <Button variant="outline" size="sm" className="h-8 text-[12px] px-4" onClick={() => setRollbackTargetId(null)}>取消</Button>
            <Button variant="destructive" size="sm" className="h-8 text-[12px] px-4" onClick={handleRollbackConfirm}>确认回滚</Button>
          </div>
        </DialogContent>
      </Dialog>
    </div>
  );
}
