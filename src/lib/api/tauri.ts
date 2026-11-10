// Forwards every call to a Rust command of the same name in snake_case.
// Tauri converts camelCase argument keys to snake_case parameters automatically.

import { getVersion } from "@tauri-apps/api/app";
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
    // Rust commands return Err(AppError) serialised as { code, message }. Anything
    // else is Tauri's own wording, which is for us, not the player.
    const err = e as { code?: ApiError["code"]; message?: string };
    if (err?.message === undefined) {
      console.error(`${command} failed`, e);
      throw new ApiError(
        "unknown",
        "Something went wrong in Atlas. Try again.",
      );
    }
    throw new ApiError(err.code ?? "unknown", err.message);
  }
}

export const tauriApi: AtlasApi = {
  getSession: () => call("get_session"),
  signIn: () => call("sign_in"),
  cancelSignIn: () => call("cancel_sign_in"),
  signOut: () => call("sign_out"),
  updateProfile: (patch) => call("update_profile", { patch }),
  resetFriendCode: () => call("reset_friend_code"),
  deleteAccount: () => call("delete_account"),

  listLaunchers: () => call("list_launchers"),
  detectLaunchers: () => call("detect_launchers"),
  addLauncher: (kind, rootPath) => call("add_launcher", { kind, rootPath }),
  listDetectedInstances: () => call("list_detected_instances"),

  listInstances: () => call("list_instances"),
  getInstance: (id) => call("get_instance", { id }),
  trackInstances: (refs) => call("track_instances", { refs }),
  trackFolder: (gameDir) => call("track_folder", { gameDir }),
  untrackInstance: (id) => call("untrack_instance", { id }),
  updateInstance: (id, patch) => call("update_instance", { id, patch }),
  rescanInstance: (id) => call("rescan_instance", { id }),
  openInLauncher: (id) => call("open_in_launcher", { id }),
  playInstance: (id) => call("play_instance", { id }),
  openInstanceFolder: (id) => call("open_instance_folder", { id }),
  setContentEnabled: (instanceId, contentId, enabled) =>
    call("set_content_enabled", { instanceId, contentId, enabled }),
  restoreBackup: (instanceId, backupId) =>
    call("restore_backup", { instanceId, backupId }),
  cancelJob: (instanceId) => call("cancel_job", { instanceId }),
  openManualDownload: (instanceId, pageUrl) =>
    call("open_manual_download", { instanceId, pageUrl }),
  exportMrpack: (instanceId) => call("export_mrpack", { instanceId }),
  importMrpack: (launcherId) => call("import_mrpack", { launcherId }),
  setShareMode: (instanceId, contentId, mode) =>
    call("set_share_mode", { instanceId, contentId, mode }),
  setOptionalInstalled: (instanceId, contentId, installed) =>
    call("set_optional_installed", { instanceId, contentId, installed }),

  listSharedInstances: () => call("list_shared_instances"),
  getSharedInstance: (id) => call("get_shared_instance", { id }),
  acceptInvite: (sharedInstanceId, launcherId, installOptional, autoUpdate) =>
    call("accept_invite", {
      sharedInstanceId,
      launcherId,
      installOptional,
      autoUpdate,
    }),
  acceptInviteInto: (
    sharedInstanceId,
    instanceId,
    installOptional,
    autoUpdate,
  ) =>
    call("accept_invite_into", {
      sharedInstanceId,
      instanceId,
      installOptional,
      autoUpdate,
    }),
  declineInvite: (sharedInstanceId) =>
    call("decline_invite", { sharedInstanceId }),
  preparePublish: (instanceId) => call("prepare_publish", { instanceId }),
  setFileRules: (instanceId, rules) =>
    call("set_file_rules", { instanceId, rules }),
  shareInstance: (instanceId, input) =>
    call("share_instance", { instanceId, input }),
  publishInstance: (instanceId, input) =>
    call("publish_instance", { instanceId, input }),
  previewSync: (instanceId) => call("preview_sync", { instanceId }),
  syncInstance: (instanceId, options) =>
    call("sync_instance", { instanceId, options: options ?? {} }),
  revertLocalChanges: (instanceId) =>
    call("revert_local_changes", { instanceId }),
  inviteToSharedInstance: (id, input) =>
    call("invite_to_shared_instance", { id, input }),
  setSharedMemberRole: (id, userId, role) =>
    call("set_shared_member_role", { id, userId, role }),
  removeSharedMember: (id, userId) =>
    call("remove_shared_member", { id, userId }),
  leaveSharedInstance: (id) => call("leave_shared_instance", { id }),
  archiveSharedInstance: (id) => call("archive_shared_instance", { id }),

  listFriends: () => call("list_friends"),
  listFriendRequests: () => call("list_friend_requests"),
  previewFriend: (who) => call("preview_friend", { who }),
  sendFriendRequest: (who) => call("send_friend_request", { who }),
  sendFriendRequestTo: (userId) => call("send_friend_request_to", { userId }),
  respondToFriendRequest: (userId, accept) =>
    call("respond_to_friend_request", { userId, accept }),
  cancelFriendRequest: (userId) => call("cancel_friend_request", { userId }),
  removeFriend: (userId) => call("remove_friend", { userId }),
  blockUser: (userId) => call("block_user", { userId }),
  unblockUser: (userId) => call("unblock_user", { userId }),
  listBlocked: () => call("list_blocked"),
  takeOpenedLink: () => call("take_opened_link"),

  report: (input) => call("report", { input }),
  reportFile: (instanceId, contentId, reason, details) =>
    call("report_file", { instanceId, contentId, reason, details }),
  openLegalPage: (page) => call("open_legal_page", { page }),
  exportMyData: () => call("export_my_data"),
  listMyUploads: () => call("list_my_uploads"),
  removeMyUpload: (sha256) => call("remove_my_upload", { sha256 }),
  removeUnusedUploads: () => call("remove_unused_uploads"),

  getAppSettings: () => call("get_app_settings"),
  updateAppSettings: (patch) => call("update_app_settings", { patch }),
  windowReady: () => call("window_ready"),
  appVersion: () => getVersion(),
  checkForUpdate: () => call("check_for_update"),
  installUpdate: () => call("install_update"),
  openReleasePage: () => call("open_release_page"),

  listGroups: () => call("list_groups"),
  getGroup: (id) => call("get_group", { id }),
  createGroup: (input) => call("create_group", { input }),
  deleteGroup: (id) => call("delete_group", { id }),
  createGroupInvite: (groupId, input) =>
    call("create_group_invite", { groupId, input }),
  previewGroupInvite: (code) => call("preview_group_invite", { code }),
  joinGroup: (code) => call("join_group", { code }),
  inviteFriendsToGroup: (groupId, friendIds) =>
    call("invite_friends_to_group", { groupId, friendIds }),
  listGroupInvites: () => call("list_group_invites"),
  declineGroupInvite: (code) => call("decline_group_invite", { code }),
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
