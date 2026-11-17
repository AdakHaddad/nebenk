'use client';

import { useState } from 'react';
import { 
  Database, 
  Search, 
  Plus, 
  Edit, 
  Trash2, 
  Eye,
  RefreshCw,
  Copy,
  Check,
  Key
} from 'lucide-react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { cn, formatBytes, formatRevision, truncate } from '@/lib/utils';

interface KVEntry {
  key: string;
  value: string;
  revision: number;
  size: number;
  updatedAt: string;
}

const mockEntries: KVEntry[] = [
  { key: 'user:1001', value: '{"name":"Alice","email":"alice@example.com","role":"admin"}', revision: 15420, size: 78, updatedAt: '2m ago' },
  { key: 'user:1002', value: '{"name":"Bob","email":"bob@example.com","role":"user"}', revision: 15418, size: 72, updatedAt: '15m ago' },
  { key: 'config:feature-flags', value: '{"new-ui":true,"beta-api":false,"dark-mode":true}', revision: 15415, size: 58, updatedAt: '1h ago' },
  { key: 'session:abc123', value: '{"userId":1001,"expires":1734567890,"permissions":["read","write"]}', revision: 15410, size: 92, updatedAt: '3h ago' },
  { key: 'cache:product:500', value: '{"id":500,"name":"Widget Pro","price":99.99,"stock":42}', revision: 15405, size: 68, updatedAt: '6h ago' },
  { key: 'metrics:dau', value: '{"date":"2024-11-17","count":1247}', revision: 15400, size: 42, updatedAt: '12h ago' },
];

export function KeyValueStore() {
  const [search, setSearch] = useState('');
  const [showModal, setShowModal] = useState(false);
  const [editingEntry, setEditingEntry] = useState<KVEntry | null>(null);
  const [formData, setFormData] = useState({ key: '', value: '' });

  const filteredEntries = mockEntries.filter((entry) =>
    entry.key.toLowerCase().includes(search.toLowerCase()) ||
    entry.value.toLowerCase().includes(search.toLowerCase())
  );

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (editingEntry) {
      // Update logic here
    } else {
      // Create logic here
    }
    setShowModal(false);
    setEditingEntry(null);
    setFormData({ key: '', value: '' });
  };

  const handleEdit = (entry: KVEntry) => {
    setEditingEntry(entry);
    setFormData({ key: entry.key, value: entry.value });
    setShowModal(true);
  };

  const handleDelete = (key: string) => {
    if (confirm(`Delete key "${key}"?`)) {
      // Delete logic here
    }
  };

  const copyToClipboard = (text: string) => {
    navigator.clipboard.writeText(text);
  };

  return (
    <div className="space-y-6">
      <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
        <div>
          <h1 className="text-3xl font-bold tracking-tight flex items-center gap-2">
            <Database className="h-8 w-8 text-primary" />
            KV Store
          </h1>
          <p className="text-muted-foreground">Browse and manage key-value pairs</p>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="outline" size="sm">
            <RefreshCw className="h-4 w-4 mr-2" />
            Refresh
          </Button>
          <Button onClick={() => { setEditingEntry(null); setFormData({ key: '', value: '' }); setShowModal(true); }}>
            <Plus className="h-4 w-4 mr-2" />
            Add Key
          </Button>
        </div>
      </div>

      <div className="flex gap-4">
        <div className="relative flex-1">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
          <Input
            placeholder="Search keys or values..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="pl-10"
          />
        </div>
        <Badge variant="outline">{filteredEntries.length} / {mockEntries.length} entries</Badge>
      </div>

      <Card>
        <CardContent className="p-0">
          <div className="overflow-x-auto">
            <table className="w-full">
              <thead>
                <tr className="border-b border-border bg-muted/50">
                  <th className="px-6 py-3 text-left text-xs font-medium text-muted-foreground uppercase tracking-wider">Key</th>
                  <th className="px-6 py-3 text-left text-xs font-medium text-muted-foreground uppercase tracking-wider">Value (JSON)</th>
                  <th className="px-6 py-3 text-left text-xs font-medium text-muted-foreground uppercase tracking-wider">Revision</th>
                  <th className="px-6 py-3 text-left text-xs font-medium text-muted-foreground uppercase tracking-wider">Size</th>
                  <th className="px-6 py-3 text-left text-xs font-medium text-muted-foreground uppercase tracking-wider">Updated</th>
                  <th className="px-6 py-3 text-right text-xs font-medium text-muted-foreground uppercase tracking-wider">Actions</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-border">
                {filteredEntries.map((entry) => (
                  <tr key={entry.key} className="hover:bg-muted/50 transition-colors">
                    <td className="px-6 py-4">
                      <code className="font-mono text-sm bg-muted px-2 py-1 rounded">{entry.key}</code>
                    </td>
                    <td className="px-6 py-4 max-w-md">
                      <code className="font-mono text-sm text-muted-foreground truncate block">
                        {truncate(entry.value, 80)}
                      </code>
                    </td>
                    <td className="px-6 py-4 font-mono text-sm">{formatRevision(entry.revision)}</td>
                    <td className="px-6 py-4 text-sm">{formatBytes(entry.size)}</td>
                    <td className="px-6 py-4 text-sm text-muted-foreground">{entry.updatedAt}</td>
                    <td className="px-6 py-4 text-right">
                      <div className="flex items-center justify-end gap-1">
                        <Button 
                          variant="ghost" 
                          size="icon" 
                          className="h-8 w-8"
                          onClick={() => copyToClipboard(entry.key)}
                        >
                          <Copy className="h-4 w-4" />
                        </Button>
                        <Button 
                          variant="ghost" 
                          size="icon" 
                          className="h-8 w-8"
                          onClick={() => copyToClipboard(entry.value)}
                        >
                          <Eye className="h-4 w-4" />
                        </Button>
                        <Button variant="ghost" size="icon" className="h-8 w-8" onClick={() => handleEdit(entry)}>
                          <Edit className="h-4 w-4" />
                        </Button>
                        <Button variant="ghost" size="icon" className="h-8 w-8 text-red-500 hover:text-red-600" onClick={() => handleDelete(entry.key)}>
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

      {/* Modal */}
      {showModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
          <div className="bg-card rounded-lg shadow-lg p-6 w-full max-w-2xl mx-4 max-h-[80vh] overflow-auto">
            <div className="flex items-center justify-between mb-4">
              <h2 className="text-xl font-bold">{editingEntry ? 'Edit Key' : 'Add New Key'}</h2>
              <Button variant="ghost" size="icon" onClick={() => setShowModal(false)}>
                <span className="text-2xl">&times;</span>
              </Button>
            </div>
            <form onSubmit={handleSubmit} className="space-y-4">
              <div>
                <label className="block text-sm font-medium mb-1">Key</label>
                <Input
                  value={formData.key}
                  onChange={(e) => setFormData({ ...formData, key: e.target.value })}
                  placeholder="e.g., user:1001"
                  disabled={!!editingEntry}
                />
              </div>
              <div>
                <label className="block text-sm font-medium mb-1">Value (JSON)</label>
                <textarea
                  value={formData.value}
                  onChange={(e) => setFormData({ ...formData, value: e.target.value })}
                  placeholder='{"name": "value"}'
                  className="w-full min-h-[150px] p-3 border border-input bg-background rounded-lg font-mono text-sm focus:outline-none focus:ring-2 focus:ring-ring"
                  rows={8}
                />
              </div>
              <div className="flex justify-end gap-2 pt-4 border-t">
                <Button type="button" variant="outline" onClick={() => setShowModal(false)}>Cancel</Button>
                <Button type="submit">{editingEntry ? 'Update' : 'Create'}</Button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
}
