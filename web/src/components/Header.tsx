'use client';

import { Menu, Sun, Moon, Bell, Settings, User } from 'lucide-react';
import { cn } from '@/lib/utils';

interface HeaderProps {
  onMenuClick: () => void;
  isDark: boolean;
  onThemeToggle: () => void;
}

export function Header({ onMenuClick, isDark, onThemeToggle }: HeaderProps) {
  return (
    <header className={cn(
      'sticky top-0 z-30 flex h-16 items-center gap-4 border-b border-border bg-background/95',
      'backdrop-blur supports-[backdrop-filter]:bg-background/60',
      'px-4 md:px-6'
    )}>
      <button
        onClick={onMenuClick}
        className="lg:hidden p-2 rounded-lg hover:bg-accent transition-colors"
        aria-label="Open menu"
      >
        <Menu className="h-5 w-5" />
      </button>

      <div className="flex-1" />

      <div className="flex items-center gap-2">
        <button
          onClick={onThemeToggle}
          className="p-2 rounded-lg hover:bg-accent transition-colors"
          aria-label={isDark ? 'Switch to light mode' : 'Switch to dark mode'}
        >
          {isDark ? <Sun className="h-5 w-5" /> : <Moon className="h-5 w-5" />}
        </button>

        <button className="p-2 rounded-lg hover:bg-accent transition-colors relative" aria-label="Notifications">
          <Bell className="h-5 w-5" />
          <span className="absolute top-1 right-1 h-2 w-2 rounded-full bg-red-500" />
        </button>

        <div className="w-px h-6 bg-border mx-1 hidden sm:block" />

        <button className="p-2 rounded-lg hover:bg-accent transition-colors" aria-label="Settings">
          <Settings className="h-5 w-5" />
        </button>

        <button className="p-2 rounded-lg hover:bg-accent transition-colors" aria-label="User menu">
          <User className="h-5 w-5" />
        </button>
      </div>
    </header>
  );
}
