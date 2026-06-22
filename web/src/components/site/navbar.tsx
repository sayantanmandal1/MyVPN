"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { Download } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Logo } from "@/components/site/logo";
import { GithubMark } from "@/components/site/icons";
import { site } from "@/lib/site";
import { cn } from "@/lib/utils";

const links = [
  { href: "#features", label: "Features" },
  { href: "#how", label: "How it works" },
  { href: "#security", label: "Security" },
];

export function Navbar() {
  const [scrolled, setScrolled] = useState(false);

  useEffect(() => {
    const onScroll = () => setScrolled(window.scrollY > 12);
    onScroll();
    window.addEventListener("scroll", onScroll, { passive: true });
    return () => window.removeEventListener("scroll", onScroll);
  }, []);

  return (
    <header className="fixed inset-x-0 top-0 z-50 flex justify-center px-4 pt-4">
      <nav
        className={cn(
          "flex w-full max-w-5xl items-center gap-3 rounded-2xl px-3 py-2.5 transition-all duration-300",
          scrolled ? "glass-strong" : "border border-transparent",
        )}
      >
        <a href="#" className="flex items-center gap-2.5 px-1">
          <Logo className="h-8 w-8" />
          <span className="text-[15px] font-semibold tracking-tight">MyVPN</span>
        </a>

        <div className="mx-auto hidden items-center gap-1 md:flex">
          {links.map((l) => (
            <a
              key={l.href}
              href={l.href}
              className="rounded-lg px-3 py-2 text-sm text-[color:var(--color-muted)] transition hover:text-ink"
            >
              {l.label}
            </a>
          ))}
          <Link
            href="/changelog"
            className="rounded-lg px-3 py-2 text-sm text-[color:var(--color-muted)] transition hover:text-ink"
          >
            Changelog
          </Link>
        </div>

        <div className="ml-auto flex items-center gap-2 md:ml-0">
          <a
            href={site.github}
            target="_blank"
            rel="noreferrer"
            className="hidden rounded-lg p-2 text-[color:var(--color-muted)] transition hover:bg-white/5 hover:text-ink sm:block"
            aria-label="GitHub"
          >
            <GithubMark className="h-5 w-5" />
          </a>
          <Button asChild size="md">
            <a href={site.download}>
              <Download className="h-4 w-4" />
              Download
            </a>
          </Button>
        </div>
      </nav>
    </header>
  );
}
