// @faridguzman: Vue Router with two-layer guards — auth (JWT present?) then identity (keys set up?).
// Hash history is used so the Tauri webview and Vite dev server both handle routing consistently.
import { createRouter, createWebHashHistory } from "vue-router";
import { useIdentityStore } from "../stores/identity";
import { useAuthStore } from "../stores/auth";

const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: "/", redirect: "/chat" },
    {
      // @faridguzman: OAuth callback — server redirects here with ?token=JWT (dev mode).
      // In production the engage:// deep-link is used instead.
      path: "/auth",
      component: () => import("../views/AuthCallbackView.vue"),
    },
    {
      path: "/login",
      component: () => import("../views/LoginView.vue"),
      meta: { requiresNoAuth: true },
    },
    {
      path: "/setup",
      component: () => import("../views/SetupView.vue"),
      meta: { requiresAuth: true, requiresNoIdentity: true },
    },
    {
      path: "/chat",
      component: () => import("../views/ChatView.vue"),
      meta: { requiresAuth: true, requiresIdentity: true },
    },
    {
      path: "/chat/:contactId",
      component: () => import("../views/ChatView.vue"),
      meta: { requiresAuth: true, requiresIdentity: true },
    },
    {
      // @faridguzman: Group conversation thread
      path: "/group/:groupId",
      component: () => import("../views/GroupView.vue"),
      meta: { requiresAuth: true, requiresIdentity: true },
    },
    {
      path: "/settings",
      component: () => import("../views/SettingsView.vue"),
      meta: { requiresAuth: true, requiresIdentity: true },
    },
    {
      // @faridguzman: Invite acceptance — reachable via engage://invite?token=TOKEN deep link
      // or directly as /#/invite?token=TOKEN.  Requires auth so the recipient must sign in
      // before adding a contact (identity keys need to exist for the session to work).
      path: "/invite",
      component: () => import("../views/InviteView.vue"),
      meta: { requiresAuth: true, requiresIdentity: true },
    },
  ],
});

// @faridguzman: Guard order matters — check auth before identity so unauthenticated
// users never reach a route that would call invoke() and crash without Tauri context.
router.beforeEach((to) => {
  const auth = useAuthStore();
  const identity = useIdentityStore();

  if (to.meta.requiresAuth && !auth.isAuthenticated) return "/login";
  if (to.meta.requiresNoAuth && auth.isAuthenticated) return "/chat";
  if (to.meta.requiresIdentity && !identity.isSetup) return "/setup";
  if (to.meta.requiresNoIdentity && identity.isSetup) return "/chat";
});

export default router;
