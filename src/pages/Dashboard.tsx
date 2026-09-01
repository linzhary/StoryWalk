import { useCallback, useEffect, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { Plus, Loader2, Pencil, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { getStories, createStory, updateStory, deleteStory } from "@/lib/api";
import type { Story } from "@/lib/mock-data";

export default function Dashboard() {
  const navigate = useNavigate();
  const [stories, setStories] = useState<Story[]>([]);
  const [loading, setLoading] = useState(true);
  const [dialogOpen, setDialogOpen] = useState(false);
  const [editingStory, setEditingStory] = useState<Story | null>(null);
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [style, setStyle] = useState<"modern" | "ancient">("modern");
  const [storyMode, setStoryMode] = useState<"card" | "chat">("card");
  const [saving, setSaving] = useState(false);
  const [deleteConfirmId, setDeleteConfirmId] = useState<string | null>(null);
  const [deleting, setDeleting] = useState(false);

  const loadStories = useCallback(async () => {
    try {
      const data = await getStories();
      setStories(data);
    } catch (e) {
      console.error(e);
    }
    setLoading(false);
  }, []);

  useEffect(() => { loadStories(); }, [loadStories]);

  const handleOpenCreate = () => {
    setEditingStory(null);
    setTitle("");
    setDescription("");
    setStyle("modern");
    setStoryMode("card");
    setDialogOpen(true);
  };

  const handleOpenEdit = (story: Story) => {
    setEditingStory(story);
    setTitle(story.title);
    setDescription(story.description);
    setDialogOpen(true);
  };

  const handleSave = async () => {
    if (!title.trim()) return;
    setSaving(true);
    try {
      if (editingStory) {
        await updateStory(editingStory.id, { title: title.trim(), description });
      } else {
        const newStory = await createStory({ title: title.trim(), description, style, mode: storyMode });
        setDialogOpen(false);
        setTitle("");
        setDescription("");
        setSaving(false);
        navigate(`/stories/${newStory.id}`);
        return;
      }
      setDialogOpen(false);
      setTitle("");
      setDescription("");
      await loadStories();
    } catch (e) {
      console.error(e);
    }
    setSaving(false);
  };

  const handleDelete = async () => {
    if (!deleteConfirmId) return;
    setDeleting(true);
    try {
      await deleteStory(deleteConfirmId);
      setDeleteConfirmId(null);
      await loadStories();
    } catch (e) {
      console.error(e);
    }
    setDeleting(false);
  };

  const isEdit = !!editingStory;

  return (
    <div className="flex-1 overflow-y-auto">
      <div className="px-8 pt-6 pb-8">
        {/* Header */}
        <div className="flex items-baseline gap-2.5 mb-5">
          <h1 className="text-[15px] font-semibold text-foreground">故事列表</h1>
          <p className="text-xs text-muted-foreground/70">
            {loading ? "加载中..." : `共 ${stories.length} 个故事`}
          </p>
        </div>

        {/* Empty state */}
        {!loading && stories.length === 0 && (
          <p className="text-xs text-muted-foreground/60 mb-4">
            还没有故事,点击下方"新建故事"卡片创建你的第一个故事。
          </p>
        )}

        {/* Grid */}
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
          {loading
            ? Array.from({ length: 3 }).map((_, i) => (
                <div key={i} className="h-32 bg-muted/50 rounded-lg animate-pulse" />
              ))
            : stories.map((story) => (
                <div key={story.id} className="group relative h-32 rounded-lg border border-border/50 bg-card p-4 transition-colors duration-150 hover:border-primary/25 hover:bg-muted/30 overflow-hidden">
                  <Link to={`/stories/${story.id}`} className="block h-full">
                    {/* Content */}
                    <div className="h-full flex flex-col">
                      <h3 className="font-medium text-[14px] leading-snug mb-1.5 text-foreground/85 group-hover:text-primary transition-colors duration-150">
                        {story.title}
                      </h3>
                      <div className="text-xs text-muted-foreground/70 leading-relaxed line-clamp-2 flex-1">
                        {story.description || "暂无简介"}
                      </div>
                    </div>
                  </Link>
                  {/* Edit / Delete buttons */}
                  <div className="absolute top-1.5 right-1.5 z-20 flex gap-0.5 opacity-0 group-hover:opacity-100 transition-opacity duration-150">
                    <button
                      onClick={(e) => { e.stopPropagation(); e.preventDefault(); handleOpenEdit(story); }}
                      className="p-1 rounded text-muted-foreground/40 hover:text-foreground hover:bg-muted/60 transition-colors"
                      title="编辑"
                    >
                      <Pencil className="h-3.5 w-3.5" />
                    </button>
                    <button
                      onClick={(e) => { e.stopPropagation(); e.preventDefault(); setDeleteConfirmId(story.id); }}
                      className="p-1 rounded text-muted-foreground/40 hover:text-destructive hover:bg-destructive/10 transition-colors"
                      title="删除"
                    >
                      <Trash2 className="h-3.5 w-3.5" />
                    </button>
                  </div>
                </div>
              ))}

          {/* Create card */}
          <button
            onClick={handleOpenCreate}
            className="h-32 rounded-lg border border-dashed border-border/60 hover:border-primary/30 hover:bg-primary/[0.02] transition-colors flex items-center justify-center group"
          >
            <div className="text-center">
              <div className="w-9 h-9 rounded-full bg-muted/40 group-hover:bg-primary/10 transition-colors flex items-center justify-center mx-auto mb-2.5">
                <Plus className="h-4 w-4 text-muted-foreground/40 group-hover:text-primary/60 transition-colors" />
              </div>
              <div className="text-xs text-muted-foreground/50 group-hover:text-muted-foreground/70 transition-colors">
                新建故事
              </div>
            </div>
          </button>
        </div>

        {/* Create / Edit Dialog */}
        <Dialog open={dialogOpen} onOpenChange={setDialogOpen}>
          <DialogContent className="sm:max-w-[420px] p-6" showCloseButton={false}>
            <DialogHeader>
              <DialogTitle className="text-[16px]">{isEdit ? "编辑故事" : "新建故事"}</DialogTitle>
            </DialogHeader>
            <div className="space-y-4 pt-3">
              <div>
                <label className="text-[12px] font-medium text-muted-foreground/70 mb-1.5 block">标题</label>
                <Input
                  placeholder="输入故事标题..."
                  value={title}
                  onChange={(e) => setTitle(e.target.value)}
                  onKeyDown={(e) => { if (e.key === "Enter") handleSave(); }}
                  className="h-9 text-[14px]"
                  autoFocus
                />
              </div>
              <div>
                <label className="text-[12px] font-medium text-muted-foreground/70 mb-1.5 block">简介（可选）</label>
                <Textarea
                  placeholder="简要描述你的故事..."
                  value={description}
                  onChange={(e) => setDescription(e.target.value)}
                  className="min-h-[90px] text-[14px]"
                />
              </div>
              {!isEdit && (
                <div>
                  <label className="text-[12px] font-medium text-muted-foreground/70 mb-1.5 block">故事模式</label>
                  <div className="flex gap-2">
                    {([
                      { key: "card", label: "写卡", desc: "正文沉淀为剧情卡片" },
                      { key: "chat", label: "游戏", desc: "叙事者+NPC 驱动的 RPG，直接演出" },
                    ] as const).map(m => (
                      <button
                        key={m.key}
                        type="button"
                        onClick={() => setStoryMode(m.key)}
                        className={`flex-1 h-9 rounded-lg border text-[12px] font-medium transition-colors ${
                          storyMode === m.key
                            ? "border-primary/50 bg-primary/10 text-primary"
                            : "border-border/60 text-muted-foreground/70 hover:bg-muted/60"
                        }`}
                        title={m.desc}
                      >
                        {m.label}
                      </button>
                    ))}
                  </div>
                  <p className="text-[11px] text-muted-foreground/50 mt-1.5">
                    {storyMode === "card" ? "剧情正文写入左侧卡片，素材自动沉淀" : "AI 担任叙事者与 NPC，剧情直接在聊天中演出，素材沉淀由「素材沉淀」按钮手动触发"}
                  </p>
                </div>
              )}
              {!isEdit && (
                <div>
                  <label className="text-[12px] font-medium text-muted-foreground/70 mb-1.5 block">文风</label>
                  <div className="flex gap-2">
                    {(["modern", "ancient"] as const).map(s => (
                      <button
                        key={s}
                        type="button"
                        onClick={() => setStyle(s)}
                        className={`flex-1 h-9 rounded-lg border text-[12px] font-medium transition-colors ${
                          style === s
                            ? "border-primary/50 bg-primary/10 text-primary"
                            : "border-border/60 text-muted-foreground/70 hover:bg-muted/60"
                        }`}
                      >
                        {s === "modern" ? "现代" : "古代"}
                      </button>
                    ))}
                  </div>
                  <p className="text-[11px] text-muted-foreground/50 mt-1.5">将作为初始创作准则写入该故事的 guidelines.md</p>
                </div>
              )}
            </div>
            <div className="flex justify-end gap-2 pt-5 border-t mt-5">
              <Button variant="outline" size="sm" className="h-8 text-[12px] px-4" onClick={() => setDialogOpen(false)}>取消</Button>
              <Button size="sm" className="h-8 text-[12px] px-4" onClick={handleSave} disabled={saving || !title.trim()}>
                {saving ? <><Loader2 className="h-3.5 w-3.5 mr-1 animate-spin" />保存中...</> : isEdit ? "保存" : "创建故事"}
              </Button>
            </div>
          </DialogContent>
        </Dialog>

        {/* Delete Confirm Dialog */}
        <Dialog open={!!deleteConfirmId} onOpenChange={() => !deleting && setDeleteConfirmId(null)}>
          <DialogContent className="sm:max-w-[360px] p-6" showCloseButton={false}>
            <DialogHeader>
              <DialogTitle className="text-[16px]">确认删除</DialogTitle>
            </DialogHeader>
            <p className="text-sm text-muted-foreground/70 pt-1 leading-relaxed">
              确定要删除「{stories.find((s) => s.id === deleteConfirmId)?.title || ""}」吗？故事下的所有内容和资料将被永久删除。
            </p>
            <div className="flex justify-end gap-2 pt-5 border-t mt-5">
              <Button variant="outline" size="sm" className="h-8 text-[12px] px-4" onClick={() => setDeleteConfirmId(null)} disabled={deleting}>取消</Button>
              <Button variant="destructive" size="sm" className="h-8 text-[12px] px-4" onClick={handleDelete} disabled={deleting}>
                {deleting ? <><Loader2 className="h-3.5 w-3.5 mr-1 animate-spin" />删除中...</> : "删除"}
              </Button>
            </div>
          </DialogContent>
        </Dialog>
      </div>
    </div>
  );
}
