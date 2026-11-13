// @vitest-environment jsdom
import { cleanup, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ShortcutRegistry } from "@zandor/core";
import { afterEach, describe, expect, test, vi } from "vitest";
import { Dialog, Input, ShortcutProvider, ShortcutScope, scopesFromElement, useShortcut } from "../index";
import { renderUi } from "../test-utils";

afterEach(cleanup);

function Bind({
  id,
  combo,
  scope,
  run,
  allowInModal,
}: {
  id: string;
  combo: string;
  scope: string;
  run: () => void;
  allowInModal?: boolean;
}) {
  useShortcut({ id, combo, scope, description: "test", run, allowInModal });
  return null;
}

describe("scopesFromElement", () => {
  test("terdalam dulu, global diabaikan", () => {
    document.body.innerHTML =
      '<div data-shortcut-scope="view.settings"><div data-shortcut-scope="panel.chat"><button id="b">x</button></div></div>';
    expect(scopesFromElement(document.getElementById("b"))).toEqual(["panel.chat", "view.settings"]);
    expect(scopesFromElement(null)).toEqual([]);
  });
});

describe("ShortcutProvider + useShortcut", () => {
  test("global jalan dari mana saja; scope panel hanya saat fokus di dalamnya", async () => {
    const user = userEvent.setup();
    const calls: string[] = [];
    renderUi(
      <ShortcutProvider registry={new ShortcutRegistry()}>
        <Bind id="g" combo="Ctrl+E" scope="global" run={() => calls.push("global")} />
        <Bind id="c" combo="Ctrl+E" scope="panel.chat" run={() => calls.push("chat")} />
        <ShortcutScope name="panel.chat">
          <button type="button">di chat</button>
        </ShortcutScope>
        <ShortcutScope name="panel.docs">
          <button type="button">di dokumen</button>
        </ShortcutScope>
      </ShortcutProvider>,
    );
    screen.getByRole("button", { name: "di chat" }).focus();
    await user.keyboard("{Control>}e{/Control}");
    screen.getByRole("button", { name: "di dokumen" }).focus();
    await user.keyboard("{Control>}e{/Control}");
    expect(calls).toEqual(["chat", "global"]);
  });

  test("tanpa modifier tidak jalan saat mengetik di input", async () => {
    const user = userEvent.setup();
    const run = vi.fn();
    renderUi(
      <ShortcutProvider registry={new ShortcutRegistry()}>
        <Bind id="slash" combo="/" scope="global" run={run} />
        <Input aria-label="kolom" />
        <button type="button">tombol</button>
      </ShortcutProvider>,
    );
    await user.click(screen.getByRole("textbox", { name: "kolom" }));
    await user.keyboard("/");
    expect(run).not.toHaveBeenCalled();
    screen.getByRole("button", { name: "tombol" }).focus();
    await user.keyboard("/");
    expect(run).toHaveBeenCalledOnce();
  });

  test("handler terbaru dipakai tanpa daftar ulang; dilepas saat unmount", async () => {
    const user = userEvent.setup();
    const registry = new ShortcutRegistry();
    const a = vi.fn();
    const b = vi.fn();
    const { rerender, unmount } = renderUi(
      <ShortcutProvider registry={registry}>
        <Bind id="x" combo="Ctrl+J" scope="global" run={a} />
      </ShortcutProvider>,
    );
    rerender(
      <ShortcutProvider registry={registry}>
        <Bind id="x" combo="Ctrl+J" scope="global" run={b} />
      </ShortcutProvider>,
    );
    await user.keyboard("{Control>}j{/Control}");
    expect(a).not.toHaveBeenCalled();
    expect(b).toHaveBeenCalledOnce();
    unmount();
    expect(registry.list()).toHaveLength(0);
  });

  test("dialog (scope modal) memblokir shortcut global", async () => {
    const user = userEvent.setup();
    const run = vi.fn();
    renderUi(
      <ShortcutProvider registry={new ShortcutRegistry()}>
        <Bind id="g" combo="Ctrl+B" scope="global" run={run} />
        <Dialog open onOpenChange={() => {}} title="Modal">
          <button type="button">dalam modal</button>
        </Dialog>
      </ShortcutProvider>,
    );
    const inModal = await screen.findByRole("button", { name: "dalam modal" });
    inModal.focus();
    await user.keyboard("{Control>}b{/Control}");
    expect(run).not.toHaveBeenCalled();
  });
});
