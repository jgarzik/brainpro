import { describe, it, expect, beforeEach } from "vitest";
import { useUIStore } from "@/store/uiStore";

describe("uiStore", () => {
  beforeEach(() => {
    // Reset to default state
    useUIStore.setState({
      sidebarCollapsed: false,
      theme: "system",
      effectiveTheme: "light",
      toasts: [],
      passwordDialogOpen: false,
    });
  });

  describe("theme", () => {
    it("setTheme updates theme state", () => {
      const store = useUIStore.getState();

      store.setTheme("dark");
      expect(useUIStore.getState().theme).toBe("dark");

      store.setTheme("light");
      expect(useUIStore.getState().theme).toBe("light");

      store.setTheme("system");
      expect(useUIStore.getState().theme).toBe("system");
    });
  });

  describe("sidebar", () => {
    it("toggleSidebar flips collapsed state", () => {
      const store = useUIStore.getState();
      expect(store.sidebarCollapsed).toBe(false);

      store.toggleSidebar();
      expect(useUIStore.getState().sidebarCollapsed).toBe(true);

      store.toggleSidebar();
      expect(useUIStore.getState().sidebarCollapsed).toBe(false);
    });

    it("setSidebarCollapsed sets explicit state", () => {
      const store = useUIStore.getState();

      store.setSidebarCollapsed(true);
      expect(useUIStore.getState().sidebarCollapsed).toBe(true);

      store.setSidebarCollapsed(false);
      expect(useUIStore.getState().sidebarCollapsed).toBe(false);
    });
  });

  describe("toasts", () => {
    it("addToast adds a toast with generated id", () => {
      const store = useUIStore.getState();
      const id = store.addToast({ type: "success", message: "Test message" });

      expect(id).toBeTruthy();
      const state = useUIStore.getState();
      expect(state.toasts.length).toBe(1);
      expect(state.toasts[0]?.id).toBe(id);
      expect(state.toasts[0]?.type).toBe("success");
      expect(state.toasts[0]?.message).toBe("Test message");
    });

    it("removeToast removes specific toast", () => {
      const store = useUIStore.getState();
      const id1 = store.addToast({ type: "info", message: "First" });
      const id2 = store.addToast({ type: "error", message: "Second" });

      expect(useUIStore.getState().toasts.length).toBe(2);

      store.removeToast(id1);

      const state = useUIStore.getState();
      expect(state.toasts.length).toBe(1);
      expect(state.toasts[0]?.id).toBe(id2);
    });

    it("clearToasts removes all toasts", () => {
      const store = useUIStore.getState();
      store.addToast({ type: "info", message: "First" });
      store.addToast({ type: "error", message: "Second" });
      store.addToast({ type: "warning", message: "Third" });

      expect(useUIStore.getState().toasts.length).toBe(3);

      store.clearToasts();

      expect(useUIStore.getState().toasts.length).toBe(0);
    });
  });

  describe("passwordDialog", () => {
    it("setPasswordDialogOpen updates state", () => {
      const store = useUIStore.getState();

      store.setPasswordDialogOpen(true);
      expect(useUIStore.getState().passwordDialogOpen).toBe(true);

      store.setPasswordDialogOpen(false);
      expect(useUIStore.getState().passwordDialogOpen).toBe(false);
    });
  });
});
