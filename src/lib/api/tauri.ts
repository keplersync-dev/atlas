// Real implementation: forwards every call to a Rust command of the same name in snake_case.
// Tauri converts camelCase argument keys to snake_case parameters automatically.
// Commands that don't exist yet will reject, so keep VITE_ATLAS_API=mock until they do.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { AtlasEvent } from "$lib/types";
import { ApiError, type AtlasApi } from "./api";

async function call<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (e) {
    // Rust commands return Err(AppError) serialised as { code, message } (see docs/backend-plan.md).
    const err = e as { code?: ApiError["code"]; message?: string };
    throw new ApiError(err.code ?? "unknown", err.message ?? String(e));
  }
}

export const tauriApi: AtlasApi = {
  getSession: () => call("get_session"),
  signIn: (provider) => call("sign_in", { provider }),
  signOut: () => call("sign_out"),
  linkIdentity: (provider) => call("link_identity", { provider }),
  unlinkIdentity: (provider) => call("unlink_identity", { provider }),
  updateProfile: (patch) => call("update_profile", { patch }),
  resetFriendCode: () => call("reset_friend_code"),
  deleteAccount: () => call("delete_account"),

  listLaunchers: () => call("list_launchers"),
  detectLaunchers: () => call("detect_launchers"),
  addLauncher: (kind, rootPath) => call("add_launcher", { kind, rootPath }),
  removeLauncher: (id) => call("remove_launcher", { id }),
  listDetectedInstances: () => call("list_detected_instances"),

  listInstances: () => call("list_instances"),
  getInstance: (id) => call("get_instance", { id }),
  trackInstances: (refs) => call("track_instances", { refs }),
  trackFolder: (gameDir) => call("track_folder", { gameDir }),
  untrackInstance: (id) => call("untrack_instance", { id }),
  updateInstance: (id, patch) => call("update_instance", { id, patch }),
  rescanInstance: (id) => call("rescan_instance", { id }),
  openInLauncher: (id) => call("open_in_launcher", { id }),
  openInstanceFolder: (id) => call("open_instance_folder", { id }),
  setContentEnabled: (instanceId, contentId, enabled) =>
    call("set_content_enabled", { instanceId, contentId, enabled }),
  restoreBackup: (instanceId, backupId) =>
    call("restore_backup", { instanceId, backupId }),

  listSharedInstances: () => call("list_shared_instances"),
  getSharedInstance: (id) => call("get_shared_instance", { id }),
  acceptInvite: (sharedInstanceId, launcherId) =>
    call("accept_invite", { sharedInstanceId, launcherId }),
  declineInvite: (sharedInstanceId) =>
    call("decline_invite", { sharedInstanceId }),
  preparePublish: (instanceId) => call("prepare_publish", { instanceId }),
  shareInstance: (instanceId, input) =>
    call("share_instance", { instanceId, input }),
  publishInstance: (instanceId, input) =>
    call("publish_instance", { instanceId, input }),
  previewSync: (instanceId) => call("preview_sync", { instanceId }),
  syncInstance: (instanceId, options) =>
    call("sync_instance", { instanceId, options: options ?? {} }),
  revertLocalChanges: (instanceId) =>
    call("revert_local_changes", { instanceId }),
  updateSharedInstance: (id, patch) =>
    call("update_shared_instance", { id, patch }),
  inviteToSharedInstance: (id, input) =>
    call("invite_to_shared_instance", { id, input }),
  setSharedMemberRole: (id, userId, role) =>
    call("set_shared_member_role", { id, userId, role }),
  removeSharedMember: (id, userId) =>
    call("remove_shared_member", { id, userId }),
  unshareFromGroup: (id, groupId) =>
    call("unshare_from_group", { id, groupId }),
  transferSharedInstance: (id, userId) =>
    call("transfer_shared_instance", { id, userId }),
  leaveSharedInstance: (id) => call("leave_shared_instance", { id }),
  archiveSharedInstance: (id) => call("archive_shared_instance", { id }),

  listFriends: () => call("list_friends"),
  listFriendRequests: () => call("list_friend_requests"),
  previewFriendCode: (code) => call("preview_friend_code", { code }),
  sendFriendRequest: (code) => call("send_friend_request", { code }),
  respondToFriendRequest: (userId, accept) =>
    call("respond_to_friend_request", { userId, accept }),
  cancelFriendRequest: (userId) => call("cancel_friend_request", { userId }),
  removeFriend: (userId) => call("remove_friend", { userId }),
  blockUser: (userId) => call("block_user", { userId }),

  listGroups: () => call("list_groups"),
  getGroup: (id) => call("get_group", { id }),
  createGroup: (input) => call("create_group", { input }),
  updateGroup: (id, patch) => call("update_group", { id, patch }),
  deleteGroup: (id) => call("delete_group", { id }),
  createGroupInvite: (groupId, input) =>
    call("create_group_invite", { groupId, input }),
  revokeGroupInvite: (groupId, code) =>
    call("revoke_group_invite", { groupId, code }),
  previewGroupInvite: (code) => call("preview_group_invite", { code }),
  joinGroup: (code) => call("join_group", { code }),
  setGroupMemberRole: (groupId, userId, role) =>
    call("set_group_member_role", { groupId, userId, role }),
  removeGroupMember: (groupId, userId) =>
    call("remove_group_member", { groupId, userId }),
  transferGroup: (groupId, userId) =>
    call("transfer_group", { groupId, userId }),

  listActivity: () => call("list_activity"),
  markActivityRead: (ids) => call("mark_activity_read", { ids }),

  subscribe(listener) {
    // Rust emits every AtlasEvent on one channel.
    const unlisten = listen<AtlasEvent>("atlas://event", (e) =>
      listener(e.payload),
    );
    return () => void unlisten.then((fn) => fn());
  },
};
