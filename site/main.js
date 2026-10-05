const REPO = "thedavidweng/sdf-flash-gui";
const RELEASES = `https://github.com/${REPO}/releases/latest`;

function detectOs() {
  const platform = (navigator.userAgentData?.platform || navigator.platform || "").toLowerCase();
  const ua = navigator.userAgent.toLowerCase();
  if (/iphone|ipad|android/.test(ua)) return null;
  if (platform.includes("mac") || ua.includes("mac os")) return "mac";
  if (platform.includes("win") || ua.includes("windows")) return "win";
  if (platform.includes("linux") || ua.includes("linux")) return "linux";
  return null;
}

const OS_LABEL = { mac: "Download for macOS", win: "Download for Windows", linux: "Download for Linux" };
const OS_PRIMARY_ASSET = { mac: "_aarch64.dmg", win: ".msi", linux: ".appimage" };

async function latestRelease() {
  const key = "sdf-flash-gui:latest-release";
  const cached = sessionStorage.getItem(key);
  if (cached) return JSON.parse(cached);
  const res = await fetch(`https://api.github.com/repos/${REPO}/releases/latest`, {
    headers: { Accept: "application/vnd.github+json" },
  });
  if (!res.ok) throw new Error(`GitHub API ${res.status}`);
  const json = await res.json();
  const release = {
    tag: json.tag_name,
    assets: (json.assets || []).map((a) => ({ name: a.name, url: a.browser_download_url })),
  };
  sessionStorage.setItem(key, JSON.stringify(release));
  return release;
}

function findAsset(release, suffix) {
  return release.assets.find((a) => a.name.toLowerCase().endsWith(suffix.toLowerCase()));
}

async function wireDownloads() {
  const os = detectOs();
  const primary = document.querySelector("[data-primary-download]");
  const label = document.querySelector("[data-primary-label]");
  if (os) {
    label.textContent = OS_LABEL[os];
    document.querySelector(`.card[data-os="${os}"]`)?.classList.add("current");
  } else {
    primary.href = "#install";
    label.textContent = "Get the desktop app";
  }

  let release;
  try {
    release = await latestRelease();
  } catch {
    return;
  }

  if (release.tag) {
    document.querySelectorAll("[data-version]").forEach((el) => (el.textContent = release.tag));
  }
  document.querySelectorAll("[data-asset]").forEach((a) => {
    const asset = findAsset(release, a.dataset.asset);
    if (asset) a.href = asset.url;
  });
  if (os) {
    const asset = findAsset(release, OS_PRIMARY_ASSET[os]);
    primary.href = asset ? asset.url : RELEASES;
  }
}

function wireCopy() {
  document.querySelectorAll("[data-copy]").forEach((box) => {
    const button = box.querySelector(".copy");
    button?.addEventListener("click", async () => {
      try {
        await navigator.clipboard.writeText(box.dataset.copy);
        button.textContent = "copied";
        button.classList.add("done");
      } catch {
        button.textContent = "⌘C";
      }
      setTimeout(() => {
        button.textContent = "copy";
        button.classList.remove("done");
      }, 1600);
    });
  });
}

const boot = document.querySelector("[data-boot]");
const bootLog = document.querySelector("[data-boot-log]");
const bootMeter = document.querySelector("[data-boot-meter]");
const bootButton = document.querySelector("[data-boot-start]");
let booted = false;

function log(text, cls) {
  const line = document.createElement("span");
  if (cls) line.className = cls;
  line.textContent = `\n${text}`;
  bootLog.appendChild(line);
  return line;
}

const mb = (n) => (n / 1048576).toFixed(1);

async function fetchWithProgress(url, onProgress) {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`${url}: HTTP ${res.status}`);
  const total = Number(res.headers.get("content-length")) || 0;
  if (!res.body) return new Uint8Array(await res.arrayBuffer());
  const reader = res.body.getReader();
  const chunks = [];
  let received = 0;
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    chunks.push(value);
    received += value.length;
    onProgress(received, Math.max(total, received));
  }
  const bytes = new Uint8Array(received);
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.length;
  }
  return bytes;
}

function hasWebGl() {
  try {
    const c = document.createElement("canvas");
    return !!(c.getContext("webgl2") || c.getContext("webgl"));
  } catch {
    return false;
  }
}

async function bootDemo() {
  if (booted) return;
  booted = true;
  boot.classList.add("running");

  if (!hasWebGl()) {
    log("error: WebGL is unavailable in this browser", "err");
    log("the desktop app looks exactly like this, minus the error", "err");
    return;
  }

  try {
    const fetching = log("  fetching sdf_flash_gui_bg.wasm");
    const bytes = await fetchWithProgress("demo/sdf_flash_gui_bg.wasm", (got, total) => {
      fetching.textContent = `\n  fetching sdf_flash_gui_bg.wasm  ${mb(got)} MB`;
      bootMeter.style.width = `${Math.min(100, (got / total) * 100)}%`;
    });
    log("  instantiating egui + glow renderer");
    const wasm = await import("./demo/sdf_flash_gui.js");
    await wasm.default({ module_or_path: bytes });
    log("  probing /dev/sr0 via simulated sdftool", "ok");
    await wasm.start("app");
    requestAnimationFrame(() => boot.classList.add("gone"));
  } catch (err) {
    console.error(err);
    log(`error: ${err?.message || err}`, "err");
  }
}

function wireDemo() {
  bootButton.addEventListener("click", bootDemo);
  const saveData = navigator.connection?.saveData;
  if (saveData || !("IntersectionObserver" in window)) {
    log("  press Boot demo to load the WebAssembly build");
    return;
  }
  const observer = new IntersectionObserver(
    (entries) => {
      if (entries.some((e) => e.isIntersecting)) {
        observer.disconnect();
        bootDemo();
      }
    },
    { threshold: 0.2 },
  );
  observer.observe(document.getElementById("app"));
}

wireCopy();
wireDemo();
wireDownloads();
