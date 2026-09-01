import { useCallback, useEffect, useRef, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { ChevronDown, ChevronRight, Copy, Pencil, Check, Trash2, Quote } from "lucide-react";
import type { StoryCard } from "@/lib/mock-data";
import { updateStoryCard, deleteStoryCard } from "@/lib/api";

const MD_COMPONENTS = {
  // Paragraph spacing
  p: ({ children }: any) => <p className="mb-[0.28em] last:mb-0">{children}</p>,
  // Inline code
  code: ({ className, children, ...props }: any) => {
    const isInline = !className;
    return isInline
      ? <code className="font-mono text-[0.88em] font-medium border rounded-[5px] px-[0.28em] py-[0.08em] mx-[0.03em] whitespace-nowrap transition-colors duration-150 [box-decoration-break:clone]" style={{ background: "var(--md-inline-code-bg)", borderColor: "var(--md-inline-code-border)" }} {...props}>{children}</code>
      : (
        <div className="my-[0.72em] border border-border/30 rounded-lg overflow-hidden">
          <div className="flex items-center justify-between min-h-[1.86rem] px-[0.62rem] py-[0.26rem] border-b border-border/30" style={{ background: "var(--md-code-toolbar-bg)" }}>
            <span className="text-[10px] font-semibold text-muted-foreground/60 uppercase tracking-wider select-none">
              {className?.replace("language-", "") || "code"}
            </span>
          </div>
          <pre className="m-0 p-[0.78rem_0.86rem] overflow-x-auto text-[13px] leading-[1.52] font-mono" style={{ background: "var(--md-code-bg)" }}>
            <code>{children}</code>
          </pre>
        </div>
      );
  },
  // Blockquote
  blockquote: ({ children }: any) => (
    <blockquote className="my-[0.7em] pl-[0.72rem] pr-[0.72rem] py-[0.58rem] border-l-[3px] rounded-r-[6px] text-muted-foreground" style={{ background: "var(--md-blockquote-bg)", borderLeftColor: "var(--md-blockquote-border)" }}>
      {children}
    </blockquote>
  ),
  // Link
  a: ({ href, children }: any) => (
    <a href={href} className="text-primary underline decoration-1 underline-offset-2 hover:text-[#234a6d] hover:decoration-[1.5px]" target="_blank" rel="noreferrer">{children}</a>
  ),
  // Lists
  ul: ({ children }: any) => <ul className="list-disc pl-[1.35rem] my-0 [&>li]:mt-[0.28em] [&>li]:leading-[1.58] first:mt-0">{children}</ul>,
  ol: ({ children }: any) => <ol className="list-decimal pl-[1.35rem] my-0 [&>li]:mt-[0.28em] [&>li]:leading-[1.58] first:mt-0">{children}</ol>,
  // Table
  table: ({ children }: any) => (
    <div
      className="my-[0.86em] border rounded-lg overflow-auto"
      style={{ borderColor: "var(--md-table-border)", background: "var(--md-table-surface)", boxShadow: "var(--md-table-shadow)" }}
    >
      <table className="w-full border-collapse border-spacing-0 text-[13px] leading-[1.52] [&_tbody_tr:nth-child(even)]:bg-[var(--md-table-row-stripe)] [&_tbody>tr:hover]:bg-[var(--md-table-row-hover)]">{children}</table>
    </div>
  ),
  thead: ({ children }: any) => <thead>{children}</thead>,
  th: ({ children }: any) => (
    <th
      className="px-[0.78rem] py-[0.48rem] text-left text-[13px] font-semibold border-b first:border-l-0 border-l"
      style={{ background: "var(--md-table-header-bg)", color: "var(--md-table-header-fg)", borderBottomColor: "var(--md-table-cell-border)", borderLeftColor: "var(--md-table-col-divider)" }}
    >
      {children}
    </th>
  ),
  td: ({ children }: any) => (
    <td
      className="px-[0.78rem] py-[0.48rem] text-left align-top border-b first:border-l-0 border-l"
      style={{ borderBottomColor: "var(--md-table-cell-border)", borderLeftColor: "var(--md-table-col-divider)" }}
    >
      {children}
    </td>
  ),
  tr: ({ children }: any) => (
    <tr className="[&:last-child_td]:border-b-0">
      {children}
    </tr>
  ),
  // HR
  hr: () => <hr className="h-px my-[1.04em] border-0 rounded bg-border" />,
  // Strong / em
  strong: ({ children }: any) => <strong className="font-semibold">{children}</strong>,
  em: ({ children }: any) => <em className="text-muted-foreground italic">{children}</em>,
  // Heading
  h1: ({ children }: any) => <h1 className="text-[16px] font-semibold leading-[1.45] my-[0.86em_0_0.36em]">{children}</h1>,
  h2: ({ children }: any) => <h2 className="text-[15px] font-semibold leading-[1.45] my-[0.86em_0_0.36em]">{children}</h2>,
  h3: ({ children }: any) => <h3 className="text-[14px] font-semibold leading-[1.45] my-[0.86em_0_0.36em]">{children}</h3>,
  h4: ({ children }: any) => <h4 className="text-[14px] font-semibold leading-[1.45] text-muted-foreground my-[0.86em_0_0.36em]">{children}</h4>,
};

interface StoryCardsPanelProps {
  storyId: string;
  cards: StoryCard[];
  onCardsChanged: () => void;
  /** 引用整张卡片到右侧聊天栏：仅传轮次号（标签 [第N轮]） */
  onQuoteCard?: (roundNumber: number) => void;
  /** 引用卡片片段到右侧聊天栏：轮次 + 字符区间（标签 [第N轮:S-E]） */
  onQuoteFragment?: (roundNumber: number, start: number, end: number) => void;
}

function countChineseChars(text: string): number {
  const matches = text.match(/[\u4e00-\u9fff\u3400-\u4dbf]/g);
  return matches ? matches.length : 0;
}

function parseTimestamp(raw: string): Date | null {
  try {
    let s = raw.trim();
    // SQLite datetime('now') 返回 "YYYY-MM-DD HH:MM:SS"（UTC，无时区标记），
    // 归一化为 ISO 8601 再解析，否则 new Date 可能得到 Invalid Date
    if (!s.includes("T")) {
      s = s.replace(" ", "T");
      if (!/[+-]\d{2}:?\d{2}$|Z$/i.test(s)) s += "Z";
    }
    const date = new Date(s);
    return isNaN(date.getTime()) ? null : date;
  } catch {
    return null;
  }
}

function formatTimestamp(raw: string): string {
  const date = parseTimestamp(raw);
  if (!date) return "";
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMin = Math.floor(diffMs / 60000);
  if (diffMin < 1) return "刚刚";
  if (diffMin < 60) return `${diffMin} 分钟前`;
  const diffHour = Math.floor(diffMin / 60);
  if (diffHour < 24) return `${diffHour} 小时前`;
  const diffDay = Math.floor(diffHour / 24);
  if (diffDay < 7) return `${diffDay} 天前`;
  return date.toLocaleDateString("zh-CN", { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" });
}

export default function StoryCardsPanel({ storyId: _storyId, cards, onCardsChanged, onQuoteCard, onQuoteFragment }: StoryCardsPanelProps) {
  // 展开集合，默认空 = 所有卡片折叠
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set());
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editContent, setEditContent] = useState("");
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const [fragMenu, setFragMenu] = useState<{ roundNumber: number; start: number; end: number; x: number; y: number } | null>(null);
  const fragMenuRef = useRef<HTMLDivElement>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const prevLengthRef = useRef(cards.length);

  // 将 Range 的起止点映射为容器内扁平文本（按文档序拼接的 text node）的字符偏移
  const computeRangeOffsets = useCallback((container: HTMLElement, range: Range): { start: number; end: number } => {
    const texts: Node[] = [];
    const walker = document.createTreeWalker(container, NodeFilter.SHOW_TEXT, null);
    let t: Node | null;
    while ((t = walker.nextNode())) texts.push(t);

    const boundaryOffset = (node: Node, offset: number): number => {
      let acc = 0;
      for (const tn of texts) {
        const len = (tn.textContent ?? "").length;
        if (tn === node) return acc + Math.min(offset, len);
        if (node.contains(tn)) {
          // node 是元素（可能为直接父级或更深祖先），offset 指向其子节点位置
          let c: Node | null = tn;
          let idx = 0;
          while (c && c.parentNode && c.parentNode !== node) c = c.parentNode;
          if (c && c.parentNode === node) {
            for (let s = c.previousSibling; s; s = s.previousSibling) idx++;
          }
          if (idx < offset) { acc += len; continue; }
          return acc;
        }
        // 一般顺序：node 在 tn 之后 → tn 在边界之前
        if ((tn.compareDocumentPosition(node) & Node.DOCUMENT_POSITION_FOLLOWING) !== 0) { acc += len; continue; }
        return acc;
      }
      return acc;
    };

    const start = boundaryOffset(range.startContainer, range.startOffset);
    const end = boundaryOffset(range.endContainer, range.endOffset);
    return { start, end };
  }, []);

  // 正文右键：选区非空且落在当前卡片内时，弹出「引用到聊天」菜单
  const handleBodyContextMenu = useCallback((e: React.MouseEvent<HTMLDivElement>, card: StoryCard) => {
    const sel = window.getSelection();
    if (!sel || sel.isCollapsed || sel.rangeCount === 0) return;
    const range = sel.getRangeAt(0);
    const container = e.currentTarget;
    if (!container.contains(range.commonAncestorContainer)) return;
    const { start, end } = computeRangeOffsets(container, range);
    if (start >= end) return;
    e.preventDefault();
    setFragMenu({ roundNumber: card.roundNumber, start, end, x: e.clientX, y: e.clientY });
  }, [computeRangeOffsets]);

  // 点击菜单外部 / Escape / 滚动时关闭片段菜单
  useEffect(() => {
    if (!fragMenu) return;
    const onDocMouseDown = (e: MouseEvent) => {
      if (fragMenuRef.current && !fragMenuRef.current.contains(e.target as Node)) setFragMenu(null);
    };
    const onKey = (e: KeyboardEvent) => { if (e.key === "Escape") setFragMenu(null); };
    const onScroll = () => setFragMenu(null);
    document.addEventListener("mousedown", onDocMouseDown);
    document.addEventListener("keydown", onKey);
    document.addEventListener("scroll", onScroll, true);
    return () => {
      document.removeEventListener("mousedown", onDocMouseDown);
      document.removeEventListener("keydown", onKey);
      document.removeEventListener("scroll", onScroll, true);
    };
  }, [fragMenu]);

  // Auto-scroll to bottom when new cards are added
  useEffect(() => {
    if (cards.length > prevLengthRef.current && scrollRef.current) {
      scrollRef.current.scrollTo({ top: scrollRef.current.scrollHeight, behavior: "smooth" });
    }
    prevLengthRef.current = cards.length;
  }, [cards.length]);

  const toggleExpand = useCallback((cardId: string) => {
    setExpandedIds((prev) => {
      const next = new Set(prev);
      if (next.has(cardId)) {
        next.delete(cardId);
      } else {
        next.add(cardId);
      }
      return next;
    });
  }, []);

  const handleCopy = useCallback(async (content: string, cardId: string) => {
    try {
      await navigator.clipboard.writeText(content);
      setCopiedId(cardId);
      setTimeout(() => setCopiedId(null), 2000);
    } catch {
      // Fallback: ignore
    }
  }, []);

  const handleDelete = useCallback(async (cardId: string) => {
    try {
      await deleteStoryCard(cardId);
      onCardsChanged();
    } catch (e) { console.error(e); }
  }, [onCardsChanged]);

  const startEditing = useCallback((cardId: string, content: string) => {
    setEditingId(cardId);
    setEditContent(content);
  }, []);

  const cancelEditing = useCallback(() => {
    setEditingId(null);
    setEditContent("");
  }, []);

  const saveEdit = useCallback(async () => {
    if (!editingId) return;
    const cardId = editingId;
    const content = editContent;
    setEditingId(null);
    setEditContent("");
    try {
      await updateStoryCard(cardId, content);
      onCardsChanged();
    } catch (e) {
      console.error("Failed to update story card:", e);
    }
  }, [editingId, editContent, onCardsChanged]);

  const handleEditKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        saveEdit();
      } else if (e.key === "Escape") {
        e.preventDefault();
        cancelEditing();
      }
    },
    [saveEdit, cancelEditing]
  );

  return (
    <div ref={scrollRef} className="h-full overflow-y-auto px-4 py-4 [scrollbar-gutter:stable]">
      {cards.length === 0 && (
        <div className="flex items-center justify-center h-full text-sm text-muted-foreground/50">
          暂无创作卡片
        </div>
      )}
      {cards.map((card) => {
        const expanded = expandedIds.has(card.id);
        const editing = editingId === card.id;
        const chineseCount = countChineseChars(card.content);

        return (
          <div
            key={card.id}
            className="border rounded-xl bg-card mb-3 shadow-sm overflow-hidden"
          >
            {/* Header: badge + time + actions (always visible) + expand toggle */}
            <div className="flex items-center gap-2 px-4 py-2.5">
              <span className="inline-flex items-center px-2 py-0.5 text-xs font-medium rounded-full bg-primary/10 text-primary shrink-0">
                第 {card.roundNumber} 轮
              </span>
              <span className="text-xs text-muted-foreground flex-1 min-w-0 truncate">
                {formatTimestamp(card.createdAt)}
              </span>
              <div className="flex items-center gap-1 shrink-0">
                {editing ? (
                  <>
                    <button
                      onClick={cancelEditing}
                      className="inline-flex items-center gap-1 px-2 py-1 rounded text-muted-foreground/60 hover:text-destructive hover:bg-destructive/10 transition-colors"
                      title="取消"
                    >
                      取消
                    </button>
                    <button
                      onMouseDown={(e) => {
                        // Prevent onBlur from firing before click
                        e.preventDefault();
                        saveEdit();
                      }}
                      className="inline-flex items-center gap-1 px-2 py-1 rounded text-primary hover:bg-primary/10 transition-colors"
                      title="保存 (⌘Enter)"
                    >
                      <Check className="h-3.5 w-3.5" />
                      <span>保存</span>
                    </button>
                  </>
                ) : (
                  <>
                    {/* Quote button: reference this card in the right-side chat */}
                    {onQuoteCard && (
                      <button
                        onClick={() => onQuoteCard(card.roundNumber)}
                        className="inline-flex items-center gap-1 px-2 py-1 rounded text-muted-foreground/60 hover:text-foreground hover:bg-muted/60 transition-colors"
                        title="引用到聊天"
                      >
                        <Quote className="h-3.5 w-3.5" />
                      </button>
                    )}
                    {/* Edit button: expands the card first, then starts editing */}
                    <button
                      onClick={() => {
                        setExpandedIds(prev => new Set(prev).add(card.id));
                        startEditing(card.id, card.content);
                      }}
                      className="inline-flex items-center gap-1 px-2 py-1 rounded text-muted-foreground/60 hover:text-foreground hover:bg-muted/60 transition-colors"
                      title="编辑"
                    >
                      <Pencil className="h-3.5 w-3.5" />
                    </button>
                    {/* Copy button */}
                    <button
                      onClick={() => handleCopy(card.content, card.id)}
                      className="inline-flex items-center gap-1 px-2 py-1 rounded text-muted-foreground/60 hover:text-foreground hover:bg-muted/60 transition-colors"
                      title="复制"
                    >
                      {copiedId === card.id ? (
                        <>
                          <Check className="h-3.5 w-3.5 text-green-500" />
                          <span className="text-green-500">已复制</span>
                        </>
                      ) : (
                        <Copy className="h-3.5 w-3.5" />
                      )}
                    </button>
                    {/* Delete button */}
                    <button
                      onClick={() => handleDelete(card.id)}
                      className="inline-flex items-center gap-1 px-2 py-1 rounded text-muted-foreground/60 hover:text-destructive hover:bg-destructive/10 transition-colors"
                      title="删除"
                    >
                      <Trash2 className="h-3.5 w-3.5" />
                    </button>
                  </>
                )}
                <button
                  onClick={() => toggleExpand(card.id)}
                  className="p-1 rounded text-muted-foreground/60 hover:text-foreground hover:bg-muted/60 transition-colors"
                  title={expanded ? "折叠" : "展开"}
                >
                  {expanded ? (
                    <ChevronDown className="h-4 w-4" />
                  ) : (
                    <ChevronRight className="h-4 w-4" />
                  )}
                </button>
              </div>
            </div>

            {/* Body */}
            {expanded && !editing && (
              <div
                className="px-4 pb-3 text-sm leading-relaxed cursor-default"
                onDoubleClick={() => startEditing(card.id, card.content)}
                onContextMenu={(e) => handleBodyContextMenu(e, card)}
              >
                <ReactMarkdown remarkPlugins={[remarkGfm]} components={MD_COMPONENTS}>
                  {card.content}
                </ReactMarkdown>
              </div>
            )}

            {/* Edit mode */}
            {expanded && editing && (
              <div className="px-4 pb-3">
                <textarea
                  className="w-full min-h-[120px] text-sm leading-relaxed rounded-lg border border-input bg-background px-3 py-2 outline-none focus-visible:ring-1 focus-visible:ring-ring resize-y"
                  value={editContent}
                  onChange={(e) => setEditContent(e.target.value)}
                  onKeyDown={handleEditKeyDown}
                  onBlur={saveEdit}
                  autoFocus
                />
              </div>
            )}

            {/* Footer: word count only */}
            {expanded && (
              <div className="flex items-center px-4 py-2 border-t text-xs text-muted-foreground">
                <span>{chineseCount} 字</span>
              </div>
            )}
          </div>
        );
      })}

      {/* Fragment quote menu (right-click on selected text in a card body) */}
      {fragMenu && (
        <div
          ref={fragMenuRef}
          className="fixed z-50 min-w-[132px] rounded-lg border border-border/60 bg-card shadow-lg overflow-hidden"
          style={{ left: fragMenu.x, top: fragMenu.y }}
        >
          <button
            className="flex items-center gap-1.5 w-full px-3 py-2 text-[13px] text-foreground/85 hover:bg-muted/60 transition-colors"
            onClick={() => {
              onQuoteFragment?.(fragMenu.roundNumber, fragMenu.start, fragMenu.end);
              setFragMenu(null);
            }}
          >
            <Quote className="h-3.5 w-3.5" />
            引用到聊天
          </button>
          <p className="px-3 pb-2 text-[11px] text-muted-foreground/60 select-none">
            {`第 ${fragMenu.roundNumber} 轮 ${fragMenu.start}-${fragMenu.end} 字`}
          </p>
        </div>
      )}
    </div>
  );
}
