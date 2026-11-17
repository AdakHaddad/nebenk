'use client';

import { useEffect, useState } from 'react';
import Link from 'next/link';
import { 
  BookOpen, 
  Github, 
  Terminal, 
  Server, 
  Database, 
  Network,
  ArrowRight,
  Menu,
  X,
  Sun,
  Moon
} from 'lucide-react';
import { cn } from '@/lib/utils';

export default function HomePage() {
  const [isDark, setIsDark] = useState(false);
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false);

  useEffect(() => {
    const saved = localStorage.getItem('theme');
    if (saved) setIsDark(saved === 'dark');
    else if (window.matchMedia('(prefers-color-scheme: dark)').matches) setIsDark(true);
  }, []);

  useEffect(() => {
    document.documentElement.classList.toggle('dark', isDark);
    localStorage.setItem('theme', isDark ? 'dark' : 'light');
  }, [isDark]);

  const navItems = [
    { href: '/', label: 'Introduction', icon: BookOpen },
    { href: '/architecture', label: 'Architecture', icon: Server },
    { href: '/api', label: 'API Reference', icon: Terminal },
    { href: '/getting-started', label: 'Getting Started', icon: Database },
    { href: '/prd', label: 'Product Requirements', icon: Network },
  ];

  return (
    <div className="min-h-screen bg-background font-sans antialiased">
      {/* Header */}
      <header className="sticky top-0 z-50 w-full border-b border-border bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/60">
        <div className="container mx-auto flex h-16 items-center justify-between px-4">
          <Link href="/" className="flex items-center gap-2 font-semibold text-xl">
            <span className="text-primary">NEBENK</span>
          </Link>

          <nav className={cn(
            'hidden md:flex items-center gap-6',
            mobileMenuOpen ? 'flex flex-col absolute top-16 left-0 right-0 bg-background p-4 border-b border-border' : ''
          )}>
            {navItems.map((item) => (
              <Link
                key={item.href}
                href={item.href}
                className="flex items-center gap-1.5 text-sm font-medium text-muted-foreground hover:text-foreground transition-colors"
              >
                <item.icon className="h-4 w-4" />
                {item.label}
              </Link>
            ))}
            <a
              href="https://github.com/AdakHaddad/nebenk"
              target="_blank"
              rel="noopener noreferrer"
              className="flex items-center gap-1.5 text-sm font-medium text-muted-foreground hover:text-foreground transition-colors"
            >
              <Github className="h-4 w-4" />
              GitHub
            </a>
          </nav>

          <div className="flex items-center gap-4">
            <button
              onClick={() => setIsDark(!isDark)}
              className="p-2 rounded-lg hover:bg-accent transition-colors"
              aria-label={isDark ? 'Switch to light mode' : 'Switch to dark mode'}
            >
              {isDark ? <Sun className="h-5 w-5" /> : <Moon className="h-5 w-5" />}
            </button>
            <button
              className="md:hidden p-2 rounded-lg hover:bg-accent transition-colors"
              onClick={() => setMobileMenuOpen(!mobileMenuOpen)}
              aria-label="Toggle menu"
            >
              {mobileMenuOpen ? <X className="h-5 w-5" /> : <Menu className="h-5 w-5" />}
            </button>
          </div>
        </div>
      </header>

      {/* Hero */}
      <main>
        <section className="container mx-auto py-20 px-4 text-center">
          <h1 className="text-4xl md:text-6xl font-bold tracking-tight mb-6">
            NEBENK
            <br />
            <span className="text-primary">Lightweight Distributed Runtime</span>
          </h1>
          <p className="text-xl md:text-2xl text-muted-foreground max-w-3xl mx-auto mb-10">
            Run stateful applications across heterogeneous devices with replicated state.
            The application survives. The device is disposable.
          </p>
          <div className="flex flex-col sm:flex-row items-center justify-center gap-4">
            <Link
              href="/getting-started"
              className="inline-flex items-center justify-center gap-2 rounded-lg bg-primary px-6 py-3 text-sm font-medium text-primary-foreground hover:bg-primary/90 transition-colors"
            >
              Get Started
              <ArrowRight className="h-4 w-4" />
            </Link>
            <Link
              href="/architecture"
              className="inline-flex items-center justify-center gap-2 rounded-lg border border-input bg-background px-6 py-3 text-sm font-medium hover:bg-accent transition-colors"
            >
              Architecture
            </Link>
          </div>
        </section>

        {/* Features */}
        <section className="container mx-auto py-20 px-4">
          <div className="grid md:grid-cols-3 gap-6">
            <FeatureCard
              icon={Server}
              title="Portable Execution"
              description="Run WebAssembly applications across Android, Linux, macOS, Windows, ARM, x86 without modification."
            />
            <FeatureCard
              icon={Database}
              title="Portable State"
              description="Application state is snapshotted and replicated. Surviving nodes recover state after device failure."
            />
            <FeatureCard
              icon={Network}
              title="P2P Networking"
              description="libp2p with QUIC transport, encrypted peer connections, NAT traversal, and mDNS discovery."
            />
          </div>
        </section>

        {/* Quick Links */}
        <section className="container mx-auto py-20 px-4 border-y border-border">
          <h2 className="text-3xl font-bold text-center mb-12">Documentation</h2>
          <div className="grid md:grid-cols-2 lg:grid-cols-4 gap-6">
            <DocCard
              href="/getting-started"
              icon={Terminal}
              title="Getting Started"
              description="Install NEBENK, initialize a node, and run your first cluster in minutes."
            />
            <DocCard
              href="/architecture"
              icon={Server}
              title="Architecture"
              description="Deep dive into the crate architecture, replication model, and failure recovery."
            />
            <DocCard
              href="/api"
              icon={Terminal}
              title="API Reference"
              description="Complete API documentation for Runtime, Membership, Storage, and WASM engine."
            />
            <DocCard
              href="/prd"
              icon={BookOpen}
              title="Product Requirements"
              description="Full product vision, use cases, functional requirements, and roadmap."
            />
          </div>
        </section>

        {/* Code Example */}
        <section className="container mx-auto py-20 px-4">
          <h2 className="text-3xl font-bold text-center mb-8">Quick Example</h2>
          <div className="max-w-4xl mx-auto bg-muted rounded-lg p-6 overflow-x-auto">
            <pre className="text-sm text-foreground font-mono"><code>{`# Build from source
git clone https://github.com/AdakHaddad/nebenk
cd nebenk
cargo build --release

# Initialize a node
./target/release/nebenk init --data-dir .nebenk --cluster my-cluster

# Start as primary
./target/release/nebenk --data-dir .nebenk --cluster my-cluster start --primary

# Write data (primary only)
./target/release/nebenk --data-dir .nebenk put mykey "hello world"

# Read data
./target/release/nebenk --data-dir .nebenk get mykey
# hello world`}</code></pre>
          </div>
        </section>

        {/* Footer CTA */}
        <section className="container mx-auto py-20 px-4 text-center">
          <h2 className="text-3xl font-bold mb-4">Ready to Build?</h2>
          <p className="text-muted-foreground mb-8 max-w-2xl mx-auto">
            Join the NEBENK community. Contribute, report issues, or start building distributed applications today.
          </p>
          <div className="flex flex-col sm:flex-row items-center justify-center gap-4">
            <a
              href="https://github.com/AdakHaddad/nebenk"
              target="_blank"
              rel="noopener noreferrer"
              className="inline-flex items-center justify-center gap-2 rounded-lg bg-primary px-6 py-3 text-sm font-medium text-primary-foreground hover:bg-primary/90 transition-colors"
            >
              <Github className="h-4 w-4" />
              View on GitHub
            </a>
            <a
              href="https://github.com/AdakHaddad/nebenk/issues"
              target="_blank"
              rel="noopener noreferrer"
              className="inline-flex items-center justify-center gap-2 rounded-lg border border-input bg-background px-6 py-3 text-sm font-medium hover:bg-accent transition-colors"
            >
              Report Issue
            </a>
          </div>
        </section>
      </main>

      {/* Footer */}
      <footer className="border-t border-border py-8">
        <div className="container mx-auto px-4 text-center text-sm text-muted-foreground">
          <p>NEBENK — Lightweight distributed application runtime</p>
          <p className="mt-2">Apache License 2.0 • Built with Rust, WebAssembly, and ❤️</p>
        </div>
      </footer>
    </div>
  );
}

function FeatureCard({ icon: Icon, title, description }: { icon: React.ComponentType<{ className?: string }>; title: string; description: string }) {
  return (
    <div className="bg-card border border-border rounded-lg p-6 hover:border-primary/50 transition-colors">
      <div className="text-primary mb-4">
        <Icon className="h-8 w-8" />
      </div>
      <h3 className="text-xl font-semibold mb-2">{title}</h3>
      <p className="text-muted-foreground">{description}</p>
    </div>
  );
}

function DocCard({ href, icon: Icon, title, description }: { href: string; icon: React.ComponentType<{ className?: string }>; title: string; description: string }) {
  return (
    <Link
      href={href}
      className="group bg-card border border-border rounded-lg p-6 hover:border-primary/50 hover:shadow-lg transition-all"
    >
      <div className="text-primary mb-4">
        <Icon className="h-8 w-8" />
      </div>
      <h3 className="text-xl font-semibold mb-2 group-hover:text-primary transition-colors">{title}</h3>
      <p className="text-muted-foreground">{description}</p>
      <div className="mt-4 flex items-center gap-1 text-primary text-sm font-medium group-hover:gap-2 transition-all">
        Read more
        <ArrowRight className="h-4 w-4" />
      </div>
    </Link>
  );
}
