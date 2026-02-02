/**
 * UI state store
 */

import { create } from "zustand";
import { persist } from "zustand/middleware";

/** Toast notification */
export interface Toast {
  id: string;
  type: "success" | "error" | "warning" | "info";
  message: string;
  duration?: number | undefined;
}

interface UIStore {
  // Sidebar state
  sidebarCollapsed: boolean;

  // Theme
  theme: "light" | "dark" | "system";
  effectiveTheme: "light" | "dark";

  // Toasts
  toasts: Toast[];

  // Modal state
  passwordDialogOpen: boolean;

  // Actions
  toggleSidebar: () => void;
  setSidebarCollapsed: (collapsed: boolean) => void;

  setTheme: (theme: "light" | "dark" | "system") => void;
  updateEffectiveTheme: () => void;

  addToast: (toast: Omit<Toast, "id">) => string;
  removeToast: (id: string) => void;
  clearToasts: () => void;

  setPasswordDialogOpen: (open: boolean) => void;
}

function getSystemTheme(): "light" | "dark" {
  if (typeof window === "undefined") return "light";
  return window.matchMedia("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
}

export const useUIStore = create<UIStore>()(
  persist(
    (set, get) => ({
      sidebarCollapsed: false,
      theme: "system",
      effectiveTheme: getSystemTheme(),
      toasts: [],
      passwordDialogOpen: false,

      toggleSidebar: () =>
        set((state) => ({ sidebarCollapsed: !state.sidebarCollapsed })),

      setSidebarCollapsed: (collapsed) => set({ sidebarCollapsed: collapsed }),

      setTheme: (theme) => {
        set({ theme });
        get().updateEffectiveTheme();
      },

      updateEffectiveTheme: () => {
        const { theme } = get();
        const effective = theme === "system" ? getSystemTheme() : theme;
        set({ effectiveTheme: effective });

        // Apply to document
        if (typeof document !== "undefined") {
          document.documentElement.classList.toggle(
            "dark",
            effective === "dark",
          );
        }
      },

      addToast: (toast) => {
        const id = crypto.randomUUID();
        set((state) => ({
          toasts: [...state.toasts, { ...toast, id }],
        }));
        return id;
      },

      removeToast: (id) =>
        set((state) => ({
          toasts: state.toasts.filter((t) => t.id !== id),
        })),

      clearToasts: () => set({ toasts: [] }),

      setPasswordDialogOpen: (open) => set({ passwordDialogOpen: open }),
    }),
    {
      name: "brainpro-ui",
      partialize: (state) => ({
        sidebarCollapsed: state.sidebarCollapsed,
        theme: state.theme,
      }),
    },
  ),
);

// Initialize theme on load
if (typeof window !== "undefined") {
  useUIStore.getState().updateEffectiveTheme();

  // Listen for system theme changes
  window
    .matchMedia("(prefers-color-scheme: dark)")
    .addEventListener("change", () => {
      useUIStore.getState().updateEffectiveTheme();
    });
}
