// @faridguzman: WebRTC composable — peer-to-peer voice and video calls.
//
// Signaling transport: the existing engage WebSocket connection.
// NAT traversal:      Google STUN (dev) + coturn TURN (prod).
// Media encryption:   DTLS-SRTP (mandatory in all modern browsers/Tauri webview).
//
// Call state machine:
//   idle → calling → active     (caller side)
//   idle → ringing → active     (callee side)
//   any  → ended                (hangup or error)
//
// The composable is module-level so any component can read call state reactively.

import { ref, computed } from "vue";
import { useServerApi } from "./useServerApi";

// ── Types ─────────────────────────────────────────────────────────────────────

export type CallStatus = "idle" | "calling" | "ringing" | "active" | "ended";

export interface IncomingCall {
  callId: string;
  fromUserId: string;
  sdp: string;
  isVideo: boolean;
}

// ── Module-level state (shared across all component instances) ────────────────

const status     = ref<CallStatus>("idle");
const callId     = ref<string>("");
const peerId     = ref<string>("");
const isVideo    = ref(false);
const incoming   = ref<IncomingCall | null>(null);

// @faridguzman: localStream is the microphone/camera feed captured on this device.
// remoteStream is the decoded media arriving from the peer.
const localStream  = ref<MediaStream | null>(null);
const remoteStream = ref<MediaStream | null>(null);

const isMuted   = ref(false);
const isCamOff  = ref(false);

let pc: RTCPeerConnection | null = null;

// @faridguzman: sendSignal is injected by useWebSocket on first call so we
// avoid a circular import (useWebSocket → useWebRTC → useWebSocket).
let _send: ((payload: unknown) => void) | null = null;

export function injectSend(fn: (payload: unknown) => void) {
  _send = fn;
}

function send(payload: unknown) {
  _send?.(payload);
}

// ── ICE server config ─────────────────────────────────────────────────────────

interface IceServer { urls: string[]; username?: string; credential?: string; }

async function getIceServers(): Promise<IceServer[]> {
  try {
    const api = useServerApi();
    const { iceServers } = await api.fetchTurnCredentials();
    return iceServers;
  } catch {
    // @faridguzman: Fall back to public STUN if the server is unreachable or
    // TURN_SECRET is not configured.  Works fine for LAN and open-NAT calls.
    return [{ urls: ["stun:stun.l.google.com:19302"] }];
  }
}

// ── RTCPeerConnection lifecycle ───────────────────────────────────────────────

async function createPeerConnection(targetId: string, iceServers: IceServer[]) {
  pc = new RTCPeerConnection({ iceServers });

  // @faridguzman: Trickle ICE — send each candidate as it is discovered rather
  // than waiting for gathering to complete (reduces call setup latency).
  pc.onicecandidate = (e) => {
    if (!e.candidate) return;
    send({
      type: "ice_candidate",
      to: targetId,
      callId: callId.value,
      candidate: e.candidate.candidate,
      sdpMid: e.candidate.sdpMid,
      sdpMLineIndex: e.candidate.sdpMLineIndex,
    });
  };

  pc.ontrack = (e) => {
    // @faridguzman: First track event creates the remoteStream; subsequent
    // tracks (audio + video) are added to the same stream.
    if (!remoteStream.value) {
      remoteStream.value = new MediaStream();
    }
    remoteStream.value.addTrack(e.track);
  };

  pc.onconnectionstatechange = () => {
    if (pc?.connectionState === "disconnected" || pc?.connectionState === "failed") {
      useWebRTC().endCall();
    }
  };

  return pc;
}

// ── Public API ────────────────────────────────────────────────────────────────

export function useWebRTC() {

  // @faridguzman: Start an outbound call to targetId.
  async function startCall(targetId: string, withVideo: boolean) {
    if (status.value !== "idle") return;

    const id = crypto.randomUUID();
    callId.value = id;
    peerId.value = targetId;
    isVideo.value = withVideo;
    status.value = "calling";

    try {
      // Capture local media before creating the offer so tracks are attached
      localStream.value = await navigator.mediaDevices.getUserMedia({
        audio: true,
        video: withVideo,
      });

      const iceServers = await getIceServers();
      const conn = await createPeerConnection(targetId, iceServers);

      localStream.value.getTracks().forEach((t) => conn.addTrack(t, localStream.value!));

      const offer = await conn.createOffer();
      await conn.setLocalDescription(offer);

      send({
        type: "call_offer",
        to: targetId,
        callId: id,
        sdp: offer.sdp,
        isVideo: withVideo,
      });
    } catch (err) {
      console.error("[WebRTC] startCall failed:", err);
      endCall();
    }
  }

  // @faridguzman: Called when the callee accepts an incoming call.
  async function acceptCall() {
    const inc = incoming.value;
    if (!inc || status.value !== "ringing") return;

    status.value = "active";
    incoming.value = null;
    peerId.value = inc.fromUserId;
    callId.value = inc.callId;
    isVideo.value = inc.isVideo;

    try {
      localStream.value = await navigator.mediaDevices.getUserMedia({
        audio: true,
        video: inc.isVideo,
      });

      const iceServers = await getIceServers();
      const conn = await createPeerConnection(inc.fromUserId, iceServers);

      localStream.value.getTracks().forEach((t) => conn.addTrack(t, localStream.value!));

      await conn.setRemoteDescription({ type: "offer", sdp: inc.sdp });
      const answer = await conn.createAnswer();
      await conn.setLocalDescription(answer);

      send({
        type: "call_answer",
        to: inc.fromUserId,
        callId: inc.callId,
        sdp: answer.sdp,
      });
    } catch (err) {
      console.error("[WebRTC] acceptCall failed:", err);
      endCall();
    }
  }

  // @faridguzman: Decline an incoming call without establishing a connection.
  function declineCall() {
    const inc = incoming.value;
    if (!inc) return;
    send({ type: "call_hangup", to: inc.fromUserId, callId: inc.callId });
    incoming.value = null;
    status.value = "idle";
  }

  // @faridguzman: Handle an SDP answer arriving from the callee.
  async function handleAnswer(sdp: string) {
    if (!pc) return;
    await pc.setRemoteDescription({ type: "answer", sdp }).catch((e) =>
      console.error("[WebRTC] setRemoteDescription(answer):", e)
    );
    status.value = "active";
  }

  // @faridguzman: Add a trickled ICE candidate from the remote peer.
  async function handleIceCandidate(
    candidate: string,
    sdpMid: string | null,
    sdpMLineIndex: number | null
  ) {
    if (!pc) return;
    await pc
      .addIceCandidate({ candidate, sdpMid: sdpMid ?? undefined, sdpMLineIndex: sdpMLineIndex ?? undefined })
      .catch((e) => console.error("[WebRTC] addIceCandidate:", e));
  }

  // @faridguzman: Receive an incoming call offer.  Transitions to "ringing"
  // so the UI can show the incoming call dialog.
  function handleOffer(offer: IncomingCall) {
    if (status.value !== "idle") {
      // Already in a call — decline automatically
      send({ type: "call_hangup", to: offer.fromUserId, callId: offer.callId });
      return;
    }
    incoming.value = offer;
    status.value = "ringing";
  }

  // @faridguzman: Tear down the peer connection and reset all state.
  function endCall() {
    localStream.value?.getTracks().forEach((t) => t.stop());
    remoteStream.value?.getTracks().forEach((t) => t.stop());
    localStream.value  = null;
    remoteStream.value = null;

    if (pc) {
      pc.close();
      pc = null;
    }

    // Notify remote peer if we were active or calling
    if (status.value === "active" || status.value === "calling") {
      send({ type: "call_hangup", to: peerId.value, callId: callId.value });
    }

    incoming.value = null;
    status.value   = "ended";

    // Reset to idle after a brief pause so UI can show "call ended"
    setTimeout(() => {
      if (status.value === "ended") status.value = "idle";
    }, 2000);
  }

  function toggleMute() {
    localStream.value?.getAudioTracks().forEach((t) => {
      t.enabled = !t.enabled;
    });
    isMuted.value = !isMuted.value;
  }

  function toggleCamera() {
    localStream.value?.getVideoTracks().forEach((t) => {
      t.enabled = !t.enabled;
    });
    isCamOff.value = !isCamOff.value;
  }

  const isInCall   = computed(() => status.value === "active");
  const isCalling  = computed(() => status.value === "calling");
  const isRinging  = computed(() => status.value === "ringing");

  return {
    status,
    callId,
    peerId,
    isVideo,
    incoming,
    localStream,
    remoteStream,
    isMuted,
    isCamOff,
    isInCall,
    isCalling,
    isRinging,
    startCall,
    acceptCall,
    declineCall,
    handleAnswer,
    handleIceCandidate,
    handleOffer,
    endCall,
    toggleMute,
    toggleCamera,
  };
}
