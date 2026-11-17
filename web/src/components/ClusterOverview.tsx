'use client';

import { 
  Server, 
  Database, 
  Activity, 
  TrendingUp,
  AlertTriangle,
  CheckCircle,
  Clock
} from 'lucide-react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { cn, formatBytes, formatDuration, formatRevision } from '@/lib/utils';

interface StatCardProps {
  title: string;
  value: string | number;
  icon: React.ReactNode;
  trend?: { value: string; positive: boolean };
  description?: string;
}

function StatCard({ title, value, icon, trend, description }: StatCardProps) {
  return (
    <Card>
      <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
        <CardTitle className="text-sm font-medium text-muted-foreground">{title}</CardTitle>
        <div className="text-muted-foreground">{icon}</div>
      </CardHeader>
      <CardContent>
        <div className="text-2xl font-bold">{value}</div>
        <div className="flex items-center gap-2 text-xs text-muted-foreground">
          {trend && (
            <Badge variant={trend.positive ? 'default' : 'destructive'} className="gap-1">
              {trend.positive ? <TrendingUp className="h-3 w-3" /> : <Activity className="h-3 w-3" />}
              {trend.value}
            </Badge>
          )}
          {description && <span>{description}</span>}
        </div>
      </CardContent>
    </Card>
  );
}

const mockClusterData = {
  clusterId: 'production-cluster',
  status: 'healthy' as const,
  nodes: 3,
  primary: 'node-1',
  revision: 15420,
  uptime: 86400000 * 7,
  storage: 2450000000,
  operationsPerSec: 1247,
  latencyP50: 12,
  latencyP99: 89,
};

export function ClusterOverview() {
  return (
    <div className="space-y-6">
      <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Cluster Overview</h1>
          <p className="text-muted-foreground">
            Monitoring <code className="px-1.5 py-0.5 bg-muted rounded font-mono text-sm">
              {mockClusterData.clusterId}
            </code>
          </p>
        </div>
        <div className="flex items-center gap-2">
          <Badge variant={mockClusterData.status === 'healthy' ? 'default' : 'destructive'} className="gap-1">
            <CheckCircle className="h-3 w-3" />
            {mockClusterData.status.charAt(0).toUpperCase() + mockClusterData.status.slice(1)}
          </Badge>
        </div>
      </div>

      <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
        <StatCard
          title="Total Nodes"
          value={mockClusterData.nodes}
          icon={<Server className="h-5 w-5 text-primary" />}
          description="1 primary, 2 replicas"
        />
        <StatCard
          title="Current Revision"
          value={formatRevision(mockClusterData.revision)}
          icon={<Activity className="h-5 w-5 text-blue-500" />}
          trend={{ value: '+234/hr', positive: true }}
          description="Auto-incrementing"
        />
        <StatCard
          title="Storage Used"
          value={formatBytes(mockClusterData.storage)}
          icon={<Database className="h-5 w-5 text-green-500" />}
          description="Including WAL + snapshots"
        />
        <StatCard
          title="Uptime"
          value={formatDuration(mockClusterData.uptime)}
          icon={<Clock className="h-5 w-5 text-amber-500" />}
          description="Since last restart"
        />
      </div>

      <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Activity className="h-5 w-5" />
              Performance Metrics
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            <div>
              <div className="flex justify-between text-sm mb-1">
                <span className="text-muted-foreground">Ops/sec</span>
                <span className="font-mono font-medium">{mockClusterData.operationsPerSec.toLocaleString()}</span>
              </div>
              <div className="h-2 bg-muted rounded-full overflow-hidden">
                <div 
                  className="h-full bg-primary rounded-full transition-all" 
                  style={{ width: '65%' }}
                />
              </div>
            </div>
            <div>
              <div className="flex justify-between text-sm mb-1">
                <span className="text-muted-foreground">P50 Latency</span>
                <span className="font-mono font-medium">{mockClusterData.latencyP50}ms</span>
              </div>
              <div className="h-2 bg-muted rounded-full overflow-hidden">
                <div 
                  className="h-full bg-green-500 rounded-full transition-all" 
                  style={{ width: '40%' }}
                />
              </div>
            </div>
            <div>
              <div className="flex justify-between text-sm mb-1">
                <span className="text-muted-foreground">P99 Latency</span>
                <span className="font-mono font-medium">{mockClusterData.latencyP99}ms</span>
              </div>
              <div className="h-2 bg-muted rounded-full overflow-hidden">
                <div 
                  className="h-full bg-amber-500 rounded-full transition-all" 
                  style={{ width: '75%' }}
                />
              </div>
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Server className="h-5 w-5" />
              Node Status
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            {[
              { id: 'node-1', role: 'Primary', status: 'healthy', revision: 15420, latency: 8 },
              { id: 'node-2', role: 'Replica', status: 'healthy', revision: 15420, latency: 12 },
              { id: 'node-3', role: 'Replica', status: 'healthy', revision: 15418, latency: 15 },
            ].map((node) => (
              <div key={node.id} className="flex items-center justify-between p-3 bg-muted/50 rounded-lg">
                <div className="flex items-center gap-3">
                  <div className={cn(
                    'h-2.5 w-2.5 rounded-full',
                    node.status === 'healthy' ? 'bg-green-500' : 'bg-red-500'
                  )} />
                  <div>
                    <p className="font-medium text-sm">{node.id}</p>
                    <p className="text-xs text-muted-foreground">{node.role}</p>
                  </div>
                </div>
                <div className="text-right">
                  <p className="font-mono text-sm">Rev: {formatRevision(node.revision)}</p>
                  <p className="text-xs text-muted-foreground">{node.latency}ms latency</p>
                </div>
              </div>
            ))}
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <AlertTriangle className="h-5 w-5" />
              Recent Events
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            {[
              { time: '2m ago', type: 'info', msg: 'Snapshot created at revision 15400' },
              { time: '15m ago', type: 'info', msg: 'Node node-3 caught up to primary' },
              { time: '1h ago', type: 'warning', msg: 'High latency detected on node-2 (P99: 145ms)' },
              { time: '3h ago', type: 'info', msg: 'Scheduled snapshot completed successfully' },
              { time: '6h ago', type: 'info', msg: 'Cluster membership stable' },
            ].map((event, i) => (
              <div key={i} className="flex items-start gap-3 p-2">
                <div className={cn(
                  'h-1.5 w-1.5 rounded-full mt-1.5 flex-shrink-0',
                  event.type === 'warning' ? 'bg-amber-500' : 'bg-blue-500'
                )} />
                <div className="flex-1 min-w-0">
                  <p className="text-sm">{event.msg}</p>
                  <p className="text-xs text-muted-foreground">{event.time}</p>
                </div>
              </div>
            ))}
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
