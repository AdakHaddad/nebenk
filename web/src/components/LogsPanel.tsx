'use client';

import { useState, useEffect, useRef } from 'react';
import { 
  Terminal, 
  Filter,
  Download,
  Trash2,
  Pause,
  Play,
  Search,
  ChevronDown,
  ChevronUp
} from 'lucide-react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { cn } from '@/lib/utils';

type LogLevel = 'debug' | 'info' | 'warn' | 'error';
type LogSource = 'membership' | 'replication' | 'storage' | 'network' | 'runtime' | 'wasm';

interface LogEntry {
  id: number;
  timestamp: string;
  level: LogLevel;
  source: LogSource;
  message: string;
  nodeId?: string;
}

const mockLogs: LogEntry[] = [
  { id: 1, timestamp: '2024-11-17 10:23:45.123', level: 'info', source: 'membership', message: 'Node node-3 marked as synced at revision 15420', nodeId: 'node-1' },
  { id: 2, timestamp: '2024-11-17 10:23:44.891', level: 'info', source: 'replication', message: 'Snapshot created successfully for revision 15420 (size: 2.1MB)', nodeId: 'node-1' },
  { id: 3, timestamp: '2024-11-17 10:23:44.102', level: 'info', source: 'runtime', message: 'Applied command PUT user:1003 at revision 15420', nodeId: 'node-1' },
  { id: 4, timestamp: '2024-11-17 10:23:43.567', level: 'debug', source: 'storage', message: 'WAL append: operation 15420 written to disk', nodeId: 'node-1' },
  { id: 5, timestamp: '2024-11-17 10:23:42.234', level: 'info', source: 'network', message: 'Heartbeat received from node-2 (phi=0.12)', nodeId: 'node-1' },
  { id: 6, timestamp: '2024-11-17 10:23:41.890', level: 'info', source: 'network', message: 'Heartbeat received from node-3 (phi=0.08)', nodeId: 'node-1' },
  { id: 7, timestamp: '2024-11-17 10:23:40.123', level: 'warn', source: 'membership', message: 'Node node-3 phi increased to 8.5 (suspected)', nodeId: 'node-1' },
  { id: 8, timestamp: '2024-11-17 10:23:39.456', level: 'info', source: 'replication', message: 'Sync request from node-3: last_known_revision=15400', nodeId: 'node-1' },
  { id: 9, timestamp: '2024-11-17 10:23:38.789', level: 'debug', source: 'storage', message: 'Snapshot load: revision 15400, hash verified', nodeId: 'node-3' },
  { id: 10, timestamp: '2024-11-17 10:23:37.234', level: 'info', source: 'runtime', message: 'Node recovered from WAL replay: 20 operations applied', nodeId: 'node-3' },
  { id: 11, timestamp: '2024-11-17 10:23:35.678', level: 'error', source: 'network', message: 'Connection to node-2 lost, retrying...', nodeId: 'node-1' },
  { id: 12, timestamp: '2024-11-17 10:23:34.123', level: 'warn', source: 'membership', message: 'Node node-2 phi=12.3 (failed threshold exceeded)', nodeId: 'node-1' },
  { id: 13, timestamp: '2024-11-17 10:23:30.456', level: 'info', source: 'membership', message: 'Failover initiated: promoting node-3 to primary', nodeId: 'node-1' },
  { id: 14, timestamp: '2024-11-17 10:23:29.890', level: 'info', source: 'replication', message: 'Live replication stream started to node-3', nodeId: 'node-1' },
  { id: 15, timestamp: '2024-11-17 10:23:28.234', level: 'info', source: 'runtime', message: 'New primary elected: node-3', nodeId: 'node-1' },
];

const levelColors: Record<LogLevel, string> = {
  debug: 'text-gray-500 bg-gray-100 dark:bg-gray-800',
  info: 'text-blue-600 bg-blue-100 dark:bg-blue-900/30',
  warn: 'text-amber-600 bg-amber-100 dark:bg-amber-900/30',
  error: 'text-red-600 bg-red-100 dark:bg-red-900/30',
};

const sourceColors: Record<LogSource, string> = {
  membership: 'text-purple-600',
  replication: 'text-green-600',
  storage: 'text-orange-600',
  network: 'text-cyan-600',
  runtime: 'text-indigo-600',
  wasm: 'text-pink-600',
};

export function LogsPanel() {
  const [logs, setLogs] = useState<LogEntry[]>(mockLogs);
  const [autoScroll, setAutoScroll] = useState(true);
  const [paused, setPaused] = useState(false);
  const [search, setSearch] = useState('');
  const [levelFilter, setLevelFilter] = useState<LogLevel | 'all'>('all');
  const [sourceFilter, setSourceFilter] = useState<LogSource | 'all'>('all');
  const logContainerRef = useRef<HTMLDivElement>(null);

  // Simulate live logs
  useEffect(() => {
    if (paused) return;
    const interval = setInterval(() => {
      const newLog: LogEntry = {
        id: Date.now(),
        timestamp: new Date().toISOString().replace('T', ' ').slice(0, 23),
        level: ['debug', 'info', 'warn', 'error'][Math.floor(Math.random() * 4)] as LogLevel,
        source: ['membership', 'replication', 'storage', 'network', 'runtime', 'wasm'][Math.floor(Math.random() * 6)] as LogSource,
        message: 'Simulated log entry for demo',
        nodeId: 'node-1',
      };
      setLogs(prev => [newLog, ...prev.slice(0, 999)]);
    }, 3000);
    return () => clearInterval(interval);
  }, [paused]);

  const filteredLogs = logs.filter((log) => {
    const matchesSearch = log.message.toLowerCase().includes(search.toLowerCase()) ||
      log.source.toLowerCase().includes(search.toLowerCase()) ||
      log.nodeId?.toLowerCase().includes(search.toLowerCase());
    const matchesLevel = levelFilter === 'all' || log.level === levelFilter;
    const matchesSource = sourceFilter === 'all' || log.source === sourceFilter;
    return matchesSearch && matchesLevel && matchesSource;
  });

  const scrollToBottom = () => {
    logContainerRef.current?.scrollTo({ top: logContainerRef.current.scrollHeight, behavior: 'smooth' });
  };

  useEffect(() => {
    if (autoScroll && !paused) scrollToBottom();
  }, [logs, autoScroll, paused]);

  return (
    <div className="space-y-6 h-[calc(100vh-200px)] flex flex-col">
      <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
        <div>
          <h1 className="text-3xl font-bold tracking-tight flex items-center gap-2">
            <Terminal className="h-8 w-8 text-primary" />
            Live Logs
          </h1>
          <p className="text-muted-foreground">Real-time cluster event stream</p>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="outline" size="sm" onClick={() => navigator.clipboard.writeText(
            filteredLogs.map(l => `[${l.timestamp}] [${l.level.toUpperCase()}] [${l.source}] ${l.message}`).join('\n')
          )}>
            <Download className="h-4 w-4 mr-2" />
            Export
          </Button>
          <Button variant="outline" size="sm" onClick={() => setLogs([])}>
            <Trash2 className="h-4 w-4 mr-2" />
            Clear
          </Button>
          <Button variant={paused ? 'default' : 'outline'} size="sm" onClick={() => setPaused(!paused)}>
            {paused ? <Play className="h-4 w-4 mr-2" /> : <Pause className="h-4 w-4 mr-2" />}
            {paused ? 'Resume' : 'Pause'}
          </Button>
        </div>
      </div>

      <div className="flex flex-col sm:flex-row gap-4">
        <div className="relative flex-1">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
          <Input
            placeholder="Filter logs..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="pl-10"
          />
        </div>
        <div className="flex items-center gap-2">
          <Filter className="h-4 w-4 text-muted-foreground" />
          <select
            value={levelFilter}
            onChange={(e) => setLevelFilter(e.target.value as typeof levelFilter)}
            className="px-3 py-2 text-sm border border-input bg-background rounded-lg w-36"
          >
            <option value="all">All Levels</option>
            <option value="error">Error</option>
            <option value="warn">Warning</option>
            <option value="info">Info</option>
            <option value="debug">Debug</option>
          </select>
          <select
            value={sourceFilter}
            onChange={(e) => setSourceFilter(e.target.value as typeof sourceFilter)}
            className="px-3 py-2 text-sm border border-input bg-background rounded-lg w-40"
          >
            <option value="all">All Sources</option>
            <option value="membership">Membership</option>
            <option value="replication">Replication</option>
            <option value="storage">Storage</option>
            <option value="network">Network</option>
            <option value="runtime">Runtime</option>
            <option value="wasm">WASM</option>
          </select>
          <Button variant="outline" size="sm" onClick={() => setAutoScroll(!autoScroll)}>
            {autoScroll ? <ChevronDown className="h-4 w-4 mr-2" /> : <ChevronUp className="h-4 w-4 mr-2" />}
            {autoScroll ? 'Auto-scroll' : 'Manual'}
          </Button>
        </div>
      </div>

      <Card className="flex-1 flex flex-col min-h-0">
        <CardContent className="flex-1 p-0 overflow-hidden">
          <div 
            ref={logContainerRef}
            className="h-full overflow-y-auto font-mono text-sm p-4"
            onScroll={(e) => {
              const { scrollTop, scrollHeight, clientHeight } = e.currentTarget;
              if (scrollHeight - scrollTop - clientHeight < 100) {
                setAutoScroll(true);
              } else {
                setAutoScroll(false);
              }
            }}
          >
            {filteredLogs.map((log) => (
              <div 
                key={log.id} 
                className="py-1 px-2 border-l-2 border-transparent hover:border-primary/20 transition-colors"
                style={{ fontFamily: 'var(--font-mono)' }}
              >
                <span className="text-muted-foreground mr-3">{log.timestamp}</span>
                <Badge 
                  variant="outline" 
                  className={cn('mr-2 px-2 py-0.5 text-xs', levelColors[log.level])}
                >
                  {log.level.toUpperCase()}
                </Badge>
                <Badge 
                  variant="outline" 
                  className={cn('mr-2 px-2 py-0.5 text-xs', sourceColors[log.source])}
                >
                  {log.source}
                </Badge>
                {log.nodeId && (
                  <Badge variant="outline" className="mr-2 px-2 py-0.5 text-xs text-muted-foreground">
                    {log.nodeId}
                  </Badge>
                )}
                <span className={cn(log.level === 'error' ? 'text-red-500' : log.level === 'warn' ? 'text-amber-500' : '')}>
                  {log.message}
                </span>
              </div>
            ))}
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
