import { describe, expect, test } from "vitest";
import { type ProviderAccount, type ProviderHost, type ProviderSpec, ProviderStore, type ProviderView } from "./store";

/** Host tiruan dengan akun ganda: mencatat panggilan dan menjaga daftar akun seperti Rust. */
function fakeHost() {
  const calls: string[] = [];
  const specs: ProviderSpec[] = [];
  const accounts = new Map<string, ProviderAccount[]>();
  const host: ProviderHost = {
    secure: true,
    async list(): Promise<ProviderView[]> {
      return specs.map((p) => ({
        ...p,
        hasKey: (accounts.get(p.id)?.length ?? 0) > 0,
        accounts: accounts.get(p.id) ?? [],
      }));
    },
    async save(p) {
      calls.push(`save:${p.id}`);
      if (!specs.some((x) => x.id === p.id)) specs.push(p);
    },
    async remove(id) {
      calls.push(`remove:${id}`);
      specs.splice(
        specs.findIndex((p) => p.id === id),
        1,
      );
      accounts.delete(id);
    },
    async addAccount(id, label, key, activate) {
      calls.push(`add:${id}:${label}:${key}:${activate}`);
      const list = accounts.get(id) ?? [];
      const acc = { id: `a${list.length + 1}`, label, active: activate || list.length === 0 };
      accounts.set(id, [...(activate ? list.map((a) => ({ ...a, active: false })) : list), acc]);
      return acc.id;
    },
    async activateAccount(id, acct) {
      calls.push(`use:${id}:${acct}`);
      accounts.set(
        id,
        (accounts.get(id) ?? []).map((a) => ({ ...a, active: a.id === acct })),
      );
    },
    async renameAccount(id, acct, label) {
      calls.push(`rename:${id}:${acct}:${label}`);
      accounts.set(
        id,
        (accounts.get(id) ?? []).map((a) => (a.id === acct ? { ...a, label } : a)),
      );
    },
    async removeAccount(id, acct) {
      calls.push(`del:${id}:${acct}`);
      const left = (accounts.get(id) ?? []).filter((a) => a.id !== acct);
      accounts.set(id, left);
      return left.length;
    },
    probe: async () => {
      throw new Error("tidak dipakai");
    },
    ping: async () => {
      throw new Error("tidak dipakai");
    },
    detectLocal: async () => [],
    openExternal: async () => {},
  };
  return { host, calls, accounts };
}

describe("ProviderStore: akun ganda (ADR-056)", () => {
  test("addAccount membuat provider katalog dulu (tanpa kunci), lalu menambah akun; kunci hanya lewat host", async () => {
    const { host, calls } = fakeHost();
    let changed = 0;
    const store = new ProviderStore({ host, onChanged: () => changed++ });
    await store.refresh();
    const id = await store.addAccount({ providerId: "openai", name: "OpenAI", label: " Kantor ", apiKey: " sk-1 " });
    expect(id).toBe("a1");
    expect(calls).toEqual(["save:openai", "add:openai:Kantor:sk-1:true"]);
    await store.addAccount({ providerId: "openai", name: "OpenAI", label: "Pribadi", apiKey: "sk-2" });
    // provider sudah ada → tidak disimpan ulang
    expect(calls.filter((c) => c.startsWith("save:"))).toHaveLength(1);
    const p = store.state().providers[0];
    expect(p?.accounts?.map((a) => [a.label, a.active])).toEqual([
      ["Kantor", false],
      ["Pribadi", true],
    ]);
    expect(JSON.stringify(store.state())).not.toContain("sk-1");
    expect(changed).toBe(2);
  });

  test("kunci kosong ditolak; host tanpa dukungan akun memberi galat jelas", async () => {
    const { host } = fakeHost();
    const store = new ProviderStore({ host });
    await expect(store.addAccount({ providerId: "x", name: "X", label: "", apiKey: "  " })).rejects.toThrow(/kunci/);
    const { addAccount: _a, ...bare } = host;
    const plain = new ProviderStore({ host: bare });
    await expect(plain.addAccount({ providerId: "x", name: "X", label: "", apiKey: "k" })).rejects.toThrow(
      /akun ganda/,
    );
  });

  test("activate, rename, dan remove memperbarui daftar; menghapus akun terakhir provider katalog melepas provider", async () => {
    const { host, calls } = fakeHost();
    const store = new ProviderStore({ host });
    await store.addAccount({ providerId: "openai", name: "OpenAI", label: "A", apiKey: "k1" });
    await store.addAccount({ providerId: "openai", name: "OpenAI", label: "B", apiKey: "k2" });
    await store.activateAccount("openai", "a1");
    expect(store.state().providers[0]?.accounts?.find((a) => a.active)?.id).toBe("a1");
    await store.renameAccount("openai", "a2", " Kerja ");
    expect(store.state().providers[0]?.accounts?.find((a) => a.id === "a2")?.label).toBe("Kerja");
    await store.removeAccount("openai", "a2");
    expect(store.state().providers).toHaveLength(1);
    await store.removeAccount("openai", "a1");
    expect(store.state().providers).toHaveLength(0);
    expect(calls.at(-1)).toBe("remove:openai");
  });
});
