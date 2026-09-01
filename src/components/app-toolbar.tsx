import { useCallback, useEffect, useState } from "react";
import { Link, useLocation } from "react-router-dom";
import { Home, Minus, Square, X } from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { platform } from "@tauri-apps/plugin-os";
import { getStory } from "@/lib/api";

const INTERACTIVE_SELECTOR = 'button, input, textarea, select, a, [role="button"]';

export default function AppToolbar() {
  const location = useLocation();
  const showHome = location.pathname !== "/";
  const [isMac, setIsMac] = useState(true);
  const [maximized, setMaximized] = useState(false);
  const [storyTitle, setStoryTitle] = useState("");

  // Extract storyId from URL path
  const storyMatch = location.pathname.match(/^\/stories\/(.+)$/);
  const storyId = storyMatch ? storyMatch[1] : null;

  useEffect(() => { setIsMac(platform() === "macos"); }, []);

  useEffect(() => {
    const win = getCurrentWindow();
    win.isMaximized().then(setMaximized);
    const unlisten = win.onResized(() => {
      win.isMaximized().then(setMaximized);
    });
    return () => { unlisten.then(fn => fn()); };
  }, []);

  // Load story title when on a story page
  useEffect(() => {
    if (storyId) {
      getStory(storyId).then(s => setStoryTitle(s.title)).catch(() => setStoryTitle(""));
    } else {
      setStoryTitle("");
    }
  }, [storyId]);

  const handleMouseDown = useCallback(async (e: React.MouseEvent) => {
    if (e.button !== 0) return;
    const target = e.target as HTMLElement | null;
    if (target?.closest(INTERACTIVE_SELECTOR)) return;
    try {
      await getCurrentWindow().startDragging();
    } catch {
      // 非窗口区域拖拽失败时忽略
    }
  }, []);

  return (
    <header
      className="flex items-center h-10 bg-card border-b shrink-0 select-none"
      style={{ paddingLeft: isMac ? 85 : 8, paddingRight: 0 }}
      onMouseDown={handleMouseDown}
    >
      <div className="flex-1 flex items-center gap-2 min-w-0">
        {showHome && (
          <Link to="/" className="p-1.5 rounded-md text-muted-foreground/60 hover:text-foreground hover:bg-muted/60 transition-colors shrink-0" title="首页" onMouseDown={(e) => e.stopPropagation()}>
            <Home className="h-[18px] w-[18px]" />
          </Link>
        )}
        {storyTitle && (
          <span className="text-sm text-foreground/70 font-medium truncate">{storyTitle}</span>
        )}
      </div>

      {/* Windows window controls */}
      {!isMac && (
        <div className="flex items-stretch h-full" onMouseDown={(e) => e.stopPropagation()}>
          <button
            onClick={() => getCurrentWindow().minimize()}
            className="px-4 text-muted-foreground/50 hover:text-foreground hover:bg-muted/60 transition-colors"
            title="最小化"
          >
            <Minus className="h-[16px] w-[16px]" />
          </button>
          <button
            onClick={async () => {
              const win = getCurrentWindow();
              if (await win.isMaximized()) {
                win.unmaximize();
              } else {
                win.maximize();
              }
            }}
            className="px-4 text-muted-foreground/50 hover:text-foreground hover:bg-muted/60 transition-colors"
            title={maximized ? "还原" : "最大化"}
          >
            {maximized ? (
              <span className="text-[12px] font-medium leading-none block" style={{ transform: "scale(1.2)" }}>❐</span>
            ) : (
              <Square className="h-[14px] w-[14px]" />
            )}
          </button>
          <button
            onClick={() => getCurrentWindow().close()}
            className="px-4 text-muted-foreground/50 hover:text-white hover:bg-red-500/80 transition-colors"
            title="关闭"
          >
            <X className="h-[16px] w-[16px]" />
          </button>
        </div>
      )}
    </header>
  );
}
