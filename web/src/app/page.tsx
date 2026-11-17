'use client';

import { useState, useEffect } from 'react';
import { ClusterOverview } from '@/components/ClusterOverview';
import { NodeList } from '@/components/NodeList';
import { KeyValueStore } from '@/components/KeyValueStore';
import { LogsPanel } from '@/components/LogsPanel';
import { Sidebar } from '@/components/Sidebar';
import { Header } from '@/components/Header';

type Tab = 'overview' | 'nodes' | 'kv-store' | 'logs';

export default function Dashboard() {
  const [activeTab, setActiveTab] = useState<Tab>('overview');
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [isDark, setIsDark] = useState(false);

  useEffect(() => {
    const saved = localStorage.getItem('theme');
    if (saved) setIsDark(saved === 'dark');
    else if (window.matchMedia('(prefers-color-scheme: dark)').matches) setIsDark(true);
  }, []);

  useEffect(() => {
    document.documentElement.classList.toggle('dark', isDark);
    localStorage.setItem('theme', isDark ? 'dark' : 'light');
  }, [isDark]);

  return (
    <div className="min-h-screen bg-background flex">
      <Sidebar
        activeTab={activeTab}
        onTabChange={setActiveTab}
        open={sidebarOpen}
        onClose={() => setSidebarOpen(false)}
      />
      <div className="flex-1 flex flex-col min-w-0">
        <Header
          onMenuClick={() => setSidebarOpen(true)}
          isDark={isDark}
          onThemeToggle={() => setIsDark(!isDark)}
        />
        <main className="flex-1 p-4 md:p-6 overflow-auto">
          {activeTab === 'overview' && <ClusterOverview />}
          {activeTab === 'nodes' && <NodeList />}
          {activeTab === 'kv-store' && <KeyValueStore />}
          {activeTab === 'logs' && <LogsPanel />}
        </main>
      </div>
    </div>
  );
}
