<script lang="ts">
  import { untrack } from "svelte";
  import { api } from "$lib/api";
  import { confirmAction } from "$lib/confirm.svelte";
  import { openDialog } from "$lib/dialogs.svelte";
  import { fileSize, formatCode } from "$lib/format";
  import { social } from "$lib/stores";
  import type { AppSettings, AuthProvider } from "$lib/types";
  import Button from "../Button.svelte";
  import Dialog from "../Dialog.svelte";
  import Head from "../Head.svelte";
  import ProviderIcon from "../ProviderIcon.svelte";
  import SignInButtons from "../SignInButtons.svelte";
  import Switch from "../Switch.svelte";

  let { onclose }: { onclose: () => void } = $props();

  const me = $derived(social.session?.user ?? null);

  // ── Profile: a draft form ──
  // Copy the saved values ONCE when the dialog opens. untrack() reads without
  // subscribing, so a store reload (new session object) can't wipe half-typed edits.
  let displayName = $state(untrack(() => me?.displayName ?? ""));
  let username = $state(untrack(() => me?.username ?? ""));

  const usernameError = $derived(
    /^[a-z0-9_]{3,16}$/.test(username)
      ? null
      : "3–16 characters: lowercase letters, numbers or _",
  );
  const displayNameError = $derived(
    displayName.trim().length === 0
      ? "Required"
      : displayName.trim().length > 32
        ? "32 characters at most"
        : null,
  );
  // Rename rate limit: the server says when the next change is allowed.
  const usernameLockedUntil = $derived(
    me?.usernameChangeAvailableAt &&
      new Date(me.usernameChangeAvailableAt) > new Date()
      ? new Date(me.usernameChangeAvailableAt)
      : null,
  );
  // "Dirty" = differs from what's saved. Save is only enabled when there's something to save.
  const dirty = $derived(
    !!me && (displayName.trim() !== me.displayName || username !== me.username),
  );
  const canSave = $derived(dirty && !usernameError && !displayNameError);

  let saving = $state(false);
  let profileMessage = $state<{ text: string; ok: boolean } | null>(null);

  async function saveProfile(e: SubmitEvent) {
    e.preventDefault();
    if (!canSave || !me) return;
    saving = true;
    profileMessage = null;
    try {
      // Only send what changed: a username change is rate-limited on the server.
      await social.updateProfile({
        ...(displayName.trim() !== me.displayName
          ? { displayName: displayName.trim() }
          : {}),
        ...(username !== me.username ? { username } : {}),
      });
      profileMessage = { text: "Saved", ok: true };
    } catch (err) {
      profileMessage = { text: (err as Error).message, ok: false };
    } finally {
      saving = false;
    }
  }

  // ── Everything else saves instantly ──
  // Shown under the row that failed, so it's in view on a small screen.
  let error = $state<{ key: string; text: string } | null>(null);
  let busy = $state<string | null>(null);
  let attempt = 0;
  let exported = $state<string | null>(null);

  // This PC's settings, which aren't part of the account.
  let pc = $state<AppSettings | null>(null);
  api
    .getAppSettings()
    .then((s) => (pc = s))
    .catch(() => {});
  const PC_SETTINGS: {
    key: keyof AppSettings;
    group: string;
    title: string;
    hint: string;
  }[] = [
    {
      key: "autoUpdate",
      group: "Updates",
      title: "Keep new packs up to date automatically",
      hint: "Where the switch starts when you add a pack. Change it for each pack on its Sharing tab.",
    },
    {
      key: "notifications",
      group: "Notifications",
      title: "Notifications",
      hint: "Only when something needs you: a pack shared with you, a friend request, or an update that needs your say. Held until you close the game.",
    },
    {
      key: "notificationSound",
      group: "Notifications",
      title: "Notification sound",
      hint: "A quiet sound with each one. Silent while Windows is set to Do not disturb.",
    },
    {
      key: "closeToTray",
      group: "Startup",
      title: "Keep running in the tray",
      hint: "Closing the window leaves Atlas in the tray, so it keeps your packs up to date.",
    },
    {
      key: "startWithWindows",
      group: "Startup",
      title: "Start with Windows",
      hint: "Atlas starts in the tray when you sign in to Windows.",
    },
  ];

  // A section's card, and one setting in it: words on the left, the control on the right.
  const CARD =
    "divide-y divide-(--line) overflow-hidden rounded-lg border border-(--line) bg-(--bg)";
  const ROW = "flex items-start gap-4 px-4 py-3.5";

  async function act(key: string, action: () => Promise<unknown>) {
    const mine = ++attempt;
    busy = key;
    error = null;
    try {
      await action();
    } catch (err) {
      // Linking again while the browser waits starts over and rejects the first try.
      if (mine === attempt) error = { key, text: (err as Error).message };
    } finally {
      if (mine === attempt) busy = null;
    }
  }

  const PROVIDERS: { id: AuthProvider; label: string }[] = [
    { id: "minecraft", label: "Minecraft" },
    { id: "discord", label: "Discord" },
  ];

  /** The two pictures to choose from, as `Head` draws them. */
  const pictures = $derived(
    me
      ? [
          {
            id: "minecraft" as const,
            label: "Minecraft skin",
            user: {
              displayName: me.displayName,
              skinUrl: me.minecraftSkinUrl,
              avatarUrl: null,
            },
            available: !!me.minecraftSkinUrl,
            missing: me.identities.some((i) => i.provider === "minecraft")
              ? "You're using a default skin"
              : "Link Minecraft below",
          },
          {
            id: "discord" as const,
            label: "Discord picture",
            user: {
              displayName: me.displayName,
              skinUrl: null,
              avatarUrl: me.discordAvatarUrl,
            },
            available: !!me.discordAvatarUrl,
            missing: me.identities.some((i) => i.provider === "discord")
              ? "Your Discord account has no picture"
              : "Link Discord below",
          },
        ]
      : [],
  );

  /** Waiting for the browser to link an account: another Link starts over with it. */
  const linking = $derived(PROVIDERS.some((p) => p.id === busy));

  function cancelLink() {
    attempt++;
    busy = null;
    error = null;
    api.cancelSignIn().catch(() => {});
  }

  function signOut() {
    act("signout", async () => {
      await social.signOut();
      onclose();
    });
  }

  async function deleteAccount() {
    if (!me) return;
    const ok = await confirmAction({
      title: "Delete your Atlas account?",
      message:
        "Your account, friends and shares are deleted for good. Your instances stay on this PC.",
      confirmLabel: "Delete account",
      danger: true,
      typeToConfirm: me.username,
    });
    if (!ok) return;
    act("delete", async () => {
      await social.deleteAccount();
      onclose();
    });
  }
</script>

{#snippet failed(key: string)}
  {#if error?.key === key}
    <p class="mt-1.5 text-[13px] text-(--danger)" role="alert">{error.text}</p>
  {/if}
{/snippet}

<!-- Each section: a title, then a card whose settings are rows with a line between. -->
{#snippet section(title: string)}
  <h3
    class="mt-6 mb-2 text-[13px] font-semibold text-(--text-muted) first:mt-1"
  >
    {title}
  </h3>
{/snippet}

<!-- This PC: saves the moment you flip it, like Privacy. Signed in or not. -->
{#snippet thisPc()}
  {#if pc}
    {@const settings = pc}
    {#each ["Updates", "Notifications", "Startup"] as group (group)}
      {@render section(group)}
      <div class={CARD}>
        {#each PC_SETTINGS.filter((s) => s.group === group) as s (s.key)}
          <div class={ROW}>
            <div class="min-w-0 flex-1">
              <b class="block font-medium">{s.title}</b>
              <small class="block text-[13px] text-(--text-faint)"
                >{s.hint}</small
              >
              {@render failed(s.key)}
            </div>
            <Switch
              checked={settings[s.key]}
              label={s.title}
              disabled={busy === s.key}
              onchange={(next) =>
                act(s.key, async () => {
                  pc = await api.updateAppSettings({ [s.key]: next });
                })}
            />
          </div>
        {/each}
      </div>
    {/each}
  {/if}
{/snippet}

<Dialog
  title={me?.displayName ?? "Settings"}
  description={me ? `@${me.username}` : undefined}
  wide
  {onclose}
>
  {#snippet header()}
    {#if me}<Head user={me} size={40} ring="var(--bg-raise)" />{/if}
  {/snippet}

  {#if !me}
    <div class="pt-2 pb-5">
      <p class="text-[13px] text-(--text-muted)">
        You're signed out. Sign in to share packs and add friends. Your packs on
        this PC keep working either way.
      </p>
      <SignInButtons start />
    </div>
    {@render thisPc()}
  {:else}
    <!-- ── Profile (its own form with its own Save button) ── -->
    {@render section("Profile")}
    <div class={CARD}>
      <!-- Save sits beside the fields (lined up with the inputs), not on a row of its own. -->
      <form
        onsubmit={saveProfile}
        class="grid grid-cols-[1fr_1fr_auto] gap-x-3 gap-y-1.5 px-4 py-3.5"
      >
        <label class="text-[13px] text-(--text-muted)">
          Display name
          <input
            bind:value={displayName}
            maxlength="32"
            class="mt-1 h-8.5 w-full rounded-md border border-(--line-2) bg-(--bg) px-3 text-[13px] text-(--text-primary) outline-none focus:border-(--accent)"
          />
          {#if displayNameError && displayName !== me.displayName}<span
              class="mt-1 block text-(--danger)">{displayNameError}</span
            >{/if}
        </label>
        <label class="text-[13px] text-(--text-muted)">
          Username
          <!-- oninput lowercases as you type; bind:value keeps `username` in sync -->
          <input
            bind:value={username}
            oninput={() => (username = username.toLowerCase())}
            maxlength="16"
            disabled={!!usernameLockedUntil}
            class="mt-1 h-8.5 w-full rounded-md border border-(--line-2) bg-(--bg) px-3 text-[13px] text-(--text-primary) outline-none focus:border-(--accent) disabled:opacity-50"
          />
          {#if usernameLockedUntil}
            <span class="mt-1 block text-(--text-faint)"
              >You can change it again on {usernameLockedUntil.toLocaleDateString()}.</span
            >
          {:else if usernameError && username !== me.username}
            <span class="mt-1 block text-(--danger)">{usernameError}</span>
          {:else}
            <span class="mt-1 block text-(--text-faint)"
              >Can be changed once every 30 days.</span
            >
          {/if}
        </label>
        <Button
          type="submit"
          size="sm"
          variant="primary"
          class="mt-6 self-start"
          disabled={!canSave || saving}
          >{saving
            ? "Saving…"
            : profileMessage?.ok && !dirty
              ? "Saved"
              : "Save"}</Button
        >
        {#if profileMessage && !profileMessage.ok}
          <span class="col-span-3 text-[13px] text-(--danger)" role="alert"
            >{profileMessage.text}</span
          >
        {/if}
      </form>

      <!-- ── Picture: saves the moment you pick one ── -->
      <div class="px-4 py-3.5">
        <b class="mb-2 block font-medium">Picture</b>
        <div
          class="grid grid-cols-2 gap-2"
          role="radiogroup"
          aria-label="Picture"
        >
          {#each pictures as pic (pic.id)}
            {@const chosen = me.avatarSource === pic.id}
            <button
              type="button"
              role="radio"
              aria-checked={chosen}
              disabled={!pic.available || busy === "picture"}
              onclick={() =>
                act("picture", () =>
                  social.updateProfile({ avatarSource: pic.id }),
                )}
              class="flex items-center gap-3 rounded-md border px-3 py-2.5 text-left transition-colors disabled:cursor-default
            {chosen && pic.available
                ? 'border-(--accent) bg-(--accent)/8'
                : 'border-(--line-2) enabled:hover:bg-(--bg-hover)'}"
            >
              {#if pic.available}
                <Head user={pic.user} size={32} />
              {:else}
                <ProviderIcon provider={pic.id} class="size-8 opacity-40" />
              {/if}
              <span class="min-w-0">
                <b class="block font-medium {pic.available ? '' : 'opacity-60'}"
                  >{pic.label}</b
                >
                <small class="block text-[13px] text-(--text-faint)"
                  >{pic.available
                    ? chosen
                      ? "Showing now"
                      : "Use this one"
                    : pic.missing}</small
                >
              </span>
            </button>
          {/each}
        </div>
        {@render failed("picture")}
      </div>
    </div>

    <!-- ── Privacy: saves the moment you flip it ── -->
    {@render section("Privacy")}
    <div class={CARD}>
      <div class={ROW}>
        <div class="flex-1">
          <b class="block font-medium">Appear offline</b>
          <small class="text-[13px] text-(--text-faint)"
            >Friends see you as offline and can't see what you're playing.</small
          >
        </div>
        <Switch
          checked={me.appearOffline}
          label="Appear offline"
          disabled={busy === "offline"}
          onchange={(next) =>
            act("offline", () => social.updateProfile({ appearOffline: next }))}
        />
      </div>
      {@render failed("offline")}
      <div class={ROW}>
        <div class="flex-1">
          <b class="block font-medium">Your data</b>
          <small class="text-[13px] text-(--text-faint)"
            >{exported
              ? `Saved to ${exported}`
              : "A copy of everything Atlas stores about you, as a file."}</small
          >
        </div>
        <Button
          size="sm"
          disabled={busy !== null}
          onclick={() =>
            act("export", async () => {
              exported = await api.exportMyData();
            })}>{busy === "export" ? "Saving…" : "Download"}</Button
        >
      </div>
      {@render failed("export")}
    </div>

    {@render thisPc()}

    <!-- ── Sign-in methods ── -->
    {@render section("Sign in with")}
    <div class={CARD}>
      {#each PROVIDERS as p (p.id)}
        {@const linked = me.identities.find((i) => i.provider === p.id)}
        <div class="{ROW} items-center!">
          <ProviderIcon provider={p.id} />
          <div class="min-w-0 flex-1">
            <b class="block font-medium">{p.label}</b>
            <small class="text-[13px] text-(--text-faint)"
              >{linked ? linked.username : "Not linked"}</small
            >
          </div>
          {#if linked}
            <Button
              size="sm"
              variant="ghost"
              disabled={busy !== null || me.identities.length === 1}
              aria-label="Unlink {p.label}"
              title={me.identities.length === 1
                ? "You need at least one way to sign in"
                : undefined}
              onclick={async () => {
                const ok = await confirmAction({
                  title: `Unlink ${p.label}?`,
                  message: `You won't be able to sign in with ${p.label} anymore. You can link it again later.`,
                  confirmLabel: "Unlink",
                  danger: true,
                });
                if (ok) act(p.id, () => social.unlinkIdentity(p.id));
              }}>Unlink</Button
            >
          {:else}
            {#if busy === p.id}
              <Button size="sm" variant="ghost" onclick={cancelLink}
                >Cancel</Button
              >
            {/if}
            <Button
              size="sm"
              disabled={busy !== null && !linking}
              onclick={() => act(p.id, () => social.linkIdentity(p.id))}
              >{busy === p.id ? "Start again" : "Link"}</Button
            >
          {/if}
        </div>
        {@render failed(p.id)}
      {/each}
    </div>

    <!-- ── Friend code and storage ── -->
    {@render section("Account")}
    <div class={CARD}>
      <div class="{ROW} items-center!">
        <div class="min-w-0 flex-1">
          <b class="block font-medium">Friend code</b>
          <span class="font-mono font-semibold tracking-[2px] select-text"
            >{formatCode(me.friendCode)}</span
          >
          <small class="block text-[13px] text-(--text-faint)"
            >Reset it if your code was posted somewhere public.</small
          >
          {@render failed("code")}
        </div>
        <Button
          size="sm"
          variant="ghost"
          disabled={busy !== null}
          onclick={async () => {
            const ok = await confirmAction({
              title: "Reset your friend code?",
              message:
                "Your old code and link stop working right away. Friends you already have stay friends.",
              confirmLabel: "Reset code",
            });
            if (ok) act("code", () => social.resetFriendCode());
          }}>Reset code</Button
        >
      </div>
      <div class="px-4 py-3.5">
        <div class="mb-2 flex items-baseline justify-between gap-3">
          <b class="font-medium">Storage</b>
          <button
            type="button"
            onclick={() => openDialog({ kind: "storage" })}
            class="rounded px-1 text-[13px] text-(--accent) hover:underline"
            >See files</button
          >
        </div>
        <div class="h-1.5 overflow-hidden rounded-full bg-(--line)">
          <div
            class="h-full bg-(--accent)"
            style="width: {Math.min(
              100,
              (me.storage.usedBytes / me.storage.quotaBytes) * 100,
            )}%"
          ></div>
        </div>
        <small class="mt-1.5 block text-[13px] text-(--text-faint)">
          {fileSize(me.storage.usedBytes)} of {fileSize(me.storage.quotaBytes)} used
          by files you've uploaded
        </small>
      </div>
    </div>
  {/if}

  <p class="mt-6 text-center text-[13px] text-(--text-faint)">
    {#each [{ page: "terms", label: "Terms of Service" }, { page: "privacy", label: "Privacy Policy" }] as const as l, i (l.page)}
      {#if i}<span aria-hidden="true">{" · "}</span>{/if}
      <button
        type="button"
        class="hover:text-(--text-primary) hover:underline"
        onclick={() =>
          api
            .openLegalPage(l.page)
            .catch((e) => (error = { key: "legal", text: e.message }))}
        >{l.label}</button
      >
    {/each}
  </p>
  {@render failed("legal")}

  {#snippet footer()}
    {#if error?.key === "signout" || error?.key === "delete"}
      <span class="basis-full text-[13px] text-(--danger)" role="alert"
        >{error.text}</span
      >
    {/if}
    {#if me}
      <Button
        variant="ghost"
        class="mr-auto text-(--danger)!"
        disabled={busy !== null}
        onclick={deleteAccount}>Delete account</Button
      >
      <Button variant="ghost" disabled={busy !== null} onclick={signOut}
        >{busy === "signout" ? "Signing out…" : "Sign out"}</Button
      >
    {/if}
    <Button onclick={onclose}>Done</Button>
  {/snippet}
</Dialog>
