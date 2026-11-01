import { browser } from "@wdio/globals";
import { SIZES, atSize, capture, useProfile, writeReport } from "../helpers";

// Walks every page and dialog of each signed-in window at three sizes: a screenshot of
// each in e2e/out/<profile>/, and layout checks in e2e/out/report.json. Read-only: it
// opens and closes things but never confirms, sends or removes anything.
const PROFILES = (process.env.ATLAS_PROFILES ?? "haloxx,dakep").split(",");

type Step = {
  name: string;
  open: () => Promise<unknown>;
  close?: () => Promise<unknown>;
};

const go = (view: object) =>
  browser.execute((v) => (window as any).__atlas.go(v), view);
const dialog = (d: object) =>
  browser.execute((v) => (window as any).__atlas.openDialog(v), d);
const closeDialog = () =>
  browser.execute(() => (window as any).__atlas.closeDialog());
const clickText = async (selector: string, text: string) => {
  for (const el of await browser.$$(selector).getElements())
    if ((await el.getText()).trim().startsWith(text)) return el.click();
  throw new Error(`no ${selector} starting "${text}"`);
};
const escape = () => browser.keys("Escape");

async function stepsFor(): Promise<Step[]> {
  type Shared = { id: string; name: string; myRole: string; myStatus: string };
  type Inst = { id: string; name: string; status: string; role: string | null };
  const data: {
    instances: Inst[];
    shared: Shared[];
    friend: { id: string; displayName: string } | null;
  } = await browser.execute(() => {
    const a = (window as any).__atlas;
    return {
      instances: a.instances.list.map((i: any) => ({
        id: i.id,
        name: i.name,
        status: i.status,
        role: i.sync?.role ?? null,
      })),
      shared: a.social.shared.map((s: any) => ({
        id: s.id,
        name: s.name,
        myRole: s.myRole,
        myStatus: s.myStatus,
      })),
      friend: a.social.friends[0]?.user ?? null,
    };
  });
  const steps: Step[] = [
    { name: "home", open: () => go({ page: "home" }) },
    { name: "shared", open: () => go({ page: "shared" }) },
    {
      name: "shared-share-menu",
      open: async () => {
        await go({ page: "shared" });
        await clickText("main button", "Share an instance");
      },
      close: escape,
    },
    { name: "friends", open: () => go({ page: "friends" }) },
    {
      name: "sidebar-account-menu",
      open: () => browser.$('[aria-label="Account and settings"]').click(),
      close: escape,
    },
    {
      name: "dialog-settings",
      open: () => dialog({ kind: "account" }),
      close: closeDialog,
    },
    {
      name: "dialog-uploads",
      open: () => dialog({ kind: "storage" }),
      close: closeDialog,
    },
    {
      name: "dialog-track",
      open: () => dialog({ kind: "track" }),
      close: closeDialog,
    },
    {
      name: "dialog-start-with-windows",
      open: () => dialog({ kind: "startWithWindows", owner: "Haloxx" }),
      close: closeDialog,
    },
    {
      name: "dialog-link-friend",
      open: () =>
        dialog({ kind: "link", link: { kind: "friend", code: "B3WS82RA" } }),
      close: closeDialog,
    },
  ];
  const friend = data.friend;
  if (friend)
    steps.push({
      name: "dialog-report-user",
      open: () =>
        dialog({
          kind: "report",
          about: {
            kind: "user",
            id: friend.id,
            name: friend.displayName,
          },
        }),
      close: closeDialog,
    });
  const accept = data.shared.find(
    (s) => s.myRole !== "owner" && s.myStatus === "invited",
  );
  if (accept)
    steps.push({
      name: "dialog-accept",
      open: () => dialog({ kind: "accept", sharedInstanceId: accept.id }),
      close: closeDialog,
    });
  for (const [n, i] of data.instances.entries()) {
    const slug = `instance${n + 1}`;
    steps.push({
      name: `${slug}-content`,
      open: () => go({ page: "instance", id: i.id }),
    });
    for (const tab of ["Sharing", "History"])
      steps.push({
        name: `${slug}-${tab.toLowerCase()}`,
        open: async () => {
          await go({ page: "instance", id: i.id });
          await browser.pause(300);
          await clickText("[role=tab]", tab).catch(() => undefined);
        },
      });
    steps.push({
      name: `${slug}-menu`,
      open: async () => {
        await go({ page: "instance", id: i.id });
        await browser.pause(300);
        await browser.$('main [aria-label="More actions"]').click();
      },
      close: escape,
    });
    if (i.status === "ready" && !i.role)
      steps.push({
        name: `${slug}-dialog-share`,
        open: () => dialog({ kind: "share", instanceId: i.id }),
        close: closeDialog,
      });
    if (i.role === "owner")
      steps.push(
        {
          name: `${slug}-dialog-invite`,
          open: () => dialog({ kind: "invite", instanceId: i.id }),
          close: closeDialog,
        },
        {
          name: `${slug}-dialog-publish`,
          open: () => dialog({ kind: "publish", instanceId: i.id }),
          close: closeDialog,
        },
      );
  }
  return steps;
}

describe("walk every screen", () => {
  after(async () => {
    for (const [where, findings] of Object.entries(writeReport()))
      console.log(
        `\n${where}\n${findings.map((f) => `  ${f.kind}: ${f.detail}`).join("\n")}`,
      );
  });

  for (const profile of PROFILES)
    it(`as ${profile}`, async () => {
      await useProfile(profile);
      const names: string[] = [];
      try {
        for (const step of await stepsFor()) {
          for (const size of SIZES) {
            await atSize(size);
            await step.open();
            await browser.pause(700);
            await capture(profile, step.name, size);
            await step.close?.();
            await browser.pause(150);
          }
          names.push(step.name);
        }
      } finally {
        await closeDialog();
        await atSize(null);
        await go({ page: "home" });
        console.log(`${profile}: ${names.length} screens`);
      }
    });
});
