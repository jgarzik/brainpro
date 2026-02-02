import { clsx } from "clsx";
import { Wifi, WifiOff, Moon, Sun, RefreshCw } from "lucide-react";
import { useConnectionStore } from "@/store/connectionStore";
import { useUIStore } from "@/store/uiStore";

export function Header() {
  const { state, sessionId, connect, disconnect } = useConnectionStore();
  const { theme, setTheme, effectiveTheme } = useUIStore();

  const isConnected = state === "connected";
  const isConnecting = state === "connecting" || state === "authenticating";

  const toggleTheme = () => {
    if (theme === "light") {
      setTheme("dark");
    } else if (theme === "dark") {
      setTheme("system");
    } else {
      setTheme("light");
    }
  };

  const handleConnectionClick = () => {
    if (isConnected) {
      disconnect();
    } else if (!isConnecting) {
      connect().catch(() => {
        // Error handled in store
      });
    }
  };

  return (
    <header className="flex h-14 items-center justify-between border-b border-gray-200 bg-white px-4 dark:border-gray-700 dark:bg-gray-900">
      {/* Left: Session info */}
      <div className="flex items-center gap-4">
        {sessionId && (
          <span className="text-sm text-gray-500 dark:text-gray-400">
            Session:{" "}
            <code className="font-mono text-xs">{sessionId.slice(0, 8)}</code>
          </span>
        )}
      </div>

      {/* Right: Actions */}
      <div className="flex items-center gap-3">
        {/* Connection status */}
        <button
          onClick={handleConnectionClick}
          className={clsx(
            "flex items-center gap-2 rounded-md px-3 py-1.5 text-sm font-medium transition-colors",
            isConnected
              ? "text-emerald-700 hover:bg-emerald-50 dark:text-emerald-400 dark:hover:bg-emerald-900/20"
              : "text-gray-500 hover:bg-gray-100 dark:text-gray-400 dark:hover:bg-gray-800",
          )}
        >
          {isConnecting ? (
            <RefreshCw className="h-4 w-4 animate-spin" />
          ) : isConnected ? (
            <Wifi className="h-4 w-4" />
          ) : (
            <WifiOff className="h-4 w-4" />
          )}
          <span>
            {isConnecting
              ? "Connecting..."
              : isConnected
                ? "Connected"
                : "Disconnected"}
          </span>
        </button>

        {/* Theme toggle */}
        <button
          onClick={toggleTheme}
          className="rounded-md p-2 text-gray-500 hover:bg-gray-100 dark:text-gray-400 dark:hover:bg-gray-800"
          title={`Theme: ${theme}`}
        >
          {effectiveTheme === "dark" ? (
            <Moon className="h-5 w-5" />
          ) : (
            <Sun className="h-5 w-5" />
          )}
        </button>
      </div>
    </header>
  );
}
