<script lang="ts">
  import { untrack } from "svelte";
  import { confirmAction } from "$lib/confirm.svelte";
  import { fileSize, formatCode } from "$lib/format";
  import { social } from "$lib/stores";
  import type { AuthProvider } from "$lib/types";
  import Button from "../Button.svelte";
  import Dialog from "../Dialog.svelte";
  import Head from "../Head.svelte";
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
  let error = $state<string | null>(null);
  let busy = $state<string | null>(null);

  async function act(key: string, action: () => Promise<unknown>) {
    busy = key;
    error = null;
    try {
      await action();
    } catch (err) {
      error = (err as Error).message;
    } finally {
      busy = null;
    }
  }

  const PROVIDERS: {
    id: AuthProvider;
    label: string;
    mark: string;
    color: string;
  }[] = [
    { id: "minecraft", label: "Minecraft", mark: "M", color: "#9aa3ad" },
    { id: "discord", label: "Discord", mark: "D", color: "#5865f2" },
  ];

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

{#snippet section(title: string)}
  <h3 class="mt-5 mb-2 text-xs font-semibold text-(--text-muted) first:mt-1">
    {title}
  </h3>
{/snippet}

<Dialog
  title={me?.displayName ?? "Account"}
  description={me ? `@${me.username}` : undefined}
  wide
  {onclose}
>
  {#snippet header()}
    {#if me}<Head user={me} size={40} ring="#15181d" />{/if}
  {/snippet}

  {#if !me}
    <p class="py-4 text-[12.5px] text-(--text-muted)">You're signed out.</p>
  {:else}
    <!-- ── Profile (its own form with its own Save button) ── -->
    {@render section("Profile")}
    <!-- Save sits beside the fields (lined up with the inputs), not on a row of its own. -->
    <form
      onsubmit={saveProfile}
      class="grid grid-cols-[1fr_1fr_auto] gap-x-3 gap-y-1.5"
    >
      <label class="text-xs text-(--text-muted)">
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
      <label class="text-xs text-(--text-muted)">
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
        <span class="col-span-3 text-xs text-(--danger)" role="alert"
          >{profileMessage.text}</span
        >
      {/if}
    </form>

    <!-- ── Privacy: saves the moment you flip it ── -->
    {@render section("Privacy")}
    <div class="flex items-start gap-3">
      <div class="flex-1">
        <b class="block font-medium">Appear offline</b>
        <small class="text-[11.5px] text-(--text-faint)"
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

    <!-- ── Sign-in methods ── -->
    {@render section("Sign in with")}
    {#each PROVIDERS as p (p.id)}
      {@const linked = me.identities.find((i) => i.provider === p.id)}
      <div class="flex items-center gap-3 py-1.5">
        <span
          class="grid size-6 place-items-center rounded-md text-[11px] font-bold text-white"
          style="background: {p.color}">{p.mark}</span
        >
        <div class="min-w-0 flex-1">
          <b class="block font-medium">{p.label}</b>
          <small class="text-[11.5px] text-(--text-faint)"
            >{linked ? linked.username : "Not linked"}</small
          >
        </div>
        {#if linked}
          <Button
            size="sm"
            variant="ghost"
            disabled={busy !== null || me.identities.length === 1}
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
          <Button
            size="sm"
            disabled={busy !== null}
            onclick={() => act(p.id, () => social.linkIdentity(p.id))}
            >Link</Button
          >
        {/if}
      </div>
    {/each}

    <!-- ── Friend code ── -->
    {@render section("Friend code")}
    <div class="flex items-center gap-3">
      <span
        class="flex-1 font-mono text-base font-semibold tracking-[2px] select-text"
        >{formatCode(me.friendCode)}</span
      >
      <Button
        size="sm"
        variant="ghost"
        disabled={busy !== null}
        onclick={async () => {
          const ok = await confirmAction({
            title: "Reset your friend code?",
            message:
              "Your old code and link stop working right away. You'll get a new one to share.",
            confirmLabel: "Reset code",
          });
          if (ok) act("code", () => social.resetFriendCode());
        }}>Reset code</Button
      >
    </div>
    <small class="text-[11.5px] text-(--text-faint)"
      >Reset it if your code was posted somewhere public.</small
    >

    <!-- ── Storage ── -->
    {@render section("Storage")}
    <div class="h-1.5 overflow-hidden rounded-full bg-(--line-2)">
      <div
        class="h-full bg-(--accent)"
        style="width: {Math.min(
          100,
          (me.storage.usedBytes / me.storage.quotaBytes) * 100,
        )}%"
      ></div>
    </div>
    <small class="mt-1.5 block text-[11.5px] text-(--text-faint)">
      {fileSize(me.storage.usedBytes)} of {fileSize(me.storage.quotaBytes)} used by
      files you've uploaded
    </small>

    {#if error}<p class="mt-3 text-xs text-(--danger)" role="alert">
        {error}
      </p>{/if}
  {/if}

  {#snippet footer()}
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
