"use client";

import { useEffect, useState } from "react";
import { site } from "@/lib/site";

/**
 * Shows the latest published release version, fetched at runtime from GitHub so
 * it always matches the newest tagged release without needing a site redeploy.
 * Renders nothing until (and unless) a version is resolved.
 */
export function LatestVersion({ className }: { className?: string }) {
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    fetch(`https://api.github.com/repos/${site.repo}/releases/latest`, {
      headers: { Accept: "application/vnd.github+json" },
    })
      .then((r) => (r.ok ? r.json() : null))
      .then((d) => {
        const tag = d?.tag_name;
        if (active && typeof tag === "string") {
          setVersion(tag.replace(/^v/, ""));
        }
      })
      .catch(() => {});
    return () => {
      active = false;
    };
  }, []);

  if (!version) return null;
  return <span className={className}>v{version}</span>;
}
