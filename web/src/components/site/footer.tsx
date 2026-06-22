import Link from "next/link";
import { site } from "@/lib/site";
import { Logo } from "@/components/site/logo";
import { LatestVersion } from "@/components/site/version";

export function Footer() {
  return (
    <footer className="border-t border-white/10 px-6 py-12">
      <div className="mx-auto flex max-w-5xl flex-col items-center justify-between gap-6 sm:flex-row">
        <div className="flex items-center gap-2.5">
          <Logo className="h-9 w-9" />
          <div>
            <div className="flex items-center gap-2 text-sm font-semibold">
              MyVPN
              <LatestVersion className="rounded bg-white/5 px-1.5 py-0.5 text-[10px] font-normal text-faint" />
            </div>
            <div className="text-xs text-faint">{site.tagline}</div>
          </div>
        </div>
        <div className="flex items-center gap-5 text-sm text-muted">
          <a href="/#features" className="transition hover:text-ink">
            Features
          </a>
          <a href="/#how" className="transition hover:text-ink">
            How it works
          </a>
          <Link href="/changelog" className="transition hover:text-ink">
            Changelog
          </Link>
          <a
            href={site.github}
            target="_blank"
            rel="noreferrer"
            className="transition hover:text-ink"
          >
            GitHub
          </a>
        </div>
      </div>
      <div className="mx-auto mt-8 max-w-5xl text-center text-xs text-faint">
        © {new Date().getFullYear()} MyVPN · MIT licensed · Built with Tauri, iroh &
        Next.js
      </div>
    </footer>
  );
}
