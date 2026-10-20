import { api } from "$lib/api";
import type {
  ActivityItem,
  AuthProvider,
  Friend,
  FriendRequest,
  Group,
  Id,
  Me,
  Session,
  SharedInstance,
  UpdateProfileInput,
} from "$lib/types";

/** Session, friends, requests, groups and shared instances — everything that needs an Atlas account. */
class SocialStore {
  session = $state<Session | null>(null);
  friends = $state<Friend[]>([]);
  requests = $state<FriendRequest[]>([]);
  shared = $state<SharedInstance[]>([]);
  groups = $state<Group[]>([]);
  /** Newest first. */
  activity = $state<ActivityItem[]>([]);
  loaded = $state(false);

  signedIn = $derived(!!this.session?.user);
  invites = $derived(this.shared.filter((s) => s.myStatus === "invited"));
  incomingRequests = $derived(
    this.requests.filter((r) => r.direction === "incoming"),
  );
  outgoingRequests = $derived(
    this.requests.filter((r) => r.direction === "outgoing"),
  );

  async load() {
    this.session = await api.getSession();
    if (!this.session.user) {
      this.friends = [];
      this.requests = [];
      this.shared = [];
      this.groups = [];
      this.activity = [];
    } else {
      try {
        // Start all the requests at once instead of one after another.
        [this.friends, this.requests, this.shared, this.groups, this.activity] =
          await Promise.all([
            api.listFriends(),
            api.listFriendRequests(),
            api.listSharedInstances(),
            api.listGroups(),
            api.listActivity(),
          ]);
      } catch {
        // Offline or a server error: keep what we had so the UI stays usable.
      }
    }
    this.loaded = true;
  }

  async signIn(provider: AuthProvider) {
    await api.signIn(provider);
    await this.load();
  }

  // ── Account ──
  // Account calls return the updated `Me`; put it straight into the session
  // so everything showing your name or code updates without a reload.

  private setMe(me: Me) {
    if (this.session) this.session.user = me;
  }

  async updateProfile(patch: UpdateProfileInput) {
    this.setMe(await api.updateProfile(patch));
  }

  async linkIdentity(provider: AuthProvider) {
    this.setMe(await api.linkIdentity(provider));
  }

  async unlinkIdentity(provider: AuthProvider) {
    this.setMe(await api.unlinkIdentity(provider));
  }

  async resetFriendCode() {
    this.setMe(await api.resetFriendCode());
  }

  async signOut() {
    await api.signOut();
    await this.load();
  }

  async deleteAccount() {
    await api.deleteAccount();
    await this.load();
  }

  /**
   * Optimistic helper: apply a local change right away, then call the server.
   * If the server refuses, reload so the UI shows the truth again.
   */
  private async optimistic(local: () => void, remote: () => Promise<unknown>) {
    local();
    try {
      await remote();
    } catch (e) {
      await this.load();
      throw e;
    }
  }

  // ── Friends ──

  async sendFriendRequest(code: string) {
    const request = await api.sendFriendRequest(code);
    // Using the code of someone who already asked you makes you friends straight away.
    await this.load();
    return request;
  }

  respondToRequest(userId: Id, accept: boolean) {
    return this.optimistic(
      () => (this.requests = this.requests.filter((r) => r.user.id !== userId)),
      async () => {
        await api.respondToFriendRequest(userId, accept);
        if (accept) this.friends = await api.listFriends();
      },
    );
  }

  cancelRequest(userId: Id) {
    return this.optimistic(
      () => (this.requests = this.requests.filter((r) => r.user.id !== userId)),
      () => api.cancelFriendRequest(userId),
    );
  }

  removeFriend(userId: Id, block = false) {
    return this.optimistic(
      () => (this.friends = this.friends.filter((f) => f.user.id !== userId)),
      () => (block ? api.blockUser(userId) : api.removeFriend(userId)),
    );
  }

  // ── Invites ──

  declineInvite(sharedInstanceId: Id) {
    return this.optimistic(
      () =>
        (this.shared = this.shared.filter((s) => s.id !== sharedInstanceId)),
      () => api.declineInvite(sharedInstanceId),
    );
  }

  // ── Groups ──

  async createGroup(name: string) {
    const group = await api.createGroup({ name });
    this.groups = [...this.groups, group];
    return group;
  }

  async joinGroup(code: string) {
    const group = await api.joinGroup(code);
    // Joining can bring new shared instances with it (the group's invites).
    await this.load();
    return group;
  }

  async leaveGroup(groupId: Id) {
    const myId = this.session?.user?.id;
    if (!myId) return;
    await this.optimistic(
      () => (this.groups = this.groups.filter((g) => g.id !== groupId)),
      () => api.removeGroupMember(groupId, myId),
    );
  }
}

export const social = new SocialStore();
