'use client';

import { useState } from 'react';
import { 
  Server, 
  Search, 
  Filter,
  MoreVertical,
  RefreshCw,
  Trash2,
  Edit,
  Eye
} from 'lucide-react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { cn, formatBytes, formatDuration, formatRevision } from '@/lib/utils';

interface Node {
  id: string;
  role: 'Primary' | 'Replica';
  status: 'healthy' | 'suspected' | 'failed' | 'syncing';
  revision: number;
  latency: number;
  lastHeartbeat: number;
  uptime: number;
  storage: number;
  address?: string;
}

const mockNodes: Node[] = [
  {
    id: 'node-1',
    role: 'Primary',
    status: 'healthy',
    revision: 15420,
    latency: 8,
    lastHeartbeat: Date.now() - 500,
    uptime: 86400000 * 7,
    storage: 2100000000,
    address: '192.168.1.10:8001',
  },
  {
    id: 'node-2',
    role: 'Replica',
    status: 'healthy',
    revision: 15420,
    latency: 12,
    lastHeartbeat: Date.now() - 800,
    uptime: 86400000 * 5,
    storage: 2100000000,
    address: '192.168.1.11:8002',
  },
  {
    id: 'node-3',
    role: 'Replica',
    status: 'syncing',
    revision: 15418,
    latency: 45,
    lastHeartbeat: Date.now() - 2000,
    uptime: 86400000 * 3,
    storage: 1950000000,
    address: '192.168.1.12:8003',
  },
];

export function NodeList() {
  const [search, setSearch] = useState('');
  const [statusFilter, setStatusFilter] = useState<'all' | 'healthy' | 'suspected' | 'failed' | 'syncing'>('all');

  const filteredNodes = mockNodes.filter((node) => {
    const matchesSearch = node.id.toLowerCase().includes(search.toLowerCase()) ||
      node.address?.toLowerCase().includes(search.toLowerCase());
    const matchesStatus = statusFilter === 'all' || node.status === statusFilter;
    return matchesSearch && matchesStatus;
  });

  const getStatusColor = (status: Node['status']) => {
    switch (status) {
      case 'healthy': return 'bg-green-500';
      case 'suspected': return 'bg-amber-500';
      case 'failed': return 'bg-red-500';
      case 'syncing': return 'bg-blue-500';
    }
  };

  const getStatusBadgeVariant = (status: Node['status']) => {
    switch (status) {
      case 'healthy': return 'default';
      case 'suspected': return 'secondary';
      case 'failed': return 'destructive';
      case 'syncing': return 'outline';
    }
  };

  return (
    <div className="space-y-6">
      <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Cluster Nodes</h1>
          <p className="text-muted-foreground">Manage and monitor cluster membership</p>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="outline" size="sm">
            <RefreshCw className="h-4 w-4 mr-2" />
            Refresh
          </Button>
          <Button size="sm">
            <Server className="h-4 w-4 mr-2" />
            Add Node
          </Button>
        </div>
      </div>

      <div className="flex flex-col sm:flex-row gap-4">
        <div className="relative flex-1">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
          <Input
            placeholder="Search nodes..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="pl-10"
          />
        </div>
        <div className="flex items-center gap-2">
          <Filter className="h-4 w-4 text-muted-foreground" />
          <select
            value={statusFilter}
            onChange={(e) => setStatusFilter(e.target.value as typeof statusFilter)}
            className="px-3 py-2 text-sm border border-input bg-background rounded-lg focus:outline-none focus:ring-2 focus:ring-ring"
          >
            <option value="all">All Status</option>
            <option value="healthy">Healthy</option>
            <option value="suspected">Suspected</option>
            <option value="failed">Failed</option>
            <option value="syncing">Syncing</option>
          </select>
        </div>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center justify-between">
            <span>Nodes ({filteredNodes.length})</span>
            <Badge variant="outline">{mockNodes.length} total</Badge>
          </CardTitle>
        </CardHeader>
        <CardContent className="p-0">
          <div className="overflow-x-auto">
            <table className="w-full">
              <thead>
                <tr className="border-b border-border bg-muted/50">
                  <th className="px-6 py-3 text-left text-xs font-medium text-muted-foreground uppercase tracking-wider">Node</th>
                  <th className="px-6 py-3 text-left text-xs font-medium text-muted-foreground uppercase tracking-wider">Role</th>
                  <th className="px-6 py-3 text-left text-xs font-medium text-muted-foreground uppercase tracking-wider">Status</th>
                  <th className="px-6 py-3 text-left text-xs font-medium text-muted-foreground uppercase tracking-wider">Revision</th>
                  <th className="px-6 py-3 text-left text-xs font-medium text-muted-foreground uppercase tracking-wider">Latency</th>
                  <th className="px-6 py-3 text-left text-xs font-medium text-muted-foreground uppercase tracking-wider">Last Heartbeat</th>
                  <th className="px-6 py-3 text-left text-xs font-medium text-muted-foreground uppercase tracking-wider">Uptime</th>
                  <th className="px-6 py-3 text-left text-xs font-medium text-muted-foreground uppercase tracking-wider">Storage</th>
                  <th className="px-6 py-3 text-right text-xs font-medium text-muted-foreground uppercase tracking-wider">Actions</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-border">
                {filteredNodes.map((node) => (
                  <tr key={node.id} className="hover:bg-muted/50 transition-colors">
                    <td className="px-6 py-4">
                      <div>
                        <p className="font-mono font-medium">{node.id}</p>
                        {node.address && (
                          <p className="text-xs text-muted-foreground font-mono">{node.address}</p>
                        )}
                      </div>
                    </td>
                    <td className="px-6 py-4">
                      <Badge variant={node.role === 'Primary' ? 'default' : 'secondary'}>
                        {node.role}
                      </Badge>
                    </td>
                    <td className="px-6 py-4">
                      <div className="flex items-center gap-2">
                        <span className={cn('h-2 w-2 rounded-full', getStatusColor(node.status))} />
                        <Badge variant={getStatusBadgeVariant(node.status)} className="capitalize">
                          {node.status}
                        </Badge>
                      </div>
                    </td>
                    <td className="px-6 py-4 font-mono text-sm">{formatRevision(node.revision)}</td>
                    <td className="px-6 py-4">
                      <span className={cn(
                        'font-mono text-sm',
                        node.latency > 50 ? 'text-amber-500' : node.latency > 20 ? 'text-blue-500' : 'text-green-500'
                      )}>
                        {node.latency}ms
                      </span>
                    </td>
                    <td className="px-6 py-4 text-sm text-muted-foreground">
                      {Math.round((Date.now() - node.lastHeartbeat) / 1000)}s ago
                    </td>
                    <td className="px-6 py-4 text-sm text-muted-foreground">
                      {formatDuration(node.uptime)}
                    </td>
                    <td className="px-6 py-4 font-mono text-sm">{formatBytes(node.storage)}</td>
                    <td className="px-6 py-4 text-right">
                      <div className="flex items-center justify-end gap-1">
                        <Button variant="ghost" size="icon" className="h-8 w-8">
                          <Eye className="h-4 w-4" />
                        </Button>
                        <Button variant="ghost" size="icon" className="h-8 w-8">
                          <Edit className="h-4 w-4" />
                        </Button>
                        <Button variant="ghost" size="icon" className="h-8 w-8 text-red-500 hover:text-red-600">
                          <Trash2 className="h-4 w-4" />
                        </Button>
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
