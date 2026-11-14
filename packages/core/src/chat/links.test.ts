import { describe, expect, test } from "vitest";
import { messageLink, parseMessageLink } from "./links";

describe("tautan pesan", () => {
  test("bolak-balik, termasuk karakter khusus pada kunci project", () => {
    const l = { key: "C:Workalpha", sessionId: "ses_1", messageId: "msg_9" };
    expect(parseMessageLink(messageLink(l))).toEqual(l);
    expect(messageLink({ key: "chat", sessionId: "s", messageId: "m" })).toBe("zandor://chat/chat/s/m");
  });
  test("spasi di tepi diabaikan; bentuk lain ditolak", () => {
    expect(parseMessageLink("  zandor://chat/chat/s/m \n")).toEqual({ key: "chat", sessionId: "s", messageId: "m" });
    for (const bad of [
      "",
      "https://x/y",
      "zandor://chat/a/b",
      "zandor://chat/a/b/c/d",
      "zandor://chat///",
      "zandor://chat/%E0%A4%A/b/c",
    ])
      expect(parseMessageLink(bad), bad).toBeNull();
  });
});
