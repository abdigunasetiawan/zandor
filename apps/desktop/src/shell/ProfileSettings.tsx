// Kategori "Profil" di Pengaturan: nama tampilan dan avatar (inisial, unggah gambar, atau ilustrasi DiceBear lokal).
// Perubahan langsung disimpan lewat AppearanceStore (tervalidasi) dan dipakai footer sidebar serta sapaan.
import { AVATAR_STYLES, type AvatarStyle, MAX_AVATAR_IMAGE_CHARS, MAX_PROFILE_NAME } from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import {
  Avatar,
  Button,
  Field,
  ImageIcon,
  Input,
  Select,
  ShuffleIcon,
  SparklesIcon,
  Tabs,
  UploadIcon,
  UserIcon,
  useToast,
} from "@zandor/ui";
import { useMemo, useRef, useState } from "react";
import { imageToDataUrl } from "./image-resize";
import { illustrationUrl, randomSeed, styleCredit, useProfile } from "./profile";
import { useAppearance, useServices } from "./services";

const CHOICES = 12;
const errText = (e: unknown) => (e instanceof Error ? e.message : String(e));

export function ProfileSettings() {
  const { t } = useI18n();
  const { appearance: store } = useServices();
  const { profile } = useAppearance();
  const { name, src, osName } = useProfile();
  const toast = useToast();
  const fileRef = useRef<HTMLInputElement>(null);
  const avatar = profile.avatar;
  const [tab, setTab] = useState<string>(avatar?.kind ?? "initials");
  const [style, setStyle] = useState<AvatarStyle>(avatar?.kind === "illustration" ? avatar.style : "lorelei");
  const [seeds, setSeeds] = useState<string[]>(() => {
    const fresh = Array.from({ length: CHOICES }, randomSeed);
    return avatar?.kind === "illustration" ? [avatar.seed, ...fresh.slice(1)] : fresh;
  });
  const credit = useMemo(() => styleCredit(style), [style]);

  const fail = (err: string | null) => {
    if (err) toast.show({ title: t("profile.error", { error: err }), tone: "danger" });
  };
  const upload = async (file: File | undefined) => {
    if (!file) return;
    try {
      const url = await imageToDataUrl(file, {
        maxSide: 256,
        mime: "image/jpeg",
        quality: 0.85,
        maxChars: MAX_AVATAR_IMAGE_CHARS,
      });
      fail(store.setProfileAvatar({ kind: "image", src: url }));
    } catch (e) {
      toast.show({ title: t("profile.uploadError", { error: errText(e) }), tone: "danger" });
    }
  };

  return (
    <section className="settings__group profile" id="settings-profile" aria-labelledby="settings-h-profile">
      <h2 id="settings-h-profile" className="z-h4">
        {t("settings.profile")}
      </h2>

      <div className="profile__card" data-testid="profile-preview">
        <Avatar name={name} src={src} size="lg" />
        <div className="profile__who">
          <span className="z-label">{name}</span>
          <span className="z-small profile__muted">{t("profile.previewHint")}</span>
        </div>
      </div>

      <Field label={t("profile.name")} description={t("profile.nameHint", { os: osName ?? t("sb.user.fallback") })}>
        {(c) => (
          <Input
            {...c}
            value={profile.name ?? ""}
            maxLength={MAX_PROFILE_NAME}
            placeholder={osName ?? t("sb.user.fallback")}
            onChange={(e) => fail(store.setProfileName(e.target.value))}
          />
        )}
      </Field>

      <div className="profile__group">
        <h3 className="z-label profile__head">{t("profile.avatar")}</h3>
        <Tabs
          aria-label={t("profile.avatar")}
          value={tab}
          onValueChange={setTab}
          items={[
            {
              value: "initials",
              label: t("profile.avatar.initials"),
              icon: <UserIcon size="sm" />,
              content: (
                <div className="profile__row">
                  <p className="z-small profile__muted">{t("profile.initialsHint")}</p>
                  <Button size="sm" disabled={!avatar} onClick={() => fail(store.setProfileAvatar(null))}>
                    {t("profile.useInitials")}
                  </Button>
                </div>
              ),
            },
            {
              value: "image",
              label: t("profile.avatar.image"),
              icon: <ImageIcon size="sm" />,
              content: (
                <div className="profile__row">
                  <input
                    ref={fileRef}
                    type="file"
                    accept="image/png,image/jpeg,image/webp,image/gif"
                    hidden
                    aria-label={t("profile.upload")}
                    onChange={(e) => {
                      void upload(e.target.files?.[0]);
                      e.target.value = "";
                    }}
                  />
                  <Button size="sm" onClick={() => fileRef.current?.click()}>
                    <UploadIcon size="sm" />
                    {t("profile.upload")}
                  </Button>
                  <p className="z-small profile__muted">{t("profile.imageHint")}</p>
                </div>
              ),
            },
            {
              value: "illustration",
              label: t("profile.avatar.illustration"),
              icon: <SparklesIcon size="sm" />,
              content: (
                <div className="profile__illu">
                  <div className="profile__row">
                    <Select
                      aria-label={t("profile.style")}
                      size="sm"
                      value={style}
                      onValueChange={(v) => setStyle(v as AvatarStyle)}
                      items={AVATAR_STYLES.map((s) => ({ value: s, label: styleCredit(s).title }))}
                    />
                    <Button size="sm" onClick={() => setSeeds(Array.from({ length: CHOICES }, randomSeed))}>
                      <ShuffleIcon size="sm" />
                      {t("profile.shuffle")}
                    </Button>
                  </div>
                  <ul className="profile__grid" aria-label={t("profile.choices")}>
                    {seeds.map((seed, i) => {
                      const on = avatar?.kind === "illustration" && avatar.style === style && avatar.seed === seed;
                      return (
                        <li key={seed}>
                          <button
                            type="button"
                            className="profile__tile"
                            aria-pressed={on}
                            aria-label={t("profile.pick", { n: i + 1 })}
                            onClick={() => fail(store.setProfileAvatar({ kind: "illustration", style, seed }))}
                          >
                            <img src={illustrationUrl(style, seed)} alt="" />
                          </button>
                        </li>
                      );
                    })}
                  </ul>
                  <p className="z-small profile__muted">
                    {t("profile.credit", { title: credit.title, creator: credit.creator, license: credit.license })}
                  </p>
                </div>
              ),
            },
          ]}
        />
      </div>
    </section>
  );
}
