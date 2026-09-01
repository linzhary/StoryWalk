import { useCallback, useRef, useState, useEffect, type ReactNode } from "react";

interface Props {
  children: [ReactNode, ReactNode, ReactNode]; // left, center, right
  leftWidth: number;
  rightWidth: number;
  onLeftResize: (w: number) => void;
  onRightResize: (w: number) => void;
  leftMin?: number;
  leftMax?: number;
  rightMin?: number;
  rightMax?: number;
}

export default function ResizablePanel({
  children,
  leftWidth,
  rightWidth,
  onLeftResize,
  onRightResize,
  leftMin = 160,
  leftMax = 400,
  rightMin = 200,
  rightMax = 500,
}: Props) {
  const [dragging, setDragging] = useState<"left" | "right" | null>(null);
  const startXRef = useRef(0);
  const startWidthRef = useRef(0);

  const clamp = (v: number, min: number, max: number) => Math.min(Math.max(v, min), max);

  const onMouseDown = useCallback(
    (side: "left" | "right") => (e: React.MouseEvent) => {
      e.preventDefault();
      setDragging(side);
      startXRef.current = e.clientX;
      startWidthRef.current = side === "left" ? leftWidth : rightWidth;
    },
    [leftWidth, rightWidth]
  );

  useEffect(() => {
    if (!dragging) return;

    const onMouseMove = (e: MouseEvent) => {
      const delta = e.clientX - startXRef.current;
      if (dragging === "left") {
        const newW = clamp(startWidthRef.current + delta, leftMin, leftMax);
        onLeftResize(newW);
      } else {
        const newW = clamp(startWidthRef.current - delta, rightMin, rightMax);
        onRightResize(newW);
      }
    };

    const onMouseUp = () => setDragging(null);

    document.addEventListener("mousemove", onMouseMove);
    document.addEventListener("mouseup", onMouseUp);
    return () => {
      document.removeEventListener("mousemove", onMouseMove);
      document.removeEventListener("mouseup", onMouseUp);
    };
  }, [dragging, leftMin, leftMax, rightMin, rightMax, onLeftResize, onRightResize]);

  return (
    <div className="flex-1 flex min-h-0">
      <div style={{ width: leftWidth }} className="shrink-0 bg-card border-r overflow-hidden">
        {children[0]}
      </div>

      {/* Left resize handle */}
      <div
        className="w-1 shrink-0 cursor-col-resize hover:bg-element-bg-soft active:bg-primary/50 transition-colors relative group"
        onMouseDown={onMouseDown("left")}
      >
        {dragging === "left" && (
          <div className="fixed inset-0 z-50 pointer-events-none">
            <div
              className="absolute top-0 bottom-0 w-0.5 bg-primary/40"
              style={{ left: leftWidth + 1 }}
            />
          </div>
        )}
      </div>

      <div className="flex-1 min-w-0 bg-card flex flex-col overflow-hidden">
        {children[1]}
      </div>

      {/* Right resize handle */}
      <div
        className="w-1 shrink-0 cursor-col-resize hover:bg-element-bg-soft active:bg-primary/50 transition-colors relative group"
        onMouseDown={onMouseDown("right")}
      >
        {dragging === "right" && (
          <div className="fixed inset-0 z-50 pointer-events-none">
            <div
              className="absolute top-0 bottom-0 w-0.5 bg-primary/40"
              style={{ right: rightWidth + 1 }}
            />
          </div>
        )}
      </div>

      <div style={{ width: rightWidth }} className="shrink-0 bg-card border-l overflow-hidden">
        {children[2]}
      </div>
    </div>
  );
}
