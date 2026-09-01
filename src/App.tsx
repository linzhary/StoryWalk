import { Routes, Route } from "react-router-dom";
import AppToolbar from "@/components/app-toolbar";
import Dashboard from "@/pages/Dashboard";
import StoryPage from "@/pages/StoryPage";

export default function App() {
  return (
    <div className="h-dvh flex flex-col bg-background">
      <AppToolbar />
      <Routes>
        <Route path="/" element={<Dashboard />} />
        <Route path="/stories/:id" element={<StoryPage />} />
      </Routes>
    </div>
  );
}
