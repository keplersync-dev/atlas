<script lang="ts">
  import { untrack } from "svelte";
  import { api } from "$lib/api";
  import { confirmAction } from "$lib/confirm.svelte";
  import { openDialog } from "$lib/dialogs.svelte";
  import { copier } from "$lib/clipboard.svelte";
  import { fileSize } from "$lib/format";
  import { isMac } from "$lib/platform";
  import { resource } from "$lib/resource.svelte";
  import { social, updates } from "$lib/stores";
  import type { AppSettings } from "$lib/types";
  import Button from "../Button.svelte";
  import Dialog from "../Dialog.svelte";
  import Head from "../Head.svelte";
  import PersonName from "../PersonName.svelte";
  import SignInButtons from "../SignInButtons.svelte";
  import Switch from "../Switch.svelte";

  let { onclose }: { onclose: () => void } = $props();

  const me = $derived(social.session?.user ?? null);

  // ── Profile: a draft form ──
  // Copied again only when the account changes (signing in here): a store reload
  // (new session object) can't wipe half-typed edits.
  const myId = $derived(me?.id);
  let displayName = $derived(myId ? untrack(() => me!.displayName) : "");
  const displayNameError = $derived(
    displayName.trim().length === 0
      ? "Required"
      : displayName.trim().length > 32
        ? "32 characters at most"
        : null,
  );
  // "Dirty" = differs from what's saved. Save is only enabled when there's something to save.
  const dirty = $derived(!!me && displayName.trim() !== me.displayName);
  const canSave = $derived(dirty && !displayNameError);

  let saving = $state(false);
  let profileMessage = $state<{ text: string; ok: boolean } | null>(null);

  async function saveProfile(e: SubmitEvent) {
    e.preventDefault();
    if (!canSave || !me) return;
    saving = true;
    profileMessage = null;
    try {
      await social.updateProfile({ displayName: displayName.trim() });
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
  const blocked = resource(() =>
    myId ? api.listBlocked() : Promise.resolve(null),
  );

  const version = resource(() => api.appVersion());
  let upToDate = $state(false);

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
      hint: isMac
        ? "A quiet sound with each one. Silent while a Focus is on."
        : "A quiet sound with each one. Silent while Windows is set to Do not disturb.",
    },
    {
      key: "closeToTray",
      group: "Startup",
      title: `Keep running in the ${isMac ? "menu bar" : "tray"}`,
      hint: `Closing the window leaves Atlas in the ${isMac ? "menu bar" : "tray"}, so it keeps your packs up to date.`,
    },
    {
      key: "startWithWindows",
      group: "Startup",
      title: isMac ? "Open at login" : "Start with Windows",
      hint: isMac
        ? "Atlas starts in the menu bar when you log in."
        : "Atlas starts in the tray when you sign in to Windows.",
    },
  ];

  // A section's card, and one setting in it: words on the left, the control on the right.
  const CARD =
    "divide-y divide-(--line) overflow-hidden rounded-lg border border-(--line) bg-(--bg)";
  const ROW = "flex items-center gap-4 px-4 py-3.5";

  async function act(key: string, action: () => Promise<unknown>) {
    const mine = ++attempt;
    busy = key;
    error = null;
    try {
      await action();
    } catch (err) {
      if (mine === attempt) error = { key, text: (err as Error).message };
    } finally {
      if (mine === attempt) busy = null;
    }
  }

  const clip = copier();

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
        {#if group === "Updates"}
          <div class={ROW}>
            <div class="min-w-0 flex-1">
              <b class="block font-medium">Atlas {version.data ?? ""}</b>
              <small class="block text-[13px] text-(--text-faint)">
                {updates.available
                  ? `Atlas ${updates.available.version} is ready. Atlas closes for a few seconds while it updates.`
                  : upToDate
                    ? "You have the latest version."
                    : "Atlas looks for a new version every few hours."}
              </small>
              {@render failed("update")}
              {#if updates.error}<small
                  class="block text-[13px] text-(--danger)"
                  role="alert">{updates.error}</small
                >{/if}
            </div>
            {#if updates.available}
              <Button
                size="sm"
                variant="primary"
                disabled={updates.installing}
                onclick={() => updates.install()}
                >{updates.installing
                  ? "Downloading…"
                  : "Restart to update"}</Button
              >
            {:else}
              <Button
                size="sm"
                variant="ghost"
                disabled={busy === "update"}
                onclick={() =>
                  act("update", async () => {
                    upToDate = false;
                    await updates.checkNow();
                    upToDate = !updates.available;
                  })}
                >{busy === "update" ? "Checking…" : "Check for updates"}</Button
              >
            {/if}
          </div>
        {/if}
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
          <span class="mt-1 block text-(--text-faint)"
            >What friends see. Change it any time.</span
          >
        </label>
        <div class="text-[13px] text-(--text-muted)">
          Minecraft name
          <div
            class="mt-1 flex h-8.5 items-center rounded-md border border-dashed border-(--line-2) px-3 text-[13px] text-(--text-muted) select-text"
          >
            @{me.username}
          </div>
          <span class="mt-1 block text-(--text-faint)"
            >Friends add you with this. Rename in Minecraft to change it.</span
          >
        </div>
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
      <!-- Blocking is undone here, so a block from any ⋯ menu is never final. -->
      <div class={ROW}>
        <div class="min-w-0 flex-1">
          <b class="block font-medium">Blocked</b>
          {#if blocked.data?.length}
            <div class="mt-2 grid gap-1.5">
              {#each blocked.data as u (u.id)}
                <div class="flex items-center gap-2.5">
                  <Head user={u} size={24} ring="var(--bg)" />
                  <span class="min-w-0 flex-1"><PersonName user={u} /></span>
                  <Button
                    size="sm"
                    disabled={busy !== null}
                    aria-label="Unblock {u.displayName} (@{u.username})"
                    onclick={() =>
                      act("unblock", async () => {
                        await api.unblockUser(u.id);
                        blocked.reload();
                      })}>Unblock</Button
                  >
                </div>
              {/each}
            </div>
          {:else}
            <small class="text-[13px] text-(--text-faint)"
              >{blocked.loading
                ? "Loading…"
                : blocked.error
                  ? `Couldn't load who you blocked: ${blocked.error}`
                  : "Nobody. You can block someone from their ⋯ menu."}</small
            >
          {/if}
          {@render failed("unblock")}
        </div>
      </div>
    </div>

    {@render thisPc()}

    <!-- ── Friend link and storage ── -->
    {@render section("Account")}
    <div class={CARD}>
      <div class="{ROW} items-center!">
        <div class="min-w-0 flex-1">
          <b class="block font-medium">Friend link</b>
          <small class="block text-[13px] text-(--text-faint)"
            >{clip.failed === "link"
              ? `Couldn't copy. Your link: ${me.friendLink}`
              : "Anyone with it can send you a friend request. Reset it if it was posted somewhere public."}</small
          >
          {@render failed("code")}
        </div>
        <Button
          size="sm"
          icon={clip.copied === "link" ? "check" : undefined}
          onclick={() => clip.copy(me.friendLink, "link")}>Copy link</Button
        >
        <Button
          size="sm"
          variant="ghost"
          disabled={busy !== null}
          onclick={async () => {
            const ok = await confirmAction({
              title: "Reset your friend link?",
              message:
                "Your old link stops working right away. Friends you already have stay friends.",
              confirmLabel: "Reset link",
            });
            if (ok) act("code", () => social.resetFriendCode());
          }}>Reset link</Button
        >
      </div>
      <div class="px-4 py-3.5">
        <div class="mb-2 flex items-baseline justify-between gap-3">
          <b class="font-medium">Storage</b>
          <button
            type="button"
            onclick={() => openDialog({ kind: "storage" })}
            class="tap rounded px-1 text-[13px] text-(--accent) hover:underline"
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
        class="tap hover:text-(--text-primary) hover:underline"
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
