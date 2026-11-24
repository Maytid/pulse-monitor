const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const MAX_HIST = 600;
let hist = [];
let idx = -1;
let live = true;
let onlyPorts = false;
let query = "";
let down = false;
let pending = null;
const open = new Set();

const listEl = document.getElementById("list");
const memEl = document.getElementById("mem");
const pressEl = document.getElementById("press");
const pFillEl = document.getElementById("pFill");
const tsEl = document.getElementById("ts");
const scrubEl = document.getElementById("scrub");
const liveBtn = document.getElementById("liveBtn");
const portBtn = document.getElementById("portBtn");
const sparkEl = document.getElementById("spark");
const ctx = sparkEl.getContext("2d");
const modalEl = document.getElementById("modal");
const mTitle = document.getElementById("mTitle");
const mDesc = document.getElementById("mDesc");
const mCancel = document.getElementById("mCancel");
const mConfirm = document.getElementById("mConfirm");

window.addEventListener("pointerdown", () => { down = true; });
window.addEventListener("pointerup", () => { down = false; });
window.addEventListener("pointercancel", () => { down = false; });

function openConfirm(pid, name, force) {
    pending = { pid, force };
    mTitle.textContent = force ? "Force Quit Process?" : "Quit Process?";
    mDesc.textContent = force
        ? `Force quit "${name}" (PID ${pid})? Any unsaved changes will be lost immediately.`
        : `Quit "${name}" (PID ${pid})? The app will be asked to close normally.`;
    mConfirm.textContent = force ? "Force Quit" : "Quit";
    modalEl.classList.add("open");
    mCancel.focus();
}

function closeConfirm() {
    pending = null;
    modalEl.classList.remove("open");
}

mCancel.onclick = closeConfirm;

modalEl.onclick = (e) => {
    if (e.target === modalEl) closeConfirm();
};

window.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && pending) closeConfirm();
});

mConfirm.onclick = async () => {
    if (!pending) return;
    const { pid, force } = pending;
    closeConfirm();
    await invoke("kill_proc", { pid, force });
};

function fmtMem(b) {
    const mb = b / 1e6;
    return mb >= 1000 ? `${(b / 1e9).toFixed(2)} GB` : `${mb.toFixed(1)} MB`;
}

function fmtDelta(d) {
    const mb = d / 1e6;
    if (Math.abs(mb) < 0.1) return "-";
    return `${mb > 0 ? "+" : ""}${mb.toFixed(1)}M`;
}

function fmtPressure(lvl) {
    if (lvl >= 4) return { label: "Critical", cls: "lvl-4" };
    if (lvl >= 2) return { label: "Warning", cls: "lvl-2" };
    return { label: "Normal", cls: "lvl-1" };
}

function syncScrub() {
    const max = Math.max(0, hist.length - 1);
    scrubEl.max = max;
    scrubEl.value = idx;
    const pct = max > 0 ? (idx / max) * 100 : 100;
    scrubEl.style.setProperty("--p", `${pct}%`);
}

function matchProc(p) {
    const q = query;
    const hitSelf = (!onlyPorts || p.ports.length > 0) && (
        !q ||
        p.name.toLowerCase().includes(q) ||
        String(p.pid).includes(q) ||
        p.ports.some(pt => String(pt).includes(q))
    );
    const kids = p.children.map(matchProc).filter(Boolean);
    if (hitSelf || kids.length > 0) {
        return { ...p, children: kids };
    }
    return null;
}

function drawSpark() {
    const w = sparkEl.width;
    const h = sparkEl.height;
    ctx.clearRect(0, 0, w, h);
    if (hist.length < 2) return;

    const vals = hist.map(t => t.total_mem);
    const min = Math.min(...vals);
    const max = Math.max(...vals, min + 1);

    ctx.strokeStyle = "#38bdf8";
    ctx.lineWidth = 1.5;
    ctx.beginPath();
    vals.forEach((v, i) => {
        const x = (i / (vals.length - 1)) * w;
        const y = h - 3 - ((v - min) / (max - min)) * (h - 6);
        if (i === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
    });
    ctx.stroke();

    if (idx >= 0 && idx < vals.length) {
        const x = (idx / (vals.length - 1)) * w;
        ctx.strokeStyle = "#f87171";
        ctx.beginPath();
        ctx.moveTo(x, 0);
        ctx.lineTo(x, h);
        ctx.stroke();
    }
}

function flattenProcs(procs, depth = 0, out = []) {
    for (const p of procs) {
        out.push({ p, depth });
        if (p.children.length > 0 && (open.has(p.pid) || query || onlyPorts)) {
            flattenProcs(p.children, depth + 1, out);
        }
    }
    return out;
}

function createRow(p, depth) {
    const tr = document.createElement("tr");
    tr.dataset.pid = p.pid;
    tr.innerHTML = `
        <td>
            <div class="name-cell">
                <span class="exp" data-exp="${p.pid}"></span>
                <span class="pname"></span>
                <span class="kids"></span>
            </div>
        </td>
        <td class="num">${p.pid}</td>
        <td class="num"></td>
        <td class="num"></td>
        <td class="num"></td>
        <td></td>
        <td class="act">
            <button class="btn-k" data-kill="${p.pid}" data-force="0">Quit</button>
            <button class="btn-k" data-kill="${p.pid}" data-force="1">Force Quit</button>
        </td>
    `;
    updateRow(tr, p, depth);
    return tr;
}

function updateRow(tr, p, depth) {
    const cls = depth > 0 ? "child" : "";
    if (tr.className !== cls) tr.className = cls;

    const cells = tr.children;
    const nameCell = cells[0].firstElementChild;
    const pad = `${depth * 14}px`;
    if (nameCell.style.paddingLeft !== pad) nameCell.style.paddingLeft = pad;

    const hasKids = p.children.length > 0;
    const isOpen = open.has(p.pid);
    const expText = hasKids ? (isOpen ? "▼" : "▶") : "";
    const expEl = nameCell.children[0];
    if (expEl.textContent !== expText) expEl.textContent = expText;

    const pnameEl = nameCell.children[1];
    if (pnameEl.textContent !== p.name) {
        pnameEl.textContent = p.name;
        pnameEl.title = p.name;
    }

    const kidsEl = nameCell.children[2];
    if (hasKids) {
        const countStr = String(p.children.length);
        if (kidsEl.style.display === "none") kidsEl.style.display = "";
        if (kidsEl.textContent !== countStr) kidsEl.textContent = countStr;
    } else if (kidsEl.style.display !== "none") {
        kidsEl.style.display = "none";
    }

    const cpuText = `${p.cpu.toFixed(1)}%`;
    if (cells[2].textContent !== cpuText) cells[2].textContent = cpuText;

    const memText = fmtMem(p.mem);
    if (cells[3].textContent !== memText) cells[3].textContent = memText;

    const dCls = p.delta > 2e6 ? "num spike" : p.delta < -2e6 ? "num drop" : "num";
    if (cells[4].className !== dCls) cells[4].className = dCls;
    const delText = fmtDelta(p.delta);
    if (cells[4].textContent !== delText) cells[4].textContent = delText;

    const pKey = p.ports.join(",");
    if (tr.dataset.ports !== pKey) {
        tr.dataset.ports = pKey;
        cells[5].title = p.ports.map(pt => ":" + pt).join(" ");
        cells[5].innerHTML = p.ports.map(pt => `<span class="port">:${pt}</span>`).join("");
    }
}

function render() {
    if (!hist.length) return;
    if (live) idx = hist.length - 1;

    syncScrub();

    const tick = hist[idx];
    if (!tick) return;

    memEl.textContent = fmtMem(tick.total_mem);

    const p = tick.pressure ?? 0;
    const { label, cls } = fmtPressure(tick.pressure_level ?? 1);
    pressEl.textContent = `${p}% ${label}`;
    pressEl.className = `pval ${cls}`;
    pFillEl.style.width = `${p}%`;
    pFillEl.className = `pfill ${cls}`;

    tsEl.textContent = new Date(tick.ts * 1000).toLocaleTimeString([], { hour12: false });

    const procs = tick.procs.map(matchProc).filter(Boolean);
    const rows = flattenProcs(procs);
    const existing = new Map();
    for (const tr of listEl.children) {
        existing.set(Number(tr.dataset.pid), tr);
    }

    rows.forEach(({ p, depth }, i) => {
        let tr = existing.get(p.pid);
        if (tr) {
            existing.delete(p.pid);
            updateRow(tr, p, depth);
        } else {
            tr = createRow(p, depth);
        }
        if (listEl.children[i] !== tr) {
            listEl.insertBefore(tr, listEl.children[i] || null);
        }
    });

    for (const tr of existing.values()) {
        tr.remove();
    }

    drawSpark();
}

document.getElementById("q").oninput = (e) => {
    query = e.target.value.trim().toLowerCase();
    render();
};

portBtn.onclick = () => {
    onlyPorts = !onlyPorts;
    portBtn.classList.toggle("active", onlyPorts);
    render();
};

liveBtn.onclick = () => {
    live = !live;
    liveBtn.classList.toggle("active", live);
    if (live) render();
};

scrubEl.oninput = (e) => {
    live = false;
    liveBtn.classList.remove("active");
    idx = Number(e.target.value);
    render();
};

listEl.onclick = (e) => {
    const exp = e.target.closest("[data-exp]");
    if (exp && exp.dataset.exp) {
        const pid = Number(exp.dataset.exp);
        if (open.has(pid)) open.delete(pid);
        else open.add(pid);
        render();
        return;
    }

    const btn = e.target.closest("[data-kill]");
    if (btn) {
        const pid = Number(btn.dataset.kill);
        const force = btn.dataset.force === "1";
        const name = btn.closest("tr")?.querySelector(".pname")?.textContent || String(pid);
        openConfirm(pid, name, force);
    }
};

document.addEventListener("visibilitychange", async () => {
    if (!document.hidden && live) {
        hist = await invoke("get_history");
        render();
    }
});

(async () => {
    hist = await invoke("get_history");
    render();
    await listen("tick", (e) => {
        hist.push(e.payload);
        if (hist.length > MAX_HIST) {
            hist.shift();
            if (!live && idx > 0) idx--;
        }
        if (live && !down) render();
        else {
            syncScrub();
            drawSpark();
        }
    });
})();