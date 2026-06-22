import type { ReactNode } from "react";

// Minimal, safe inline markdown: **bold**, `code`, and [text](https://url).
// Everything is rendered as React elements (auto-escaped); links are restricted
// to http(s), so release notes can't inject markup.
function inline(text: string, keyBase: string): ReactNode[] {
  const nodes: ReactNode[] = [];
  const re =
    /(\*\*([^*]+)\*\*)|(`([^`]+)`)|(\[([^\]]+)\]\((https?:\/\/[^)\s]+)\))/g;
  let last = 0;
  let i = 0;
  let m: RegExpExecArray | null;
  while ((m = re.exec(text)) !== null) {
    if (m.index > last) nodes.push(text.slice(last, m.index));
    if (m[2] !== undefined) {
      nodes.push(
        <strong key={`${keyBase}-b${i}`} className="font-semibold text-ink">
          {m[2]}
        </strong>,
      );
    } else if (m[4] !== undefined) {
      nodes.push(
        <code
          key={`${keyBase}-c${i}`}
          className="rounded bg-white/10 px-1 py-0.5 font-mono text-[0.85em]"
        >
          {m[4]}
        </code>,
      );
    } else if (m[6] !== undefined && m[7] !== undefined) {
      nodes.push(
        <a
          key={`${keyBase}-a${i}`}
          href={m[7]}
          target="_blank"
          rel="noreferrer"
          className="text-accent underline-offset-2 hover:underline"
        >
          {m[6]}
        </a>,
      );
    }
    last = m.index + m[0].length;
    i++;
  }
  if (last < text.length) nodes.push(text.slice(last));
  return nodes;
}

/**
 * Renders GitHub release notes (markdown) with a small, dependency-free parser
 * that covers what release notes actually use: headings, bullet lists,
 * blockquotes, paragraphs, and inline bold/code/links.
 */
export function ReleaseNotes({ body }: { body: string }) {
  const lines = body.replace(/\r\n/g, "\n").split("\n");
  const blocks: ReactNode[] = [];
  let list: string[] = [];
  let para: string[] = [];
  let key = 0;

  const flushList = () => {
    if (!list.length) return;
    const items = [...list];
    const k = key++;
    blocks.push(
      <ul key={`ul${k}`} className="my-2 space-y-1.5">
        {items.map((it, idx) => (
          <li
            key={idx}
            className="flex gap-2 text-sm leading-relaxed text-muted"
          >
            <span className="mt-[0.5rem] h-1 w-1 shrink-0 rounded-full bg-accent/70" />
            <span>{inline(it, `li${k}-${idx}`)}</span>
          </li>
        ))}
      </ul>,
    );
    list = [];
  };

  const flushPara = () => {
    if (!para.length) return;
    const text = para.join(" ");
    const k = key++;
    blocks.push(
      <p key={`p${k}`} className="my-2 text-sm leading-relaxed text-muted">
        {inline(text, `p${k}`)}
      </p>,
    );
    para = [];
  };

  for (const raw of lines) {
    const trimmed = raw.trim();
    if (trimmed === "") {
      flushList();
      flushPara();
      continue;
    }
    const heading = /^#{1,6}\s+(.*)$/.exec(trimmed);
    const bullet = /^[-*]\s+(.*)$/.exec(trimmed);
    const quote = /^>\s?(.*)$/.exec(trimmed);
    if (heading) {
      flushList();
      flushPara();
      const k = key++;
      blocks.push(
        <h3 key={`h${k}`} className="mt-4 mb-1 text-sm font-semibold text-ink">
          {inline(heading[1], `h${k}`)}
        </h3>,
      );
    } else if (bullet) {
      flushPara();
      list.push(bullet[1]);
    } else if (quote) {
      flushList();
      flushPara();
      const k = key++;
      blocks.push(
        <blockquote
          key={`q${k}`}
          className="my-2 border-l-2 border-accent/40 pl-3 text-sm text-faint"
        >
          {inline(quote[1], `q${k}`)}
        </blockquote>,
      );
    } else {
      flushList();
      para.push(trimmed);
    }
  }
  flushList();
  flushPara();

  if (blocks.length === 0) {
    return <p className="text-sm text-faint">No notes for this release.</p>;
  }
  return <div>{blocks}</div>;
}
