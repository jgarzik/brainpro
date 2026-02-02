import { useUIStore } from "@/store/uiStore";

/** Hook for showing toasts */
export function useToast() {
  const addToast = useUIStore((s) => s.addToast);

  return {
    success: (message: string, duration?: number) =>
      addToast({ type: "success", message, duration }),
    error: (message: string, duration?: number) =>
      addToast({ type: "error", message, duration }),
    warning: (message: string, duration?: number) =>
      addToast({ type: "warning", message, duration }),
    info: (message: string, duration?: number) =>
      addToast({ type: "info", message, duration }),
  };
}
