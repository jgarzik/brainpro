import { Outlet } from "react-router-dom";
import { useEffect, useRef } from "react";
import { Sidebar } from "./Sidebar";
import { Header } from "./Header";
import { PasswordDialog } from "./PasswordDialog";
import { ToastContainer } from "@/components/ui/Toast";
import { useConnectionStore } from "@/store/connectionStore";
import { useUIStore } from "@/store/uiStore";

export function AppShell() {
  const { state, connect } = useConnectionStore();
  const setPasswordDialogOpen = useUIStore((s) => s.setPasswordDialogOpen);
  const connectAttempted = useRef(false);

  // Auto-connect on mount
  useEffect(() => {
    if (state === "disconnected" && !connectAttempted.current) {
      connectAttempted.current = true;
      connect().catch(() => {
        // If connection fails, show password dialog
        setPasswordDialogOpen(true);
      });
    }
  }, [state, connect, setPasswordDialogOpen]);

  return (
    <div className="flex h-screen overflow-hidden bg-gray-50 dark:bg-gray-950">
      <Sidebar />
      <div className="flex flex-1 flex-col overflow-hidden">
        <Header />
        <main className="flex-1 overflow-auto p-6">
          <Outlet />
        </main>
      </div>

      <PasswordDialog />
      <ToastContainer />
    </div>
  );
}
