// Render Markdown jawaban asisten. Aman secara bawaan: react-markdown tidak merender HTML mentah; tautan dibuka
// di jendela terpisah tanpa referrer (sumbernya model, jadi tidak dipercaya).
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";

const SAFE_LINK = /^(https?:|mailto:)/i;

export function Markdown({ text }: { text: string }) {
  return (
    <div className="md">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        components={{
          a: ({ href, children }) =>
            href && SAFE_LINK.test(href) ? (
              <a href={href} target="_blank" rel="noreferrer noopener">
                {children}
              </a>
            ) : (
              <span>{children}</span>
            ),
          img: ({ alt }) => <span className="md__img">{alt ?? ""}</span>,
        }}
      >
        {text}
      </ReactMarkdown>
    </div>
  );
}
