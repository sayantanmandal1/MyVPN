import { Download } from "lucide-react";
import { Button } from "@/components/ui/button";
import { GithubMark } from "@/components/site/icons";
import { Reveal } from "@/components/ui/reveal";
import { site } from "@/lib/site";

export function DownloadCta() {
  return (
    <section className="px-4 py-24">
      <Reveal className="mx-auto max-w-3xl">
        <div className="glass-strong relative overflow-hidden rounded-3xl px-6 py-14 text-center sm:px-12">
          <div
            className="absolute left-1/2 top-0 h-40 w-[420px] -translate-x-1/2 rounded-full opacity-50 blur-[80px]"
            style={{
              background:
                "radial-gradient(circle, rgba(52,211,153,0.4), transparent 70%)",
            }}
          />
          <h2 className="relative text-balance text-3xl font-semibold tracking-tight sm:text-4xl">
            Spin up your VPN in under a minute
          </h2>
          <p className="relative mx-auto mt-3 max-w-md text-pretty text-[15px] text-muted">
            Free, open source, and entirely yours. Download the installer and host
            your first network today.
          </p>
          <div className="relative mt-8 flex flex-col items-center justify-center gap-3 sm:flex-row">
            <Button asChild size="lg">
              <a href={site.download}>
                <Download className="h-4 w-4" />
                Download for Windows
              </a>
            </Button>
            <Button asChild variant="outline" size="lg">
              <a href={site.github} target="_blank" rel="noreferrer">
                <GithubMark className="h-4 w-4" />
                Star on GitHub
              </a>
            </Button>
          </div>
          <p className="relative mt-4 text-xs text-faint">
            Windows 10 / 11 · installs in seconds · auto-updating releases
          </p>
        </div>
      </Reveal>
    </section>
  );
}
