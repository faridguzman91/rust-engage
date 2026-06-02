import { createRouter, createWebHashHistory } from "vue-router";
import { useIdentityStore } from "../stores/identity";
import { useAuthStore } from "../stores/auth";

const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: "/", redirect: "/chat" },
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
      path: "/settings",
      component: () => import("../views/SettingsView.vue"),
      meta: { requiresAuth: true, requiresIdentity: true },
    },
  ],
});

router.beforeEach((to) => {
  const auth = useAuthStore();
  const identity = useIdentityStore();

  if (to.meta.requiresAuth && !auth.isAuthenticated) return "/login";
  if (to.meta.requiresNoAuth && auth.isAuthenticated) return "/chat";
  if (to.meta.requiresIdentity && !identity.isSetup) return "/setup";
  if (to.meta.requiresNoIdentity && identity.isSetup) return "/chat";
});

export default router;
