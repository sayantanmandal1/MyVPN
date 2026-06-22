import Link from "next/link";
import { ArrowLeft } from "lucide-react";
import { site } from "@/lib/site";
import { Logo } from "@/components/site/logo";
import { Footer } from "@/components/site/footer";
import { ReleaseNotes } from "@/components/site/release-notes";

export const metadata = {
  title: "Changelog · MyVPN",
  description: "Release notes for every version of MyVPN.",
};

// Refresh from GitHub hourly without a redeploy.
export const revalidate = 3600;

interface Release {
  tag_name: string;
  name: string | null;
  body: string | null;
  published_at: string | null;
  html_url: string;
  prerelease: boolean;
  draft: boolean;
}

async function getReleases(): Promise<Release[]> {
  try {
    const res = await fetch(
      `https://api.github.com/repos/${site.repo}/releases?per_page=100`,
      { headers: { Accept: "application/vnd.github+json" } },
    );
    if (!res.ok) return [];
    const data = (await res.json()) as Release[];
    return Array.isArray(data) ? data.filter((r) => !r.draft) : [];
  } catch {
    return [];
  }
}

function fmtDate(iso: string | null): string {
  if (!iso) return "";
  const d = new Date(iso);
  return Number.isNaN(d.getTime())
    ? ""
    : d.toLocaleDateString(undefined, {
        year: "numeric",
        month: "short",
        day: "numeric",
      });
}

export default async function ChangelogPage() {
  const releases = await getReleases();

  return (
    <>
      <header className="fixed inset-x-0 top-0 z-50 flex justify-center px-4 pt-4">
        <nav className="glass-strong flex w-full max-w-3xl items-center gap-3 rounded-2xl px-3 py-2.5">
          <Link href="/" className="flex items-center gap-2.5 px-1">
            <Logo className="h-8 w-8" />
            <span className="text-[15px] font-semibold tracking-tight">MyVPN</span>
          </Link>
          <Link
            href="/"
            className="ml-auto inline-flex items-center gap-1.5 rounded-lg px-3 py-2 text-sm text-[color:var(--color-muted)] transition hover:text-ink"
          >
            <ArrowLeft className="h-4 w-4" />
            Home
          </Link>
        </nav>
      </header>

      <main className="mx-auto max-w-3xl px-4 pt-28 pb-16">
        <div className="mb-10">
          <h1 className="text-3xl font-semibold tracking-tight sm:text-4xl">
            Changelog
          </h1>
          <p className="mt-2 text-[15px] text-muted">
            Every release of MyVPN, newest first — straight from GitHub.
          </p>
        </div>

        {releases.length === 0 ? (
          <div className="glass rounded-2xl p-8 text-center text-sm text-muted">
            No releases published yet. Check back soon, or browse{" "}
            <a
              href={site.releases}
              target="_blank"
              rel="noreferrer"
              className="text-accent hover:underline"
            >
              GitHub Releases
            </a>
            .
          </div>
        ) : (
          <ol className="relative space-y-5 border-l border-white/10 pl-6">
            {releases.map((r) => (
              <li key={r.tag_name} className="relative">
                <span className="absolute top-1.5 -left-[1.7rem] h-2.5 w-2.5 rounded-full bg-accent ring-4 ring-[#050507]" />
                <article className="glass rounded-2xl p-5">
                  <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
                    <h2 className="text-lg font-semibold text-ink">
                      {r.name?.trim() || r.tag_name}
                    </h2>
                    {r.prerelease && (
                      <span className="rounded-full border border-white/15 px-2 py-0.5 text-[10px] tracking-wide text-faint uppercase">
                        Pre-release
                      </span>
                    )}
                    <a
                      href={r.html_url}
                      target="_blank"
                      rel="noreferrer"
                      className="ml-auto font-mono text-xs text-faint transition hover:text-accent"
                    >
                      {r.tag_name}
                    </a>
                  </div>
                  {r.published_at && (
                    <div className="mt-0.5 text-xs text-faint">
                      {fmtDate(r.published_at)}
                    </div>
                  )}
                  <div className="mt-3">
                    <ReleaseNotes body={r.body ?? ""} />
                  </div>
                </article>
              </li>
            ))}
          </ol>
        )}
      </main>

      <Footer />
    </>
  );
}
