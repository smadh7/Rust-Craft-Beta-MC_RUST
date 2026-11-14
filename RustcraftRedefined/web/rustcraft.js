// Rustcraft Redefined - browser host.
// Forwards WebGL 2, input, storage and timing between the page and the WebAssembly game.
'use strict';
(() => {
  const canvas = document.getElementById('game');
  const status = document.getElementById('status');
  const fail = (msg) => { status.textContent = msg; status.style.display = 'flex'; console.error(msg); };
  const gl = canvas.getContext('webgl2', { antialias: false, alpha: false, depth: true, stencil: false, powerPreference: 'high-performance', preserveDrawingBuffer: true });
  if (!gl) { fail('This browser does not provide WebGL 2, which Rustcraft Redefined needs.'); return; }

  let wasm = null;
  let memory = null;
  const decoder = new TextDecoder();
  const bytes = (ptr, len) => new Uint8Array(memory.buffer, ptr, len);
  const text = (ptr, len) => decoder.decode(bytes(ptr, len));

  // Integer handle tables for WebGL objects (index 0 means "none").
  const table = () => ({ items: [null], free: [] });
  const programs = table(), buffers = table(), vaos = table(), textures = table(), uniforms = table();
  const put = (t, obj) => { const id = t.free.length ? t.free.pop() : t.items.length; t.items[id] = obj; return id; };
  const drop = (t, id) => { if (id > 0 && t.items[id]) { t.items[id] = null; t.free.push(id); } };

  const compile = (kind, src) => {
    const s = gl.createShader(kind);
    gl.shaderSource(s, src);
    gl.compileShader(s);
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error('Shader compile error:\n' + gl.getShaderInfoLog(s));
    return s;
  };

  const glImports = {
    viewport: (x, y, w, h) => gl.viewport(x, y, w, h),
    clear_color: (r, g, b, a) => gl.clearColor(r, g, b, a),
    clear: (mask) => gl.clear(mask),
    enable: (cap) => gl.enable(cap),
    disable: (cap) => gl.disable(cap),
    blend_func: (s, d) => gl.blendFunc(s, d),
    depth_func: (f) => gl.depthFunc(f),
    depth_mask: (on) => gl.depthMask(!!on),
    cull_face: (mode) => gl.cullFace(mode),
    polygon_offset: (f, u) => gl.polygonOffset(f, u),
    create_program: (vs, vsLen, fs, fsLen, attribs, attribsLen) => {
      const p = gl.createProgram();
      gl.attachShader(p, compile(gl.VERTEX_SHADER, text(vs, vsLen)));
      gl.attachShader(p, compile(gl.FRAGMENT_SHADER, text(fs, fsLen)));
      text(attribs, attribsLen).split(',').forEach((name, i) => { if (name) gl.bindAttribLocation(p, i, name); });
      gl.linkProgram(p);
      if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error('Program link error:\n' + gl.getProgramInfoLog(p));
      return put(programs, p);
    },
    use_program: (p) => gl.useProgram(programs.items[p]),
    uniform_location: (p, name, len) => { const loc = gl.getUniformLocation(programs.items[p], text(name, len)); return loc ? put(uniforms, loc) : -1; },
    uniform1i: (loc, v) => { if (loc >= 0) gl.uniform1i(uniforms.items[loc], v); },
    uniform1f: (loc, v) => { if (loc >= 0) gl.uniform1f(uniforms.items[loc], v); },
    uniform2f: (loc, a, b) => { if (loc >= 0) gl.uniform2f(uniforms.items[loc], a, b); },
    uniform3f: (loc, a, b, c) => { if (loc >= 0) gl.uniform3f(uniforms.items[loc], a, b, c); },
    uniform4f: (loc, a, b, c, d) => { if (loc >= 0) gl.uniform4f(uniforms.items[loc], a, b, c, d); },
    uniform_matrix4: (loc, ptr) => { if (loc >= 0) gl.uniformMatrix4fv(uniforms.items[loc], false, new Float32Array(memory.buffer, ptr, 16)); },
    create_buffer: () => put(buffers, gl.createBuffer()),
    bind_buffer: (target, b) => gl.bindBuffer(target, buffers.items[b]),
    buffer_data: (target, ptr, len, usage) => gl.bufferData(target, bytes(ptr, len), usage),
    buffer_sub_data: (target, offset, ptr, len) => gl.bufferSubData(target, offset, bytes(ptr, len)),
    delete_buffer: (b) => { gl.deleteBuffer(buffers.items[b]); drop(buffers, b); },
    create_vertex_array: () => put(vaos, gl.createVertexArray()),
    bind_vertex_array: (v) => gl.bindVertexArray(vaos.items[v]),
    delete_vertex_array: (v) => { gl.deleteVertexArray(vaos.items[v]); drop(vaos, v); },
    enable_vertex_attrib_array: (i) => gl.enableVertexAttribArray(i),
    vertex_attrib_pointer: (i, size, type, normalized, stride, offset) => gl.vertexAttribPointer(i, size, type, !!normalized, stride, offset),
    create_texture: () => put(textures, gl.createTexture()),
    bind_texture: (target, t) => gl.bindTexture(target, textures.items[t]),
    active_texture: (unit) => gl.activeTexture(unit),
    delete_texture: (t) => { gl.deleteTexture(textures.items[t]); drop(textures, t); },
    tex_parameteri: (target, pname, v) => gl.texParameteri(target, pname, v),
    tex_image_2d: (target, level, internal, w, h, format, type, ptr, len) => gl.texImage2D(target, level, internal, w, h, 0, format, type, ptr ? bytes(ptr, len) : null),
    tex_sub_image_2d: (target, level, x, y, w, h, format, type, ptr, len) => gl.texSubImage2D(target, level, x, y, w, h, format, type, bytes(ptr, len)),
    tex_image_3d: (target, level, internal, w, h, d, format, type, ptr, len) => gl.texImage3D(target, level, internal, w, h, d, 0, format, type, ptr ? bytes(ptr, len) : null),
    tex_sub_image_3d: (target, level, x, y, z, w, h, d, format, type, ptr, len) => gl.texSubImage3D(target, level, x, y, z, w, h, d, format, type, bytes(ptr, len)),
    draw_arrays: (mode, first, count) => gl.drawArrays(mode, first, count),
    draw_elements: (mode, count, type, offset) => gl.drawElements(mode, count, type, offset),
    read_pixels: (x, y, w, h, ptr, len) => gl.readPixels(x, y, w, h, gl.RGBA, gl.UNSIGNED_BYTE, bytes(ptr, len)),
  };

  // ---------------------------------------------------------------- storage
  // Saves live in memory for synchronous access and are mirrored to IndexedDB in the background.
  const store = new Map();
  let db = null;
  let taken = null;
  const openStore = () => new Promise((resolve) => {
    let req;
    try { req = indexedDB.open('rustcraft-redefined', 1); } catch (e) { resolve(); return; }
    req.onupgradeneeded = () => req.result.createObjectStore('files');
    req.onerror = () => resolve();
    req.onsuccess = () => {
      db = req.result;
      const cursor = db.transaction('files').objectStore('files').openCursor();
      cursor.onsuccess = () => { const c = cursor.result; if (c) { store.set(c.key, new Uint8Array(c.value)); c.continue(); } else resolve(); };
      cursor.onerror = () => resolve();
    };
  });
  const persist = (key, value) => {
    if (!db) return;
    try {
      const os = db.transaction('files', 'readwrite').objectStore('files');
      if (value) os.put(value, key); else os.delete(key);
    } catch (e) { console.warn('Could not persist', key, e); }
  };

  let wantGrab = false;
  const tryGrab = () => {
    if (wantGrab && document.pointerLockElement !== canvas) {
      const p = canvas.requestPointerLock();
      if (p && p.catch) p.catch(() => {});
    }
  };

  // Sound: effects arrive as 16-bit samples and are played through Web Audio.  Browsers only
  // allow audio after a click or key press, so the context is created on the first one.
  let audio = null;
  const sounds = new Map();
  const wakeAudio = () => {
    if (audio === null) {
      try { audio = new (window.AudioContext || window.webkitAudioContext)(); } catch (e) { audio = false; }
    }
    if (audio && audio.state === 'suspended') audio.resume().catch(() => {});
  };

  const envImports = {
    js_sound_load: (id, ptr, len, rate) => { sounds.set(id, { data: new Int16Array(memory.buffer, ptr, len).slice(), rate, buffer: null }); },
    js_sound_play: (id, volume, pitch) => {
      const s = sounds.get(id);
      if (!audio || audio.state !== 'running' || !s) return;
      try {
        if (!s.buffer) {
          s.buffer = audio.createBuffer(1, s.data.length, s.rate);
          const ch = s.buffer.getChannelData(0);
          for (let i = 0; i < s.data.length; i++) ch[i] = s.data[i] / 32768;
        }
        const src = audio.createBufferSource();
        src.buffer = s.buffer;
        src.playbackRate.value = pitch;
        const gain = audio.createGain();
        gain.gain.value = volume;
        src.connect(gain);
        gain.connect(audio.destination);
        src.start();
      } catch (e) { /* sound is never worth stopping the game for */ }
    },
    js_log: (ptr, len) => { const msg = text(ptr, len); if (msg.startsWith('PANIC')) fail('The game stopped unexpectedly.\n\n' + msg); else console.log(msg); },
    js_now: () => performance.now(),
    js_grab: (on) => { wantGrab = !!on; if (on) tryGrab(); else if (document.pointerLockElement === canvas) document.exitPointerLock(); },
    js_storage_get: (key, len) => { taken = store.get(text(key, len)) || null; return taken ? taken.length : -1; },
    js_storage_take: (dst) => { if (taken) bytes(dst, taken.length).set(taken); taken = null; },
    js_storage_set: (key, keyLen, ptr, len) => { const k = text(key, keyLen); const v = bytes(ptr, len).slice(); store.set(k, v); persist(k, v); },
    js_storage_remove: (key, len) => { const prefix = text(key, len); for (const k of [...store.keys()]) if (k === prefix || k.startsWith(prefix + '/')) { store.delete(k); persist(k, null); } },
    js_storage_keys: (prefix, len) => { const p = text(prefix, len); taken = new TextEncoder().encode([...store.keys()].filter((k) => k.startsWith(p)).sort().join('\n')); return taken.length; },
    js_download: (name, nameLen, ptr, len) => {
      const a = document.createElement('a');
      a.href = URL.createObjectURL(new Blob([bytes(ptr, len).slice()], { type: 'application/octet-stream' }));
      a.download = text(name, nameLen);
      a.click();
      setTimeout(() => URL.revokeObjectURL(a.href), 5000);
    },
  };

  // ---------------------------------------------------------------- input
  const KEYS = { Space: 32, Slash: 47, Escape: 256, Enter: 257, NumpadEnter: 257, Tab: 258, Backspace: 259, Delete: 261, ArrowRight: 262, ArrowLeft: 263, ArrowDown: 264, ArrowUp: 265,
    ShiftLeft: 340, ShiftRight: 340, ControlLeft: 341, ControlRight: 341, AltLeft: 342, AltRight: 342, Minus: 45, Equal: 61, Comma: 44, Period: 46 };
  const keyCode = (e) => {
    const c = e.code;
    if (c.startsWith('Key')) return c.charCodeAt(3);
    if (c.startsWith('Digit')) return 48 + Number(c[5]);
    if (/^F\d+$/.test(c)) return 289 + Number(c.slice(1));
    return KEYS[c] || 0;
  };
  const dpr = () => window.devicePixelRatio || 1;
  const resize = () => {
    const w = Math.max(1, Math.round(canvas.clientWidth * dpr())), h = Math.max(1, Math.round(canvas.clientHeight * dpr()));
    if (canvas.width !== w || canvas.height !== h) { canvas.width = w; canvas.height = h; if (wasm) wasm.rc_resize(w, h); }
  };
  const hookInput = () => {
    window.addEventListener('resize', resize);
    window.addEventListener('keydown', (e) => {
      const code = keyCode(e);
      const locked = document.pointerLockElement === canvas;
      if (code && !(e.ctrlKey && code === 82) && code !== 300 && code !== 301) e.preventDefault();
      wakeAudio();
      if (code) wasm.rc_key(code, 1);
      if (e.key.length === 1 && !e.ctrlKey && !e.metaKey) wasm.rc_char(e.key.codePointAt(0));
      if (!locked) tryGrab();
    });
    window.addEventListener('keyup', (e) => { const code = keyCode(e); if (code) wasm.rc_key(code, 0); });
    canvas.addEventListener('mousedown', (e) => { wakeAudio(); canvas.focus(); wasm.rc_mouse_button([0, 2, 1][e.button] ?? 0, 1); tryGrab(); e.preventDefault(); });
    window.addEventListener('mouseup', (e) => wasm.rc_mouse_button([0, 2, 1][e.button] ?? 0, 0));
    window.addEventListener('mousemove', (e) => {
      if (document.pointerLockElement === canvas) wasm.rc_mouse_move(e.movementX, e.movementY);
      else { const r = canvas.getBoundingClientRect(); wasm.rc_mouse_pos((e.clientX - r.left) * dpr(), (e.clientY - r.top) * dpr()); }
    });
    canvas.addEventListener('wheel', (e) => { wasm.rc_wheel(Math.sign(e.deltaY)); e.preventDefault(); }, { passive: false });
    canvas.addEventListener('contextmenu', (e) => e.preventDefault());
    document.addEventListener('pointerlockchange', () => wasm.rc_grab(document.pointerLockElement === canvas ? 1 : 0));
    // Closing or hiding the tab is the last reliable moment to write the world to IndexedDB.
    document.addEventListener('visibilitychange', () => { if (document.visibilityState === 'hidden') wasm.rc_save(); });
    window.addEventListener('pagehide', () => wasm.rc_save());
    window.addEventListener('blur', () => wasm.rc_focus(0));
    window.addEventListener('focus', () => wasm.rc_focus(1));
  };

  // ---------------------------------------------------------------- start-up
  const wasmBytes = () => {
    if (typeof RUSTCRAFT_WASM !== 'string') throw new Error('rustcraft_wasm.js is missing. Build it with tools/build-web.sh.');
    const bin = atob(RUSTCRAFT_WASM);
    const out = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
    return out;
  };

  const start = async () => {
    await openStore();
    const { instance } = await WebAssembly.instantiate(wasmBytes(), { gl: glImports, env: envImports });
    wasm = instance.exports;
    memory = wasm.memory;
    canvas.width = Math.max(1, Math.round(canvas.clientWidth * dpr()));
    canvas.height = Math.max(1, Math.round(canvas.clientHeight * dpr()));
    wasm.rc_init(canvas.width, canvas.height);
    hookInput();
    status.style.display = 'none';
    const api = {
      exports: wasm,
      frames: 0,
      // Runs a chat command without the leading slash, e.g. rustcraft.command('time set noon').
      command: (line) => { const data = new TextEncoder().encode(line); const p = wasm.rc_alloc(data.length); bytes(p, data.length).set(data); wasm.rc_command(p, data.length); wasm.rc_free(p, data.length); },
    };
    window.rustcraft = api;
    const loop = (t) => {
      try { wasm.rc_frame(t); } catch (e) { fail('The game stopped unexpectedly.\n\n' + e); throw e; }
      api.frames++;
      requestAnimationFrame(loop);
    };
    requestAnimationFrame(loop);
  };
  start().catch((e) => fail('Could not start the game.\n\n' + e));
})();
