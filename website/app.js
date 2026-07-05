const commands = {
  windows: {
    dev: {
      title: "Windows host smoke test",
      body: [
        "$env:GLYPHRAY_DEV_AUTO_APPROVE='1'",
        "$env:GLYPHRAY_ENABLE_PEN_INJECTION='1'",
        "$env:GLYPHRAY_ENABLE_TOUCH_INJECTION='1'",
        "$env:GLYPHRAY_ENABLE_MOUSE_INJECTION='1'",
        "$env:GLYPHRAY_ENABLE_KEYBOARD_INJECTION='1'",
        "$env:GLYPHRAY_ENABLE_VIDEO_STREAM='1'",
        "cargo run -p glyphray-windows-host -- serve",
      ].join("\n"),
    },
    safe: {
      title: "Windows host without injection flags",
      body: "cargo run -p glyphray-windows-host -- serve",
    },
  },
  android: {
    dev: {
      title: "Android debug build",
      body: ".\\gradlew.bat :apps:android-client:assembleDebug",
    },
    safe: {
      title: "Android unit test then debug build",
      body: [
        ".\\gradlew.bat :apps:android-client:testDebugUnitTest",
        ".\\gradlew.bat :apps:android-client:assembleDebug",
      ].join("\n"),
    },
  },
  rust: {
    dev: {
      title: "Rust workspace tests",
      body: "cargo test --workspace",
    },
    safe: {
      title: "Rust diagnostics build",
      body: [
        "cargo test --workspace",
        "cargo run -p glyphray-host-diagnostics",
      ].join("\n"),
    },
  },
  macos: {
    dev: {
      title: "macOS host preview with hardware encoder gate",
      body: [
        "cd hosts/macos-host",
        "swift build",
        "swift run GlyphRayMacHost",
      ].join("\n"),
    },
    safe: {
      title: "macOS host tests",
      body: [
        "cd hosts/macos-host",
        "swift test",
      ].join("\n"),
    },
  },
};

const platformSelect = document.querySelector("#platformSelect");
const channelSelect = document.querySelector("#channelSelect");
const commandTitle = document.querySelector("#commandTitle");
const commandOutput = document.querySelector("#commandOutput");
const copyButton = document.querySelector("#copyCommand");
const copyStatus = document.querySelector("#copyStatus");

function selectedCommand() {
  const platform = platformSelect.value;
  const channel = channelSelect.value;
  return commands[platform][channel] || commands[platform].dev;
}

function renderCommand() {
  const command = selectedCommand();
  commandTitle.textContent = command.title;
  commandOutput.textContent = command.body;
  copyStatus.textContent = "";
}

async function copyCommand() {
  const command = selectedCommand().body;
  try {
    await navigator.clipboard.writeText(command);
    copyStatus.textContent = "Copied to clipboard.";
  } catch {
    copyStatus.textContent = "Select the command text and copy it manually.";
  }
}

platformSelect.addEventListener("change", renderCommand);
channelSelect.addEventListener("change", renderCommand);
copyButton.addEventListener("click", copyCommand);
renderCommand();
