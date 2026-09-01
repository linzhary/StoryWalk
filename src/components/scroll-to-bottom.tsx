import { ArrowDown } from "lucide-react";

interface Props {
  visible: boolean;
  onClick: () => void;
  count?: number;
}

export default function ScrollToBottom({ visible, onClick, count }: Props) {
  return (
    <div className={`absolute left-0 right-0 bottom-0 z-10 pointer-events-none transition-opacity duration-200 ${visible ? "opacity-100" : "opacity-0"}`} style={{ height: "50px" }}>
      <div
        className="absolute inset-0 pointer-events-none"
        style={{
          background: "var(--background)",
          maskImage: "linear-gradient(to bottom, transparent 0%, black 85%)",
          WebkitMaskImage: "linear-gradient(to bottom, transparent 0%, black 85%)",
        }}
      />
      <div className="relative flex items-center justify-center pb-3 pointer-events-auto">
        {count && count > 0 ? (
          <span className="absolute right-8 text-[11px] font-medium text-muted-foreground/70 whitespace-nowrap">
            {count} 条新消息
          </span>
        ) : null}
        <button
          onClick={onClick}
          className="flex items-center justify-center w-[30px] h-[30px] rounded-full bg-background border border-border/80 shadow-[0_6px_16px_rgba(0,0,0,0.12)] text-muted-foreground/60 hover:text-foreground hover:border-border hover:shadow-lg transition-all active:scale-95"
          style={{ backdropFilter: "blur(14px)" }}
        >
          <ArrowDown className="h-4 w-4" />
        </button>
      </div>
    </div>
  );
}
