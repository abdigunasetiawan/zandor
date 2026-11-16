// Panel 4 — Git: status, stage/unstage, diff, commit, cabang. Menjalankan `git` CLI lewat Rust di project terdaftar.
import { CHAT_KEY, type FileChange, groupChanges, isUntracked } from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import {
  ArchiveIcon,
  Button,
  DownloadIcon,
  GitIcon,
  IconButton,
  Input,
  PlusIcon,
  RefreshIcon,
  Select,
  Spinner,
  Textarea,
  UndoIcon,
  UploadIcon,
  useConfirm,
  useToast,
} from "@zandor/ui";
import { type FormEvent, useEffect, useState } from "react";
import { useGit, useProjects, useServices } from "../services";
import { DiffText } from "./DiffText";
import { ToolNotice } from "./ToolNotice";

const errText = (e: unknown) => (e instanceof Error ? e.message : String(e));

function statusLabel(t: ReturnType<typeof useI18n>["t"], f: FileChange, staged: boolean): string {
  if (isUntracked(f)) return t("git.s.?");
  const c = staged ? f.index : f.worktree;
  const known = ["M", "A", "D", "R", "C", "T", "U"];
  return known.includes(c) ? t(`git.s.${c}` as never) : c;
}

export function GitPanel() {
  const { t } = useI18n();
  const { git, activity } = useServices();
  const proj = useProjects();
  const g = useGit();
  const toast = useToast();
  const confirm = useConfirm();
  const [message, setMessage] = useState("");
  const [newBranch, setNewBranch] = useState<string | null>(null);

  const project = proj.projects.find((p) => p.id === (proj.selection.key ?? CHAT_KEY));
  const root = project?.path ?? null;

  // muat saat project aktif berubah atau engine siap
  // biome-ignore lint/correctness/useExhaustiveDependencies: `git` stabil; hanya root yang relevan
  useEffect(() => {
    void git.setRoot(root);
  }, [root]);

  const fail = (e: unknown) => {
    activity.add("git", "error", errText(e));
    toast.show({ title: t("sb.error", { error: errText(e) }), tone: "danger" });
  };
  const run = async (p: Promise<unknown>) => {
    try {
      await p;
    } catch (e) {
      fail(e);
    }
  };

  if (!git.available) return <ToolNotice icon={<GitIcon size="lg" />}>{t("git.unavailable")}</ToolNotice>;
  if (!root) return <ToolNotice icon={<GitIcon size="lg" />}>{t("git.noProject")}</ToolNotice>;
  if (g.loading && !g.status) {
    return (
      <p className="sidebar__note z-small" role="status">
        <Spinner size="sm" /> {t("git.loading")}
      </p>
    );
  }
  if (g.error && !g.status) {
    return (
      <div className="gitp">
        <p className="chat__note chat__note--error z-small" role="alert">
          {g.error}
        </p>
        <Button onClick={() => void git.refresh()}>{t("git.refresh")}</Button>
      </div>
    );
  }
  if (!g.status?.isRepo) {
    return (
      <div className="gitp">
        <p className="z-p">{t("git.notRepo")}</p>
        <p className="z-small gitp__clean">{t("git.notRepoHint")}</p>
        <Button onClick={() => void git.refresh()}>{t("git.refresh")}</Button>
      </div>
    );
  }

  const st = g.status;
  const groups = groupChanges(st.files);
  const commit = async (e: FormEvent) => {
    e.preventDefault();
    if (groups.staged.length === 0) {
      toast.show({ title: t("git.commitNeedStaged"), tone: "warning" });
      return;
    }
    try {
      const hash = await git.commit(message);
      setMessage("");
      activity.add("git", "info", `commit ${hash}`);
      toast.show({ title: t("git.commitDone", { hash }), tone: "success" });
    } catch (err) {
      fail(err);
    }
  };

  const discard = async (paths: string[], label: string) => {
    const ok = await confirm({
      title: t("git.discardTitle", { name: label }),
      description: t("git.discardBody"),
      confirmLabel: t("git.discard"),
      tone: "danger",
    });
    if (!ok) return;
    try {
      await git.discard(paths);
      activity.add("git", "info", `discard ${paths.length} berkas`);
    } catch (e) {
      fail(e);
    }
  };

  const net = async (kind: "push" | "pull") => {
    try {
      const out = kind === "push" ? await git.push() : await git.pull();
      activity.add("git", "info", `${kind}: ${out || "ok"}`);
      toast.show({ title: t(kind === "push" ? "git.pushed" : "git.pulled"), tone: "success" });
    } catch (e) {
      fail(e);
    }
  };

  const row = (f: FileChange, staged: boolean) => (
    <li key={`${staged ? "s" : "u"}:${f.path}`} className="gitp__file">
      <button
        type="button"
        className="gitp__name"
        title={f.path}
        aria-label={t("git.diff", { name: f.path })}
        onClick={() => void git.showDiff(f, staged)}
      >
        <span className="gitp__badge" data-kind={staged ? f.index : isUntracked(f) ? "?" : f.worktree}>
          {statusLabel(t, f, staged)}
        </span>
        <span className="tree__label">{f.origPath ? `${f.origPath} → ${f.path}` : f.path}</span>
      </button>
      <Button
        size="sm"
        disabled={g.busy}
        aria-label={staged ? t("git.unstage", { name: f.path }) : t("git.stage", { name: f.path })}
        onClick={() => void run(staged ? git.unstage([f.path]) : git.stage([f.path]))}
      >
        {staged ? "−" : "+"}
      </Button>
      {!staged && git.canDiscard && (
        <IconButton
          size="sm"
          label={t("git.discardFile", { name: f.path })}
          icon={<UndoIcon />}
          disabled={g.busy}
          onClick={() => void discard([f.path], f.path)}
        />
      )}
    </li>
  );

  const repoLabel = (dir: string) => {
    const base = (g.projectRoot ?? "").replace(/[\\/]+$/, "");
    const rel = dir.length > base.length ? dir.slice(base.length + 1) : dir;
    return rel.replace(/\\/g, "/");
  };
  return (
    <div className="gitp">
      <div className="gitp__bar">
        {g.repos.length > 0 && (
          <Select
            aria-label={t("git.repo")}
            size="sm"
            className="gitp__select"
            items={g.repos.map((r) => ({ value: r, label: repoLabel(r) }))}
            value={g.root}
            onValueChange={(dir) => void git.selectRepo(dir)}
          />
        )}
        {g.branches.length > 0 && !st.detached ? (
          <Select
            aria-label={t("git.switchTo")}
            size="sm"
            className="gitp__select"
            items={g.branches.map((b) => ({ value: b.name, label: b.name }))}
            value={g.branches.find((b) => b.current)?.name ?? null}
            onValueChange={(name) => void run(git.switchBranch(name))}
          />
        ) : (
          <span className="gitp__branch" data-testid="git-branch">
            {st.detached ? t("git.detached") : (st.branch ?? "?")}
          </span>
        )}
        <span className="gitp__spacer" />
        <IconButton
          label={t("git.newBranch")}
          size="sm"
          icon={<PlusIcon />}
          onClick={() => setNewBranch(newBranch === null ? "" : null)}
        />
        <IconButton label={t("git.refresh")} size="sm" icon={<RefreshIcon />} onClick={() => void git.refresh()} />
      </div>
      {(git.canNetwork || git.canStash) && (
        <div className="gitp__sync" role="toolbar" aria-label={t("git.sync")}>
          {git.canNetwork && (
            <>
              <Button size="sm" disabled={g.busy || !st.upstream} onClick={() => void net("pull")}>
                <DownloadIcon size="sm" /> {t("git.pull")}
                {st.behind > 0 && <span className="z-small"> ↓{st.behind}</span>}
              </Button>
              <Button size="sm" disabled={g.busy} onClick={() => void net("push")}>
                <UploadIcon size="sm" /> {t("git.push")}
                {st.ahead > 0 && <span className="z-small"> ↑{st.ahead}</span>}
              </Button>
            </>
          )}
          {git.canStash && (
            <>
              <Button size="sm" disabled={g.busy || st.files.length === 0} onClick={() => void run(git.stashPush(""))}>
                <ArchiveIcon size="sm" /> {t("git.stash")}
              </Button>
              <Button size="sm" disabled={g.busy || g.stashes === 0} onClick={() => void run(git.stashPop())}>
                {t("git.stashPop")}
                {g.stashes > 0 && <span className="z-small"> ({g.stashes})</span>}
              </Button>
            </>
          )}
        </div>
      )}
      {newBranch !== null && (
        <form
          className="pform__row"
          onSubmit={(e) => {
            e.preventDefault();
            void run(git.createBranch(newBranch).then(() => setNewBranch(null)));
          }}
        >
          <Input
            aria-label={t("git.branchName")}
            placeholder={t("git.branchName")}
            value={newBranch}
            onChange={(e) => setNewBranch(e.target.value)}
          />
          <Button type="submit" variant="solid" disabled={!newBranch.trim() || g.busy}>
            {t("git.create")}
          </Button>
        </form>
      )}
      {g.error && (
        <p className="chat__note chat__note--error z-small" role="alert">
          {g.error}
        </p>
      )}

      {st.files.length === 0 && <p className="z-small gitp__clean">{t("git.clean")}</p>}
      {groups.conflicts.length > 0 && (
        <section aria-label={t("git.conflicts")}>
          <h3 className="z-label">{t("git.conflicts")}</h3>
          <ul className="gitp__list">{groups.conflicts.map((f) => row(f, false))}</ul>
        </section>
      )}
      {groups.staged.length > 0 && (
        <section aria-label={t("git.staged")}>
          <h3 className="z-label gitp__h">
            {t("git.staged")} ({groups.staged.length})
            <Button size="sm" disabled={g.busy} onClick={() => void run(git.unstage(groups.staged.map((f) => f.path)))}>
              {t("git.unstageAll")}
            </Button>
          </h3>
          <ul className="gitp__list">{groups.staged.map((f) => row(f, true))}</ul>
        </section>
      )}
      {groups.unstaged.length > 0 && (
        <section aria-label={t("git.changes")}>
          <h3 className="z-label gitp__h">
            {t("git.changes")} ({groups.unstaged.length})
            <Button size="sm" disabled={g.busy} onClick={() => void run(git.stage(groups.unstaged.map((f) => f.path)))}>
              {t("git.stageAll")}
            </Button>
            {git.canDiscard && (
              <Button
                size="sm"
                variant="danger"
                disabled={g.busy}
                onClick={() =>
                  void discard(
                    groups.unstaged.map((f) => f.path),
                    t("git.discardAllName", { n: groups.unstaged.length }),
                  )
                }
              >
                {t("git.discardAll")}
              </Button>
            )}
          </h3>
          <ul className="gitp__list">{groups.unstaged.map((f) => row(f, false))}</ul>
        </section>
      )}

      {g.diff && (
        <section aria-label={t("git.diffTitle", { path: g.diff.path })} className="gitp__diff">
          <h3 className="z-label gitp__h">
            {t("git.diffTitle", { path: g.diff.path })}
            <Button size="sm" onClick={() => git.closeDiff()}>
              {t("git.closeDiff")}
            </Button>
          </h3>
          {g.diff.error && <p className="chat__note chat__note--error z-small">{g.diff.error}</p>}
          {g.diff.text === null && !g.diff.error && <Spinner size="sm" />}
          {g.diff.text !== null &&
            (g.diff.text ? <DiffText text={g.diff.text} /> : <p className="z-small">{t("git.diffEmpty")}</p>)}
        </section>
      )}

      <form className="gitp__commit" onSubmit={(e) => void commit(e)}>
        <Textarea
          rows={2}
          aria-label={t("git.commitMsg")}
          placeholder={t("git.commitMsg")}
          value={message}
          onChange={(e) => setMessage(e.target.value)}
        />
        <Button type="submit" variant="solid" disabled={g.busy || !message.trim() || groups.staged.length === 0}>
          {t("git.commit")}
        </Button>
      </form>

      {g.log.length > 0 && (
        <section aria-label={t("git.recent")} className="gitp__section">
          <h3 className="z-label gitp__h">{t("git.recent")}</h3>
          <ol className="gitp__log">
            {g.log.map((c) => (
              <li key={c.hash} className="gitp__entry" title={`${c.hash} ${c.subject}`}>
                <span className="gitp__subject">{c.subject}</span>
                <span className="gitp__meta z-small">
                  <code className="gitp__hash">{c.hash}</code>
                  <span className="gitp__who">{c.author}</span>
                  <span className="gitp__when">{c.date}</span>
                </span>
              </li>
            ))}
          </ol>
        </section>
      )}
    </div>
  );
}
