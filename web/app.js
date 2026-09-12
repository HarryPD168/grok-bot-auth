const $ = (id) => document.getElementById(id);

async function api(path, options = {}) {
  const response = await fetch(path, {
    headers: { "content-type": "application/json", ...(options.headers || {}) },
    ...options,
  });
  const text = await response.text();
  let body = {};
  try { body = text ? JSON.parse(text) : {}; } catch { body = { raw: text }; }
  if (!response.ok) throw new Error(body.error || text || response.status);
  return body;
}

function renderAccount(account) {
  if (!account || !account.signedIn) {
    $("account-line").textContent = "未登录";
    return;
  }
  const state = account.sandState ? ` · ${account.sandState}` : "";
  $("account-line").textContent = `${account.displayName}${state}`;
}

function renderModels(models) {
  const select = $("model");
  const list = $("model-list");
  const current = select.value;
  const rows = [...(models || [])];
  select.innerHTML = "";
  if (list) list.innerHTML = "";
  for (const model of rows) {
    const option = document.createElement("option");
    option.value = model.id;
    const tag = model.available ? "可用" : "目录";
    option.textContent = `${model.displayName || model.id} · ${tag}${model.contextTokenLimit ? ` · ${model.contextTokenLimit}` : ""}`;
    option.dataset.available = model.available ? "1" : "0";
    select.appendChild(option);
    if (list) {
      const card = document.createElement("button");
      card.type = "button";
      card.className = "model-card";
      card.dataset.id = model.id;
      card.setAttribute("role", "option");
      const name = document.createElement("span");
      name.className = "name";
      name.textContent = model.displayName || model.id;
      const tagEl = document.createElement("span");
      tagEl.className = `tag ${model.available ? "ok" : "dir"}`;
      tagEl.textContent = tag;
      card.replaceChildren(name, tagEl);
      card.onclick = () => {
        select.value = model.id;
        renderModels(rows);
      };
      list.appendChild(card);
    }
  }
  if (!select.options.length) {
    const option = document.createElement("option");
    option.value = "grok-4.6";
    option.textContent = "grok-4.6 · 可用";
    option.dataset.available = "1";
    select.appendChild(option);
  }
  if ([...select.options].some((item) => item.value === current)) {
    select.value = current;
  } else {
    const available = rows.find((item) => item.available);
    if (available) select.value = available.id;
  }
  if (list) {
    for (const card of list.querySelectorAll(".model-card")) {
      card.setAttribute("aria-selected", card.dataset.id === select.value ? "true" : "false");
    }
  }
  const selected = rows.find((item) => item.id === select.value);
  const gb = selected ? `gb-${selected.id}` : "";
  $("model-meta").textContent = selected
    ? `${selected.available ? "Stream 可用" : "仅目录"} · Cursor id ${gb}${selected.axes?.length ? ` · 轴 ${selected.axes.map((axis) => axis.id).join(", ")}` : ""}`
    : "";
}

async function refreshStatus() {
  const status = await api("/api/status");
  $("server-status").textContent = `${status.name} ${status.version} · OpenAI ${status.openai.baseUrl}`;
  renderAccount(status.account);
  renderModels(status.models);
  const cursor = status.cursor || {};
  $("cursor-line").textContent = cursor.coexist
    ? `已启用 ${cursor.proxyUrl || ""}`
    : (cursor.settingsProxy ? `未启用（Cursor 当前代理 ${cursor.settingsProxy}）` : "未接入");
}

$("btn-import").onclick = async () => {
  $("login-help").textContent = "";
  try {
    const parsed = JSON.parse($("import-json").value);
    const body = await api("/api/import", { method: "POST", body: JSON.stringify(parsed) });
    renderAccount(body.account);
  } catch (error) {
    $("login-help").textContent = error.message;
  }
};

$("btn-logout").onclick = async () => {
  await api("/api/logout", { method: "POST", body: "{}" });
  await refreshStatus();
};

$("btn-refresh").onclick = async () => {
  $("login-help").textContent = "";
  try {
    const body = await api("/api/refresh", { method: "POST", body: "{}" });
    renderAccount(body.account);
  } catch (error) {
    $("login-help").textContent = error.message;
  }
};

$("btn-login").onclick = async () => {
  $("login-help").textContent = "";
  try {
    const start = await api("/api/login/start", { method: "POST", body: "{}" });
    window.open(start.loginUrl, "_blank", "noopener");
    $("login-help").textContent = "已打开 Cursor 登录页，正在轮询…";
    const timer = setInterval(async () => {
      try {
        const poll = await api("/api/login/poll", { method: "POST", body: JSON.stringify({ id: start.id }) });
        if (poll.status === "completed") {
          clearInterval(timer);
          renderAccount(poll.account);
          $("login-help").textContent = "登录完成";
        }
      } catch (error) {
        clearInterval(timer);
        $("login-help").textContent = error.message;
      }
    }, 1200);
  } catch (error) {
    $("login-help").textContent = error.message;
  }
};

$("btn-cursor-on").onclick = async () => {
  $("cursor-help").textContent = "";
  try {
    const body = await api("/api/cursor/enable", { method: "POST", body: "{}" });
    $("cursor-help").textContent = `代理 ${body.proxyUrl}。请完全退出并重启 Cursor，新开对话后在模型列表里选 Grok Bot。`;
    await refreshStatus();
  } catch (error) {
    $("cursor-help").textContent = error.message;
  }
};

$("btn-cursor-off").onclick = async () => {
  $("cursor-help").textContent = "";
  try {
    await api("/api/cursor/disable", { method: "POST", body: "{}" });
    $("cursor-help").textContent = "已停用。重启 Cursor 后只走官方账号。";
    await refreshStatus();
  } catch (error) {
    $("cursor-help").textContent = error.message;
  }
};

$("model").onchange = () => {
  const options = [...$("model").options].map((option) => ({
    id: option.value,
    displayName: option.textContent,
    available: option.dataset.available === "1",
  }));
  const list = $("model-list");
  if (!list) return;
  for (const card of list.querySelectorAll(".model-card")) {
    card.setAttribute("aria-selected", card.dataset.id === $("model").value ? "true" : "false");
  }
};

$("btn-sync").onclick = async () => {
  $("model-meta").textContent = "同步中…";
  try {
    const body = await api("/api/models/sync", { method: "POST", body: "{}" });
    renderModels(body.models);
  } catch (error) {
    $("model-meta").textContent = error.message;
  }
};

$("btn-send").onclick = async () => {
  $("think").textContent = "";
  $("out").textContent = "";
  const response = await fetch("/api/chat", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      message: $("prompt").value,
      model: $("model").value,
      effort: $("effort").value,
      fast: $("fast").checked,
    }),
  });
  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";
  while (true) {
    const { value, done } = await reader.read();
    if (done) break;
    buffer += decoder.decode(value, { stream: true });
    const chunks = buffer.split("\n\n");
    buffer = chunks.pop() || "";
    for (const chunk of chunks) {
      const event = (chunk.match(/^event: (.*)$/m) || [])[1] || "message";
      const data = (chunk.match(/^data: (.*)$/m) || [])[1] || "";
      if (event === "thinking") $("think").textContent += data;
      if (event === "text") $("out").textContent += data;
      if (event === "error") $("out").textContent += `\n[error] ${data}`;
    }
  }
};

refreshStatus().catch((error) => {
  $("server-status").textContent = error.message;
});
