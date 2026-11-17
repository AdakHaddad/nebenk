'use client';

import { useState } from 'react';
import Link from 'next/link';
import { usePathname } from 'next/navigation';
import { 
  LayoutDashboard, 
  Server, 
  Database, 
  Terminal,
  ChevronLeft,
  ChevronRight,
  X
} from 'lucide-react';
import { cn } from '@/lib/utils';

type Tab = 'overview' | 'nodes' | 'kv-store' | 'logs';

const tabs: { id: Tab; label: string; icon: React.ReactNode; href: string }[] = [
  { id: 'overview', label: 'Overview', icon: <LayoutDashboard className="h-5 w-5" />, href: '/' },
  { id: 'nodes', label: 'Nodes', icon: <Server className="h-5 w-5" />, href: '/nodes' },
  { id: 'kv-store', label: 'KV Store', icon: <Database className="h-5 w-5" />, href: '/kv-store' },
  { id: 'logs', label: 'Logs', icon: <Terminal className="h-5 w-5" />, href: '/logs' },
];

interface SidebarProps {
  activeTab: Tab;
  onTabChange: (tab: Tab) => void;
  open: boolean;
  onClose: () => void;
}

export function Sidebar({ activeTab, onTabChange, open, onClose }: SidebarProps) {
  const [collapsed, setCollapsed] = useState(false);
  const pathname = usePathname();

  if (!open && typeof window !== 'undefined') return null;

  return (
    <>
      {open && (
        <div 
          className="fixed inset-0 bg-black/50 z-40 lg:hidden"
          onClick={onClose}
          aria-hidden="true"
        />
      )}
      <aside
        className={cn(
          'fixed lg:static inset-y-0 left-0 z-50 bg-card border-r border-border',
          'flex flex-col transition-all duration-200 ease-in-out',
          collapsed ? 'w-16' : 'w-64',
          open ? 'translate-x-0' : '-translate-x-full lg:translate-x-0'
        )}
        style={{ minWidth: collapsed ? '64px' : '256px' }}
      >
        <div className="flex h-16 items-center justify-between px-4 border-b border-border">
          {!collapsed && (
            <Link href="/" className="flex items-center gap-2 font-semibold text-lg">
              <span className="text-primary">NEBENK</span>
            </Link>
          )}
          <button
            onClick={() => setCollapsed(!collapsed)}
            className="p-1 rounded hover:bg-accent transition-colors"
            aria-label={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
          >
            {collapsed ? <ChevronRight className="h-5 w-5" /> : <ChevronLeft className="h-5 w-5" />}
          </button>
        </div>

        <nav className="flex-1 px-3 py-4 space-y-1 overflow-y-auto" aria-label="Main navigation">
          {tabs.map((tab) => {
            const isActive = activeTab === tab.id;
            return (
              <button
                key={tab.id}
                onClick={() => { onTabChange(tab.id); onClose(); }}
                className={cn(
                  'flex items-center gap-3 px-3 py-2 rounded-lg text-sm font-medium transition-all',
                  'group w-full justify-start',
                  isActive
                    ? 'bg-primary text-primary-foreground shadow-sm'
                    : 'text-muted-foreground hover:bg-accent hover:text-accent-foreground',
                  collapsed && 'justify-center px-2'
                )}
                title={collapsed ? tab.label : undefined}
                aria-current={isActive ? 'page' : undefined}
              >
                <span className="flex-shrink-0">{tab.icon}</span>
                {!collapsed && <span>{tab.label}</span>}
              </button>
            );
          })}
        </nav>

        <div className="p-4 border-t border-border">
          {!collapsed && (
            <div className="text-xs text-muted-foreground space-y-1">
              <p className="font-medium">Cluster Status</p>
              <div className="flex items-center gap-2">
                <span className="h-2 w-2 rounded-full bg-green-500" />
                <span>Healthy</span>
              </div>
              <p className="mt-2">v0.1.0 • MVP</p>
            </div>
          )}
        </div>
      </aside>
    </>
  );
}
