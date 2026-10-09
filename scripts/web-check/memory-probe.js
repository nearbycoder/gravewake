// Injected before the page's own scripts by mobile.mjs: it records what
// the game allocates, for the memory report. `window.__probe` holds the
// WebAssembly memory, every live WebGL2 and WebGPU buffer and texture (by
// estimated size), and what the game asked WebGPU for.
(() => {
  const probe = {
    memory: null,
    wasmCompiled: 0,
    gl: new Map(),
    gpu: new Map(),
    gpuLimits: null,
    adapterLimits: null,
  };
  window.__probe = probe;
  const keep = (instance) => {
    const memory = instance && instance.exports && instance.exports.memory;
    if (memory) probe.memory = memory;
  };
  for (const name of ["instantiate", "instantiateStreaming"]) {
    const original = WebAssembly[name];
    if (!original) continue;
    WebAssembly[name] = async function (...args) {
      const result = await original.apply(this, args);
      keep(result.instance || result);
      if (result.module) probe.wasmCompiled++;
      return result;
    };
  }
  const OriginalInstance = WebAssembly.Instance;
  WebAssembly.Instance = function (...args) {
    const instance = new OriginalInstance(...args);
    keep(instance);
    return instance;
  };
  WebAssembly.Instance.prototype = OriginalInstance.prototype;
  probe.wasmBytes = () => (probe.memory ? probe.memory.buffer.byteLength : 0);

  // WebGL2: bytes per pixel of the internal formats wgpu's GL backend uses.
  const bpp = {
    0x8058: 4, 0x8c43: 4, 0x8059: 4, 0x8d70: 16, 0x881a: 8, 0x8814: 16, 0x822e: 4,
    0x822d: 2, 0x8229: 1, 0x822b: 2, 0x8cac: 4, 0x88f0: 4, 0x81a5: 2, 0x81a6: 4,
    0x8c3a: 4, 0x8d62: 2, 0x8cad: 8, 0x8c41: 3, 0x8051: 3, 0x8d7c: 4, 0x8d8e: 4,
    0x8d76: 8, 0x8d88: 8, 0x8235: 16, 0x8236: 16, 0x8234: 8, 0x8233: 8,
  };
  const glBound = new WeakMap();
  const bound = (gl) => {
    if (!glBound.has(gl)) glBound.set(gl, { buffers: {}, textures: {}, renderbuffer: null, unit: 0 });
    return glBound.get(gl);
  };
  const mipBytes = (levels, w, h, d, perPixel) => {
    let total = 0;
    for (let i = 0; i < levels; i++) {
      total += Math.max(1, w >> i) * Math.max(1, h >> i) * d * perPixel;
    }
    return total;
  };
  const wrap = (proto, name, after) => {
    const original = proto[name];
    if (!original) return;
    proto[name] = function (...args) {
      const result = original.apply(this, args);
      try { after.call(this, args, result); } catch (_) { /* never break the page */ }
      return result;
    };
  };
  for (const Context of [window.WebGL2RenderingContext].filter(Boolean)) {
    const p = Context.prototype;
    wrap(p, "activeTexture", function ([unit]) { bound(this).unit = unit; });
    wrap(p, "bindBuffer", function ([target, buffer]) { bound(this).buffers[target] = buffer; });
    wrap(p, "bindTexture", function ([target, texture]) { bound(this).textures[`${bound(this).unit}:${target}`] = texture; });
    wrap(p, "bindRenderbuffer", function ([, rb]) { bound(this).renderbuffer = rb; });
    wrap(p, "bufferData", function ([target, data]) {
      const buffer = bound(this).buffers[target];
      const size = typeof data === "number" ? data : data ? data.byteLength : 0;
      if (buffer) probe.gl.set(buffer, { bytes: size, label: `buffer ${target.toString(16)}` });
    });
    wrap(p, "texStorage2D", function ([target, levels, format, w, h]) {
      const texture = bound(this).textures[`${bound(this).unit}:${target}`];
      const faces = target === 0x8513 ? 6 : 1;
      if (texture) probe.gl.set(texture, { bytes: mipBytes(levels, w, h, faces, bpp[format] || 4), label: `texture ${w}x${h} ${levels} levels ${format.toString(16)}` });
    });
    wrap(p, "texStorage3D", function ([target, levels, format, w, h, d]) {
      const texture = bound(this).textures[`${bound(this).unit}:${target}`];
      if (texture) probe.gl.set(texture, { bytes: mipBytes(levels, w, h, d, bpp[format] || 4), label: `texture ${w}x${h}x${d} ${levels} levels ${format.toString(16)}` });
    });
    wrap(p, "texImage2D", function (args) {
      if (args.length < 8) return;
      const [target, level, format, w, h] = args;
      const texture = bound(this).textures[`${bound(this).unit}:${target}`];
      if (texture && level === 0) probe.gl.set(texture, { bytes: w * h * (bpp[format] || 4), label: `texture ${w}x${h} ${format.toString(16)}` });
    });
    wrap(p, "renderbufferStorage", function ([, format, w, h]) {
      const rb = bound(this).renderbuffer;
      if (rb) probe.gl.set(rb, { bytes: w * h * (bpp[format] || 4), label: `renderbuffer ${w}x${h}` });
    });
    wrap(p, "renderbufferStorageMultisample", function ([, samples, format, w, h]) {
      const rb = bound(this).renderbuffer;
      if (rb) probe.gl.set(rb, { bytes: w * h * Math.max(1, samples) * (bpp[format] || 4), label: `renderbuffer ${w}x${h}x${samples}` });
    });
    for (const name of ["deleteBuffer", "deleteTexture", "deleteRenderbuffer"]) {
      wrap(p, name, function ([object]) { probe.gl.delete(object); });
    }
  }

  // WebGPU.
  const gpuBpp = (format) => (/32float|32uint|32sint/.test(format) ? (/rgba/.test(format) ? 16 : /rg/.test(format) ? 8 : 4)
    : /16float|16uint|16sint/.test(format) ? (/rgba/.test(format) ? 8 : /rg/.test(format) ? 4 : 2)
    : /depth24plus-stencil8|depth32float-stencil8/.test(format) ? 5 : /^r8|^rg8/.test(format) ? (/rg/.test(format) ? 2 : 1) : 4);
  const note = (object, label, bytes) => {
    probe.gpu.set(object, { bytes, label: label || "(unlabelled)" });
  };
  if (window.GPUDevice) {
    const p = GPUDevice.prototype;
    wrap(p, "createBuffer", function ([d], buffer) { note(buffer, d.label, d.size); });
    wrap(p, "createTexture", function ([d], texture) {
      const size = Array.isArray(d.size) ? { width: d.size[0], height: d.size[1] || 1, depthOrArrayLayers: d.size[2] || 1 } : d.size;
      note(texture, d.label, mipBytes(d.mipLevelCount || 1, size.width, size.height || 1, size.depthOrArrayLayers || 1, gpuBpp(d.format)) * (d.sampleCount || 1));
    });
    wrap(GPUBuffer.prototype, "destroy", function () { probe.gpu.delete(this); });
    wrap(GPUTexture.prototype, "destroy", function () { probe.gpu.delete(this); });
  }
  if (window.GPUAdapter) {
    const original = GPUAdapter.prototype.requestDevice;
    GPUAdapter.prototype.requestDevice = function (descriptor) {
      const limits = {};
      for (const key in this.limits) limits[key] = this.limits[key];
      probe.adapterLimits = limits;
      probe.gpuLimits = descriptor && descriptor.requiredLimits ? { ...descriptor.requiredLimits } : null;
      return original.call(this, descriptor);
    };
  }
  const sum = (map) => { let t = 0; for (const v of map.values()) t += v.bytes; return t; };
  // Live objects grouped by kind and size, largest total first.
  probe.groups = (n = 12) => {
    const groups = new Map();
    for (const o of [...probe.gl.values(), ...probe.gpu.values()]) {
      const key = `${(o.bytes / 1048576).toFixed(1)} MB ${o.label}`;
      const g = groups.get(key) || { key, count: 0, bytes: 0 };
      g.count++;
      g.bytes += o.bytes;
      groups.set(key, g);
    }
    return [...groups.values()].sort((a, b) => b.bytes - a.bytes).slice(0, n)
      .map((g) => `${g.count} × ${g.key}`);
  };
  // The largest live objects, for the report.
  probe.top = (n = 15) => [...probe.gl.values(), ...probe.gpu.values()]
    .sort((a, b) => b.bytes - a.bytes).slice(0, n)
    .map((o) => `${(o.bytes / 1048576).toFixed(1)} MB ${o.label}`);
  probe.snapshot = () => ({
    wasm: probe.wasmBytes(),
    gl: sum(probe.gl),
    gpu: sum(probe.gpu),
    js: performance.memory ? performance.memory.usedJSHeapSize : null,
    state: window.gravewake ? window.gravewake.state : null,
  });
})();
