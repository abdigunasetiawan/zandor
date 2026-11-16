// @vitest-environment jsdom
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useRef, useState } from "react";
import { describe, expect, it } from "vitest";
import { useSmoothCaret } from "./smooth-caret";

function Harness() {
  const ref = useRef<HTMLTextAreaElement>(null);
  const [text, setText] = useState("");
  const caret = useSmoothCaret(ref);
  return (
    <div>
      <textarea aria-label="input" ref={ref} value={text} onChange={(e) => setText(e.target.value)} />
      <span data-testid="caret" data-visible={caret.visible || undefined} />
    </div>
  );
}

describe("useSmoothCaret", () => {
  it("tidak mengganggu teks yang diketik dan menampilkan kursor saat fokus", async () => {
    render(<Harness />);
    const input = screen.getByLabelText("input") as HTMLTextAreaElement;
    input.focus();
    fireEvent.change(input, { target: { value: "halo" } });
    fireEvent.input(input);
    expect(input.value).toBe("halo");
    await waitFor(() => expect(screen.getByTestId("caret").dataset.visible).toBe("true"));
  });
});
