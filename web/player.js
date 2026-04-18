const fileInput = document.getElementById("file-input");
const playButton = document.getElementById("play-button");
const resetButton = document.getElementById("reset-button");
const coverImage = document.getElementById("cover-image");
const motionVideo = document.getElementById("motion-video");
const manifestOutput = document.getElementById("manifest-output");
const chunksOutput = document.getElementById("chunks-output");
const placeholder = document.getElementById("placeholder");
const viewer = document.getElementById("viewer");

const textDecoder = new TextDecoder("utf-8");

let currentState = {
  photoUrl: null,
  videoUrl: null,
  manifest: null,
};

fileInput.addEventListener("change", async (event) => {
  const [file] = event.target.files ?? [];
  if (!file) return;

  cleanupUrls();

  try {
    const parsed = await parseLivePhoto(file);
    currentState.manifest = parsed.manifest;

    currentState.photoUrl = URL.createObjectURL(
      new Blob([parsed.photo.payload], { type: parsed.manifest.photo_mime }),
    );
    currentState.videoUrl = URL.createObjectURL(
      new Blob([parsed.video.payload], { type: parsed.manifest.video_mime }),
    );

    coverImage.src = currentState.photoUrl;
    motionVideo.src = currentState.videoUrl;
    motionVideo.currentTime = 0;
    motionVideo.muted = parsed.manifest.playback.muted_by_default ?? false;

    manifestOutput.textContent = JSON.stringify(parsed.manifest, null, 2);
    chunksOutput.textContent = parsed.chunks
      .map(
        (chunk) =>
          `${chunk.kind} id=${chunk.chunkId} offset=${chunk.offset} stored_length=${chunk.storedLength} flags=0x${chunk.flags.toString(16)}`,
      )
      .join("\n");

    placeholder.hidden = true;
    playButton.disabled = false;
    resetButton.disabled = false;
    showCover();
  } catch (error) {
    console.error(error);
    manifestOutput.textContent = `解析失败: ${error.message}`;
    chunksOutput.textContent = "无";
    placeholder.hidden = false;
    playButton.disabled = true;
    resetButton.disabled = true;
    coverImage.hidden = true;
    motionVideo.hidden = true;
  }
});

playButton.addEventListener("click", playMotion);
resetButton.addEventListener("click", resetToCover);

viewer.addEventListener("click", () => {
  if (!playButton.disabled) {
    playMotion();
  }
});

let pressTimer = null;

viewer.addEventListener("pointerdown", () => {
  if (playButton.disabled) return;
  pressTimer = window.setTimeout(() => {
    playMotion();
  }, 180);
});

viewer.addEventListener("pointerup", clearPressTimer);
viewer.addEventListener("pointercancel", clearPressTimer);
viewer.addEventListener("pointerleave", clearPressTimer);

motionVideo.addEventListener("ended", resetToCover);

function clearPressTimer() {
  if (pressTimer !== null) {
    window.clearTimeout(pressTimer);
    pressTimer = null;
  }
}

function showCover() {
  coverImage.hidden = false;
  motionVideo.hidden = true;
}

async function playMotion() {
  if (!motionVideo.src) return;
  coverImage.hidden = true;
  motionVideo.hidden = false;
  motionVideo.currentTime = 0;
  await motionVideo.play();
}

function resetToCover() {
  motionVideo.pause();
  motionVideo.currentTime = 0;
  showCover();
}

function cleanupUrls() {
  if (currentState.photoUrl) {
    URL.revokeObjectURL(currentState.photoUrl);
  }
  if (currentState.videoUrl) {
    URL.revokeObjectURL(currentState.videoUrl);
  }
  currentState = {
    photoUrl: null,
    videoUrl: null,
    manifest: null,
  };
}

async function parseLivePhoto(file) {
  const buffer = await file.arrayBuffer();
  const bytes = new Uint8Array(buffer);
  const view = new DataView(buffer);

  const magic = readAscii(bytes, 0, 4);
  if (magic !== "LPHF") {
    throw new Error(`非法 magic: ${magic}`);
  }

  const headerSize = view.getUint32(8, true);
  const tocOffset = Number(view.getBigUint64(20, true));
  const tocLength = Number(view.getBigUint64(28, true));
  const fileSize = Number(view.getBigUint64(36, true));
  const primaryManifestId = Number(view.getBigUint64(44, true));

  if (headerSize !== 68) {
    throw new Error(`file header_size 必须为 68，实际为 ${headerSize}`);
  }

  if (fileSize !== bytes.length) {
    throw new Error(`file_size 不匹配: header=${fileSize}, actual=${bytes.length}`);
  }

  const tocChunk = parseChunk(bytes, tocOffset);
  if (tocChunk.kind !== "TOCC") {
    throw new Error("TOC offset 未指向 TOCC chunk");
  }

  const tocPayload = parseToc(tocChunk.payload.buffer.slice(
    tocChunk.payload.byteOffset,
    tocChunk.payload.byteOffset + tocChunk.payload.byteLength,
  ));

  const chunks = tocPayload.map((entry) => ({
    ...entry,
    ...parseChunk(bytes, entry.offset),
  }));

  const manifestChunk = chunks.find((chunk) => chunk.chunkId === primaryManifestId);
  if (!manifestChunk || manifestChunk.kind !== "META") {
    throw new Error("未找到主 META chunk");
  }

  const manifest = JSON.parse(textDecoder.decode(manifestChunk.payload));
  const photo = chunks.find((chunk) => chunk.chunkId === manifest.photo_chunk_id);
  const video = chunks.find((chunk) => chunk.chunkId === manifest.video_chunk_id);

  if (!photo || !video) {
    throw new Error("manifest 中的 photo_chunk_id 或 video_chunk_id 无法解析");
  }

  return {
    headerSize,
    manifest,
    photo,
    video,
    chunks,
  };
}

function parseChunk(bytes, offset) {
  const view = new DataView(bytes.buffer, bytes.byteOffset + offset);
  const chunkType = readAscii(bytes, offset, 4);
  const headerSize = view.getUint16(6, true);
  const chunkId = Number(view.getBigUint64(8, true));
  const flags = Number(view.getBigUint64(16, true));
  const storedLength = Number(view.getBigUint64(24, true));

  if (headerSize !== 48) {
    throw new Error(`chunk ${chunkType} header_size 必须为 48，实际为 ${headerSize}`);
  }

  const payloadStart = offset + headerSize;
  const payloadEnd = payloadStart + storedLength;
  const payload = bytes.slice(payloadStart, payloadEnd);

  return {
    kind: chunkType,
    headerSize,
    chunkId,
    flags,
    storedLength,
    offset,
    payload,
  };
}

function parseToc(buffer) {
  const view = new DataView(buffer);
  const entryCount = view.getUint32(0, true);
  const entries = [];
  let offset = 8;

  for (let index = 0; index < entryCount; index += 1) {
    const chunkId = Number(view.getBigUint64(offset, true));
    const kind = readAscii(new Uint8Array(buffer), offset + 8, 4);
    const fileOffset = Number(view.getBigUint64(offset + 16, true));
    const totalLength = Number(view.getBigUint64(offset + 24, true));
    const storedLength = Number(view.getBigUint64(offset + 32, true));
    const flags = Number(view.getBigUint64(offset + 40, true));

    entries.push({
      chunkId,
      kind,
      offset: fileOffset,
      totalLength,
      storedLength,
      flags,
    });

    offset += 48;
  }

  return entries;
}

function readAscii(bytes, offset, length) {
  return String.fromCharCode(...bytes.slice(offset, offset + length));
}
