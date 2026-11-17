'use client';

import React from 'react';
import { cn } from '@/lib/utils';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import rehypeHighlight from 'rehype-highlight';

interface MarkdownRendererProps {
  content: string;
}

export function MarkdownRenderer({ content }: MarkdownRendererProps) {
  return (
    <div className={cn('prose prose-lg max-w-none dark:prose-invert')}>
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        rehypePlugins={[
          [rehypeHighlight, { ignoreMissing: true }]
        ]}
        components={{
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          code({ inline, className, children, ...props }: any) {
            const match = /language-(\w+)/.exec(className || '');
            return !inline && match ? (
              <pre {...props} className={cn('bg-muted', 'p-4', 'rounded-lg', 'overflow-x-auto')}>
                <code className={className}>{String(children).replace(/\n$/, '')}</code>
              </pre>
            ) : (
              <code className={cn('bg-muted px-1.5 py-0.5 rounded text-primary', className)} {...props}>
                {children}
              </code>
            );
          },
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          table({ children, ...props }: any) {
            return (
              <div className="overflow-x-auto">
                <table className={cn('w-full', 'border-collapse')} {...props}>
                  {children}
                </table>
              </div>
            );
          },
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          th({ children, ...props }: any) {
            return (
              <th className={cn('border', 'border-border', 'bg-muted', 'px-3', 'py-2', 'text-left', 'font-semibold')} {...props}>
                {children}
              </th>
            );
          },
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          td({ children, ...props }: any) {
            return (
              <td className={cn('border', 'border-border', 'px-3', 'py-2')} {...props}>
                {children}
              </td>
            );
          },
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          blockquote({ children, ...props }: any) {
            return (
              <blockquote className={cn('border-l-4', 'border-primary', 'pl-4', 'italic', 'text-muted-foreground', 'my-4')} {...props}>
                {children}
              </blockquote>
            );
          },
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          hr({ ...props }: any) {
            return <hr className={cn('border-border', 'my-8')} {...props} />;
          },
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          a({ children, href, ...props }: any) {
            return (
              <a
                className={cn('text-primary', 'no-underline', 'hover:underline')}
                href={href}
                target={href?.startsWith('http') ? '_blank' : undefined}
                rel={href?.startsWith('http') ? 'noopener noreferrer' : undefined}
                {...props}
              >
                {children}
              </a>
            );
          },
        }}
      >
        {content}
      </ReactMarkdown>
    </div>
  );
}
