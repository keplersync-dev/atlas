import Home from "../tabs/Home.svelte";
import Instances from "../tabs/Instances.svelte";
import Shared from "../tabs/Shared.svelte";
import Friends from "../tabs/Friends.svelte";

export const tabs = [
  {
    id: "home",
    label: "Home",
    icon: "/home.svg",
    component: Home,
  },
  {
    id: "instances",
    label: "My instances",
    icon: "/grid.svg",
    component: Instances,
  },
  {
    id: "shared",
    label: "Shared",
    icon: "/inbox.svg",
    component: Shared,
  },
  {
    id: "friends",
    label: "Friends",
    icon: "/users.svg",
    component: Friends,
  },
] as const;

export type TabComponent = (typeof tabs)[number]["component"];
