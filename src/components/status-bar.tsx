interface Props {
  mode: string;
  model: string;
  messageCount: number;
  lastSaved?: string;
}

export default function StatusBar({ mode, model, messageCount, lastSaved }: Props) {
  const modeLabel = mode === "settings" ? "设定模式" : "创作模式";
  const modelLabel = model === "deepseek-v4-pro" ? "专家" : "快速";
  const time = lastSaved
    ? new Date(lastSaved).toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit" })
    : null;

  return (
    <div className="flex items-center gap-4 px-4 h-7 bg-muted/50 border-t text-[11px] text-muted-foreground/60 select-none shrink-0">
      <span>{modeLabel}</span>
      <span className="text-border/60">·</span>
      <span>{modelLabel}</span>
      <span className="text-border/60">·</span>
      <span>{messageCount} 条消息</span>
      {time && (
        <>
          <span className="text-border/60">·</span>
          <span>最后保存 {time}</span>
        </>
      )}
    </div>
  );
}
