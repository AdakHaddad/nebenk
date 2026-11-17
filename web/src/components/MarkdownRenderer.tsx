'use client';

import React from 'react';
import { cn } from '@/lib/utils';

interface MarkdownRendererProps {
  content: string;
}

export function MarkdownRenderer({ content }: MarkdownRendererProps) {
  return (
    <div 
      className={cn(
        'prose prose-lg max-w-none dark:prose-invert',
        'prose-headings:font-semibold prose-headings:text-foreground',
        'prose-code:bg-muted prose-code:px-1.5 prose-code:py-0.5 prose-code:rounded prose-code:text-primary',
        'prose-pre:bg-muted prose-pre:border prose-pre:border-border',
        'prose-a:text-primary prose-a:no-underline hover:prose-a:underline',
        'prose-table:border prose-table:border-border',
        'prose-th:border prose-th:border-border prose-th:bg-muted',
        'prose-td:border prose-td:border-border',
        'prose-img:rounded-lg prose-img:border prose-img:border-border'
      )}
      dangerouslySetInnerHTML={{ __html: content }}
    />
  );
}
