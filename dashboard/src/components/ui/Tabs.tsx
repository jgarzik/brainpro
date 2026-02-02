import { clsx } from "clsx";

export interface TabItem {
  key: string;
  label: string;
  disabled?: boolean;
}

export interface TabsProps {
  items: TabItem[];
  activeKey: string;
  onChange: (key: string) => void;
  className?: string;
}

export function Tabs({ items, activeKey, onChange, className }: TabsProps) {
  return (
    <div
      className={clsx(
        "border-b border-gray-200 dark:border-gray-700",
        className,
      )}
    >
      <nav className="-mb-px flex space-x-6" aria-label="Tabs">
        {items.map((item) => (
          <button
            key={item.key}
            onClick={() => !item.disabled && onChange(item.key)}
            disabled={item.disabled}
            className={clsx(
              "whitespace-nowrap border-b-2 px-1 py-3 text-sm font-medium transition-colors",
              item.key === activeKey
                ? "border-blue-500 text-blue-600 dark:border-blue-400 dark:text-blue-400"
                : "border-transparent text-gray-500 hover:border-gray-300 hover:text-gray-700 dark:text-gray-400 dark:hover:border-gray-600 dark:hover:text-gray-300",
              item.disabled && "cursor-not-allowed opacity-50",
            )}
          >
            {item.label}
          </button>
        ))}
      </nav>
    </div>
  );
}

/** Tab panel for content */
export interface TabPanelProps {
  active: boolean;
  children: React.ReactNode;
  className?: string;
}

export function TabPanel({ active, children, className }: TabPanelProps) {
  if (!active) return null;
  return <div className={clsx("pt-4", className)}>{children}</div>;
}
