import { NavLink } from "react-router-dom";
import { clsx } from "clsx";
import {
  MessageSquare,
  LayoutDashboard,
  Layers,
  Wrench,
  Sparkles,
  Bot,
  Radio,
  Activity,
  Zap,
  DollarSign,
  Settings,
  Bug,
  ChevronLeft,
  ChevronRight,
} from "lucide-react";
import { ROUTES } from "@/constants/routes";
import { useUIStore } from "@/store/uiStore";

interface NavItem {
  to: string;
  label: string;
  icon: React.ComponentType<{ className?: string }>;
}

const mainNav: NavItem[] = [
  { to: ROUTES.CHAT, label: "Chat", icon: MessageSquare },
  { to: ROUTES.OVERVIEW, label: "Overview", icon: LayoutDashboard },
  { to: ROUTES.SESSIONS, label: "Sessions", icon: Layers },
];

const toolsNav: NavItem[] = [
  { to: ROUTES.TOOLS, label: "Tools", icon: Wrench },
  { to: ROUTES.SKILLS, label: "Skills", icon: Sparkles },
  { to: ROUTES.AGENTS, label: "Agents", icon: Bot },
];

const observabilityNav: NavItem[] = [
  { to: ROUTES.HEALTH, label: "Health", icon: Activity },
  { to: ROUTES.EVENTS, label: "Events", icon: Zap },
  { to: ROUTES.COSTS, label: "Costs", icon: DollarSign },
  { to: ROUTES.CHANNELS, label: "Channels", icon: Radio },
];

const systemNav: NavItem[] = [
  { to: ROUTES.CONFIG, label: "Config", icon: Settings },
  { to: ROUTES.DEBUG, label: "Debug", icon: Bug },
];

function NavGroup({
  title,
  items,
  collapsed,
}: {
  title: string;
  items: NavItem[];
  collapsed: boolean;
}) {
  return (
    <div className="mb-6">
      {!collapsed && (
        <h3 className="mb-2 px-3 text-xs font-semibold uppercase tracking-wider text-gray-400 dark:text-gray-500">
          {title}
        </h3>
      )}
      <nav className="space-y-1">
        {items.map((item) => (
          <NavLink
            key={item.to}
            to={item.to}
            className={({ isActive }) =>
              clsx(
                "flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors",
                isActive
                  ? "bg-blue-50 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400"
                  : "text-gray-700 hover:bg-gray-100 dark:text-gray-300 dark:hover:bg-gray-800",
                collapsed && "justify-center",
              )
            }
            title={collapsed ? item.label : undefined}
          >
            <item.icon className="h-5 w-5 flex-shrink-0" />
            {!collapsed && <span>{item.label}</span>}
          </NavLink>
        ))}
      </nav>
    </div>
  );
}

export function Sidebar() {
  const collapsed = useUIStore((s) => s.sidebarCollapsed);
  const toggleSidebar = useUIStore((s) => s.toggleSidebar);

  return (
    <aside
      className={clsx(
        "flex flex-col border-r border-gray-200 bg-white transition-all duration-200 dark:border-gray-700 dark:bg-gray-900",
        collapsed ? "w-16" : "w-60",
      )}
    >
      {/* Logo */}
      <div
        className={clsx(
          "flex h-14 items-center border-b border-gray-200 px-4 dark:border-gray-700",
          collapsed ? "justify-center" : "justify-between",
        )}
      >
        {!collapsed && (
          <span className="text-lg font-bold text-gray-900 dark:text-gray-100">
            BrainPro
          </span>
        )}
        <button
          onClick={toggleSidebar}
          className="rounded p-1.5 text-gray-400 hover:bg-gray-100 hover:text-gray-600 dark:hover:bg-gray-800 dark:hover:text-gray-200"
          title={collapsed ? "Expand sidebar" : "Collapse sidebar"}
        >
          {collapsed ? (
            <ChevronRight className="h-4 w-4" />
          ) : (
            <ChevronLeft className="h-4 w-4" />
          )}
        </button>
      </div>

      {/* Navigation */}
      <div className="flex-1 overflow-y-auto px-2 py-4">
        <NavGroup title="Main" items={mainNav} collapsed={collapsed} />
        <NavGroup title="Tools" items={toolsNav} collapsed={collapsed} />
        <NavGroup
          title="Observability"
          items={observabilityNav}
          collapsed={collapsed}
        />
        <NavGroup title="System" items={systemNav} collapsed={collapsed} />
      </div>
    </aside>
  );
}
