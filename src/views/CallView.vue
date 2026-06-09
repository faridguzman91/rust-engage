<script setup lang="ts">
// @faridguzman: Call UI — two modes:
//   Ringing  — incoming call overlay with Accept / Decline buttons
//   Active   — full call view with local + remote video, mute, camera, hang-up
//
// Rendered as a fixed overlay in App.vue so it appears above all routes.
import { computed, ref, watch, onUnmounted } from "vue";
import { useWebRTC } from "../composables/useWebRTC";
import { useContactsStore } from "../stores/contacts";
import Avatar from "primevue/avatar";
import Button from "primevue/button";

const rtc = useWebRTC();
const contacts = useContactsStore();

// @faridguzman: DOM refs for the <video> elements.
// localVideo is small (picture-in-picture), remoteVideo fills the view.
const localVideo  = ref<HTMLVideoElement | null>(null);
const remoteVideo = ref<HTMLVideoElement | null>(null);

// @faridguzman: Wire MediaStream objects to <video> srcObject as they arrive.
watch(
  () => rtc.localStream.value,
  (stream) => { if (localVideo.value) localVideo.value.srcObject = stream; }
);
watch(
  () => rtc.remoteStream.value,
  (stream) => { if (remoteVideo.value) remoteVideo.value.srcObject = stream; }
);

// @faridguzman: Call duration timer — starts when call becomes active.
const elapsed = ref(0);
let timer: ReturnType<typeof setInterval> | null = null;

watch(
  () => rtc.isInCall.value,
  (active) => {
    if (active) {
      elapsed.value = 0;
      timer = setInterval(() => { elapsed.value++ }, 1000);
    } else {
      if (timer) { clearInterval(timer); timer = null; }
    }
  }
);

onUnmounted(() => { if (timer) clearInterval(timer); });

function formatElapsed(secs: number): string {
  const m = Math.floor(secs / 60).toString().padStart(2, "0");
  const s = (secs % 60).toString().padStart(2, "0");
  return `${m}:${s}`;
}

const callerName = computed(() => {
  const id = rtc.incoming.value?.fromUserId ?? rtc.peerId.value;
  return contacts.getById(id)?.displayName ?? id;
});

const callerLabel = computed(() => callerName.value[0]?.toUpperCase() ?? "?");

// @faridguzman: Show the overlay only when a call is in progress or ringing.
const visible = computed(() =>
  rtc.status.value === "ringing" ||
  rtc.status.value === "calling" ||
  rtc.status.value === "active" ||
  rtc.status.value === "ended"
);
</script>

<template>
  <Teleport to="body">
    <div v-if="visible" class="call-overlay">

      <!-- ── Ringing — incoming call ──────────────────────────────────────── -->
      <div v-if="rtc.isRinging.value" class="incoming-card">
        <div class="incoming-pulse">
          <Avatar
            :label="callerLabel"
            shape="circle"
            size="xlarge"
            style="background: var(--engage-accent); color:#fff; font-size:1.8rem; font-weight:700;"
          />
        </div>
        <p class="incoming-name">{{ callerName }}</p>
        <p class="incoming-type">
          <i :class="rtc.incoming.value?.isVideo ? 'pi pi-video' : 'pi pi-phone'" />
          {{ rtc.incoming.value?.isVideo ? "Video call" : "Voice call" }}
        </p>
        <div class="incoming-actions">
          <Button
            icon="pi pi-times"
            rounded
            severity="danger"
            size="large"
            v-tooltip="'Decline'"
            @click="rtc.declineCall()"
          />
          <Button
            :icon="rtc.incoming.value?.isVideo ? 'pi pi-video' : 'pi pi-phone'"
            rounded
            size="large"
            v-tooltip="'Accept'"
            style="background:var(--engage-accent);border-color:var(--engage-accent);"
            @click="rtc.acceptCall()"
          />
        </div>
      </div>

      <!-- ── Calling — waiting for answer ────────────────────────────────── -->
      <div v-else-if="rtc.isCalling.value" class="active-call">
        <div class="call-header">
          <Avatar
            :label="callerLabel"
            shape="circle"
            size="large"
            style="background:var(--engage-accent);color:#fff;font-weight:700;"
          />
          <div>
            <p class="call-name">{{ callerName }}</p>
            <p class="call-status">Calling…</p>
          </div>
        </div>
        <div class="call-controls">
          <Button icon="pi pi-phone-slash" rounded severity="danger" size="large"
            v-tooltip="'Cancel'" @click="rtc.endCall()" />
        </div>
      </div>

      <!-- ── Active call ───────────────────────────────────────────────────── -->
      <div v-else-if="rtc.isInCall.value" class="active-call">
        <!-- Remote video fills the background; local video is picture-in-picture -->
        <video
          v-if="rtc.isVideo.value"
          ref="remoteVideo"
          class="remote-video"
          autoplay
          playsinline
        />
        <div v-else class="audio-only">
          <Avatar
            :label="callerLabel"
            shape="circle"
            size="xlarge"
            style="background:var(--engage-accent);color:#fff;font-size:2rem;font-weight:700;"
          />
          <p class="call-name">{{ callerName }}</p>
        </div>

        <video
          v-if="rtc.isVideo.value"
          ref="localVideo"
          class="local-video"
          autoplay
          playsinline
          muted
        />

        <!-- Call header bar -->
        <div class="call-header-bar">
          <span class="call-timer">{{ formatElapsed(elapsed) }}</span>
          <span v-if="rtc.isVideo.value" class="call-type-badge">
            <i class="pi pi-video" style="font-size:0.8rem;" /> Video
          </span>
        </div>

        <!-- Controls -->
        <div class="call-controls">
          <Button
            :icon="rtc.isMuted.value ? 'pi pi-microphone-slash' : 'pi pi-microphone'"
            rounded
            :severity="rtc.isMuted.value ? 'warning' : 'secondary'"
            size="large"
            v-tooltip="rtc.isMuted.value ? 'Unmute' : 'Mute'"
            @click="rtc.toggleMute()"
          />
          <Button
            v-if="rtc.isVideo.value"
            :icon="rtc.isCamOff.value ? 'pi pi-eye-slash' : 'pi pi-eye'"
            rounded
            :severity="rtc.isCamOff.value ? 'warning' : 'secondary'"
            size="large"
            v-tooltip="rtc.isCamOff.value ? 'Camera on' : 'Camera off'"
            @click="rtc.toggleCamera()"
          />
          <Button
            icon="pi pi-phone"
            rounded
            severity="danger"
            size="large"
            v-tooltip="'Hang up'"
            @click="rtc.endCall()"
          />
        </div>
      </div>

      <!-- ── Call ended ────────────────────────────────────────────────────── -->
      <div v-else-if="rtc.status.value === 'ended'" class="ended-card">
        <i class="pi pi-phone" style="font-size:2rem; opacity:0.4;" />
        <p>Call ended</p>
      </div>

    </div>
  </Teleport>
</template>

<style scoped>
.call-overlay {
  position: fixed;
  inset: 0;
  z-index: 9999;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(10, 10, 18, 0.88);
  backdrop-filter: blur(6px);
}

/* ── Incoming call ──────────────────────────────────────────────────────────── */
.incoming-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 1rem;
  background: var(--engage-sidebar-bg);
  border: 1px solid var(--engage-border);
  border-radius: 20px;
  padding: 2.5rem 2rem;
  min-width: 280px;
}

/* @faridguzman: Pulsing ring animation for incoming call */
.incoming-pulse {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
}
.incoming-pulse::before,
.incoming-pulse::after {
  content: "";
  position: absolute;
  border-radius: 50%;
  background: rgba(62, 191, 140, 0.25);
  animation: pulse-ring 1.8s ease-out infinite;
}
.incoming-pulse::before  { width: 90px;  height: 90px; }
.incoming-pulse::after   { width: 110px; height: 110px; animation-delay: 0.4s; }

@keyframes pulse-ring {
  0%   { transform: scale(0.8); opacity: 0.8; }
  100% { transform: scale(1.4); opacity: 0; }
}

.incoming-name { font-size: 1.2rem; font-weight: 700; margin: 0; }
.incoming-type { font-size: 0.85rem; color: var(--engage-muted); margin: 0; display: flex; align-items: center; gap: 0.4rem; }
.incoming-actions { display: flex; gap: 2rem; margin-top: 0.5rem; }

/* ── Active call ────────────────────────────────────────────────────────────── */
.active-call {
  position: relative;
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
}

.remote-video {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.local-video {
  position: absolute;
  bottom: 6rem;
  right: 1.5rem;
  width: 120px;
  height: 90px;
  border-radius: 12px;
  object-fit: cover;
  border: 2px solid var(--engage-accent);
  z-index: 10;
}

.audio-only {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 1rem;
}

.call-header {
  display: flex;
  align-items: center;
  gap: 1rem;
}
.call-name   { font-size: 1.1rem; font-weight: 600; margin: 0; }
.call-status { font-size: 0.82rem; color: var(--engage-muted); margin: 0; }

.call-header-bar {
  position: absolute;
  top: 1.25rem;
  left: 50%;
  transform: translateX(-50%);
  display: flex;
  align-items: center;
  gap: 0.75rem;
  background: rgba(0,0,0,0.45);
  border-radius: 20px;
  padding: 0.3rem 0.9rem;
  z-index: 10;
}
.call-timer      { font-size: 0.85rem; color: #fff; font-variant-numeric: tabular-nums; }
.call-type-badge { font-size: 0.75rem; color: var(--engage-accent); display: flex; align-items: center; gap: 0.25rem; }

.call-controls {
  position: absolute;
  bottom: 2rem;
  display: flex;
  gap: 1.5rem;
  z-index: 10;
}

/* ── Call ended ─────────────────────────────────────────────────────────────── */
.ended-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.75rem;
  color: var(--engage-muted);
  font-size: 0.9rem;
}
</style>
