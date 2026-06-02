import { createRouter, createWebHashHistory } from "vue-router";
import { useIdentityStore } from "../stores/identity";

const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    {
      path: "/",
      redirect: "/chat",
    },
    {
      path: "/setup",
      component: () => import("../views/SetupView.vue"),
      meta: { requiresNoIdentity: true },
    },
    {
      path: "/chat",
      component: () => import("../views/ChatView.vue"),
      meta: { requiresIdentity: true },
    },
    {
      path: "/chat/:contactId",
      component: () => import("../views/ChatView.vue"),
      meta: { requiresIdentity: true },
    },
    {
      path: "/settings",
      component: () => import("../views/SettingsView.vue"),
      meta: { requiresIdentity: true },
    },
  ],
});

router.beforeEach((to) => {
  const identity = useIdentityStore();
  if (to.meta.requiresIdentity && !identity.isSetup) {
    return "/setup";
  }
  if (to.meta.requiresNoIdentity && identity.isSetup) {
    return "/chat";
  }
});

export default router;
