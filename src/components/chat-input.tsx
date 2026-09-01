"use client";

import { useState, useRef, useEffect, useCallback } from "react";
import { Square, ArrowUp, Zap, Sparkles } from "lucide-react";
import { CARD_TAG_RE } from "@/components/card-tag-text";

interface ChatInputProps {
  onSend: (message: string) => void;
  disabled: boolean;
  /** 仅禁用发送（允许输入）：如后台素材沉淀进行中 */
  sendDisabled?: boolean;
  placeholder?: string;
  prefill?: string;
  /** 递增计数器：prefill 内容相同时也能触发重新填充（如重复引用同一张卡片） */
  prefillNonce?: number;
  streaming?: boolean;
  onStop?: () => void;
  model: string;
  onModelChange: () => void;
}

/** 匹配“光标前刚完成”的卡片引用标签（整卡 [第N轮] / 片段 [第N轮:S-E]） */
const TAG_END_RE = /\[第\d+轮(?:[:：]\d+-\d+)?\]$/;

function escapeHtml(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
}

/** 把纯文本（含卡片引用标签）渲染为可编辑内容 HTML：标签 → 带 data-tag 的 chip span */
function renderEditableHtml(value: string): string {
  const parts: string[] = [];
  const re = new RegExp(CARD_TAG_RE.source, "g");
  let last = 0;
  let m: RegExpExecArray | null;
  while ((m = re.exec(value)) !== null) {
    if (m.index > last) parts.push(escapeHtml(value.slice(last, m.index)));
    const raw = m[0];
    const visible =
      m[2] !== undefined && m[3] !== undefined
        ? `第${m[1]}轮 · ${m[2]}-${m[3]}字`
        : `第${m[1]}轮`;
    parts.push(
      `<span class="inline-flex items-center rounded bg-primary/10 text-primary text-[12px] px-1.5 py-0.5 mx-1 font-medium align-baseline select-none" contenteditable="false" data-tag="${escapeHtml(raw)}">${escapeHtml(visible)}</span>`
    );
    last = m.index + raw.length;
  }
  if (last < value.length) parts.push(escapeHtml(value.slice(last)));
  return parts.join("");
}

/** 从可编辑 DOM 提取纯文本：chip 还原为带括号的原始标签，BR/块级元素产生换行 */
function getPlainText(el: HTMLElement): string {
  let out = "";
  const walk = (node: Node) => {
    if (node.nodeType === Node.TEXT_NODE) { out += node.nodeValue ?? ""; return; }
    const e = node as HTMLElement;
    if (e.tagName === "BR") { if (!out.endsWith("\n")) out += "\n"; return; }
    if (e.dataset && e.dataset.tag !== undefined) { out += e.getAttribute("data-tag") ?? ""; return; }
    const block = e.tagName === "DIV" || e.tagName === "P" || e.tagName === "LI";
    if (block && out.length > 0 && !out.endsWith("\n")) out += "\n";
    e.childNodes.forEach(walk);
  };
  el.childNodes.forEach(walk);
  return out.replace(/\n{3,}/g, "\n\n");
}

/** 光标在“还原后纯文本（含标签）”空间的字符偏移 */
function captureCaretOffset(el: HTMLElement): number {
  const sel = window.getSelection();
  if (!sel || sel.rangeCount === 0) return getPlainText(el).length;
  const anchor = sel.anchorNode;
  if (!anchor || !el.contains(anchor)) return getPlainText(el).length;
  const anchorOffset = sel.anchorOffset;
  let acc = 0;
  let hit = false;
  const visit = (node: Node): void => {
    if (hit) return;
    if (node === anchor) {
      if (node.nodeType === Node.TEXT_NODE) acc += Math.min(anchorOffset, (node.nodeValue ?? "").length);
      hit = true;
      return;
    }
    if (node.nodeType === Node.TEXT_NODE) { acc += (node.nodeValue ?? "").length; return; }
    const e = node as HTMLElement;
    if (e.dataset && e.dataset.tag !== undefined) { acc += (e.getAttribute("data-tag") ?? "").length; return; }
    e.childNodes.forEach(visit);
  };
  el.childNodes.forEach(visit);
  return acc;
}

/** 把光标放到“还原后纯文本”空间的指定偏移（跳过 chip 内部） */
function placeCaret(el: HTMLElement, offset: number) {
  const sel = window.getSelection();
  if (!sel) return;
  // 收集按文档序的文本节点与 chip 片段（chip 在偏移空间中占其 data-tag 长度）
  const segments: { isChip: boolean; node: Node; len: number }[] = [];
  const visit = (node: Node) => {
    if (node.nodeType === Node.TEXT_NODE) {
      segments.push({ isChip: false, node, len: (node.nodeValue ?? "").length });
      return;
    }
    const e = node as HTMLElement;
    if (e.dataset && e.dataset.tag !== undefined) {
      segments.push({ isChip: true, node, len: (e.getAttribute("data-tag") ?? "").length });
      return;
    }
    e.childNodes.forEach(visit);
  };
  el.childNodes.forEach(visit);

  const range = document.createRange();
  let remaining = offset;
  let placed: { isChip: boolean; node: Node; off: number; len: number } | null = null;
  for (const seg of segments) {
    if (remaining <= seg.len) {
      placed = { isChip: seg.isChip, node: seg.node, off: remaining, len: seg.len };
      break;
    }
    remaining -= seg.len;
  }

  if (!placed) {
    range.selectNodeContents(el);
    range.collapse(false);
  } else if (!placed.isChip) {
    range.setStart(placed.node, Math.min(placed.off, (placed.node.nodeValue ?? "").length));
    range.collapse(true);
  } else if (placed.off >= placed.len) {
    // 偏移恰在标签末尾 → 光标落在标签之后
    range.setStartAfter(placed.node);
    range.collapse(true);
  } else {
    // 偏移落在标签内部 → 光标落在标签之前
    range.setStartBefore(placed.node);
    range.collapse(true);
  }
  sel.removeAllRanges();
  sel.addRange(range);
}

export default function ChatInput({ onSend, disabled, sendDisabled, placeholder, prefill, prefillNonce, streaming, onStop, model, onModelChange }: ChatInputProps) {
  const [value, setValue] = useState("");
  const editRef = useRef<HTMLDivElement>(null);
  const composingRef = useRef(false);
  // 最近一次光标位置（“还原后纯文本”空间偏移）；点击卡片引用时输入框已失焦，用该位置插入
  const lastCaretRef = useRef(0);

  const syncCaretRef = useCallback(() => {
    const el = editRef.current;
    if (!el) return;
    const sel = window.getSelection();
    if (sel && sel.rangeCount > 0 && sel.anchorNode && el.contains(sel.anchorNode)) {
      lastCaretRef.current = captureCaretOffset(el);
    }
  }, []);

  // 引用卡片/片段：把标签插入到最近光标位置（而非替换内容），插入后光标位于标签之后
  useEffect(() => {
    if (!prefill) return;
    const el = editRef.current;
    if (!el) return;
    const text = getPlainText(el);
    const caret = Math.min(lastCaretRef.current, text.length);
    const newText = text.slice(0, caret) + prefill + text.slice(caret);
    el.innerHTML = renderEditableHtml(newText);
    lastCaretRef.current = caret + prefill.length;
    el.focus();
    placeCaret(el, lastCaretRef.current);
    setValue(newText);
  }, [prefill, prefillNonce]);

  const handleSend = useCallback(() => {
    const el = editRef.current;
    if (!el) return;
    const text = getPlainText(el).trim();
    if (!text || disabled || sendDisabled) return;
    onSend(text);
    el.innerHTML = "";
    setValue("");
    lastCaretRef.current = 0;
  }, [onSend, disabled, sendDisabled]);

  const handleInput = useCallback(() => {
    const el = editRef.current;
    if (!el) return;
    const text = getPlainText(el);
    setValue(text);
    if (composingRef.current) return;
    // 光标前刚完成一个卡片引用标签 → 重新渲染为 chip（光标保持在原偏移）
    const sel = window.getSelection();
    if (!sel || sel.rangeCount === 0) return;
    const caret = captureCaretOffset(el);
    lastCaretRef.current = caret;
    if (TAG_END_RE.test(text.slice(0, caret))) {
      el.innerHTML = renderEditableHtml(text);
      placeCaret(el, caret);
    }
  }, []);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      if (e.nativeEvent.isComposing || composingRef.current) return;
      e.preventDefault();
      handleSend();
    }
  }, [handleSend]);

  const modelLabel = model === "deepseek-v4-pro" ? "专家" : "快速";
  const ModelIcon = model === "deepseek-v4-pro" ? Sparkles : Zap;
  const modelColor = model === "deepseek-v4-pro" ? "text-amber-600 bg-amber-500/10" : "text-emerald-600 bg-emerald-500/10";
  const canSend = !!value.trim() && !disabled && !sendDisabled;

  return (
    <div className="flex items-center gap-2.5 px-3 py-1.5 rounded-lg border border-border/60 bg-muted/30 focus-within:border-primary/30 focus-within:ring-1 focus-within:ring-ring/20 transition-all">
      <div
        ref={editRef}
        contentEditable={disabled ? "false" : "plaintext-only"}
        suppressContentEditableWarning
        spellCheck={false}
        data-placeholder={placeholder || "输入消息..."}
        onInput={handleInput}
        onKeyDown={handleKeyDown}
        onKeyUp={syncCaretRef}
        onClick={syncCaretRef}
        onSelect={syncCaretRef}
        onFocus={syncCaretRef}
        onCompositionStart={() => { composingRef.current = true; }}
        onCompositionEnd={() => { composingRef.current = false; handleInput(); }}
        className="flex-1 min-w-0 bg-transparent border-none outline-none text-sm py-1 leading-relaxed whitespace-pre-wrap break-words max-h-[140px] overflow-y-auto empty:before:content-[attr(data-placeholder)] empty:before:text-muted-foreground/40 empty:before:pointer-events-none"
      />
      <button
        onClick={onModelChange}
        disabled={disabled}
        className={`inline-flex items-center gap-1 h-7 px-2.5 rounded-full text-[11px] font-medium transition-colors disabled:opacity-50 shrink-0 ${modelColor}`}
        title={model === "deepseek-v4-flash" ? "切换到 deepseek-v4-pro" : "切换到 deepseek-v4-flash"}
      >
        <ModelIcon className="h-3 w-3" />
        {modelLabel}
      </button>
      {streaming ? (
        <button
          onClick={onStop}
          className="w-8 h-8 rounded-lg bg-destructive text-destructive-foreground flex items-center justify-center shrink-0 hover:opacity-90 transition-opacity"
        >
          <Square className="h-3.5 w-3.5" fill="currentColor" />
        </button>
      ) : (
        <button
          onClick={handleSend}
          disabled={!canSend}
          className="w-8 h-8 rounded-full bg-primary text-primary-foreground flex items-center justify-center shrink-0 hover:opacity-90 transition-opacity disabled:opacity-30"
        >
          <ArrowUp className="h-4 w-4" />
        </button>
      )}
    </div>
  );
}
