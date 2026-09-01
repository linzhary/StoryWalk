import React from "react";

/**
 * 卡片引用标签正则：整卡 [第N轮] / 片段 [第N轮:S-E]（支持全角冒号）。
 * 注意：作为模块级常量不带 /g，使用时用 `new RegExp(CARD_TAG_RE.source, "g")` 避免 lastIndex 状态残留。
 */
export const CARD_TAG_RE = /\[第(\d+)轮(?:[:：](\d+)-(\d+))?\]/;

function CardTagChip({ round, start, end }: { round: number; start?: number; end?: number }) {
  const label =
    start !== undefined && end !== undefined
      ? `第${round}轮 · ${start}-${end}字`
      : `第${round}轮`;
  return (
    <span className="inline-flex items-center rounded bg-primary/10 text-primary text-[12px] px-1.5 py-0.5 mx-1 font-medium align-baseline">
      {label}
    </span>
  );
}

/** 将纯文本中的卡片引用标签拆分为 chip + 普通文本 */
export function TaggedText({ text }: { text: string }) {
  const parts: React.ReactNode[] = [];
  const re = new RegExp(CARD_TAG_RE.source, "g");
  let last = 0;
  let m: RegExpExecArray | null;
  let key = 0;
  while ((m = re.exec(text)) !== null) {
    if (m.index > last) parts.push(text.slice(last, m.index));
    parts.push(
      <CardTagChip
        key={key++}
        round={Number(m[1])}
        start={m[2] !== undefined ? Number(m[2]) : undefined}
        end={m[3] !== undefined ? Number(m[3]) : undefined}
      />
    );
    last = m.index + m[0].length;
  }
  if (last < text.length) parts.push(text.slice(last));
  return <>{parts}</>;
}

/**
 * 递归处理 React 子节点中的字符串，把卡片引用标签渲染为 chip。
 * 用于 markdown 组件（react-markdown v10 无 text 节点钩子）的文本容器包裹。
 */
export function renderCardTags(children: React.ReactNode): React.ReactNode {
  return React.Children.map(children, (child) => {
    if (typeof child === "string") return <TaggedText text={child} />;
    if (React.isValidElement(child)) {
      const c = child as React.ReactElement<{ children?: React.ReactNode }>;
      if (c.props && c.props.children !== undefined) {
        return React.cloneElement(c, {}, renderCardTags(c.props.children));
      }
      return c;
    }
    return child;
  });
}
