import { useMemo } from "react";
import { marked } from "marked";
import DOMPurify from "dompurify";
import { api } from "../lib/api";

marked.setOptions({ gfm: true, breaks: true });

export default function Markdown({ text }: { text: string }) {
  const html = useMemo(() => DOMPurify.sanitize(marked.parse(text, { async: false }) as string), [text]);
  return (
    <div
      className="md"
      dangerouslySetInnerHTML={{ __html: html }}
      onClick={(e) => {
        const a = (e.target as HTMLElement).closest("a");
        if (a?.href) {
          e.preventDefault();
          api.openUrl(a.href);
        }
      }}
    />
  );
}
