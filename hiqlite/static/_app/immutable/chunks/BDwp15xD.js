var __defProp = Object.defineProperty;
var __typeError = (msg) => {
  throw TypeError(msg);
};
var __defNormalProp = (obj, key, value) => key in obj ? __defProp(obj, key, { enumerable: true, configurable: true, writable: true, value }) : obj[key] = value;
var __publicField = (obj, key, value) => __defNormalProp(obj, typeof key !== "symbol" ? key + "" : key, value);
var __accessCheck = (obj, member, msg) => member.has(obj) || __typeError("Cannot " + msg);
var __privateGet = (obj, member, getter) => (__accessCheck(obj, member, "read from private field"), getter ? getter.call(obj) : member.get(obj));
var __privateAdd = (obj, member, value) => member.has(obj) ? __typeError("Cannot add the same private member more than once") : member instanceof WeakSet ? member.add(obj) : member.set(obj, value);
var __privateSet = (obj, member, value, setter) => (__accessCheck(obj, member, "write to private field"), setter ? setter.call(obj, value) : member.set(obj, value), value);
var __privateMethod = (obj, member, method) => (__accessCheck(obj, member, "access private method"), method);
var _r2, _s, _t2, _n2, _a, _f, _i, _U_instances, o_fn, e_fn, l_fn, u_fn;
var Kt = Array.isArray, zt = Array.prototype.indexOf, jn = Array.from, qn = Object.defineProperty, le = Object.getOwnPropertyDescriptor, $t = Object.getOwnPropertyDescriptors, Xt = Object.prototype, Zt = Array.prototype, lt = Object.getPrototypeOf, tt = Object.isExtensible;
function Yn(e) {
  return typeof e == "function";
}
const ye = () => {
};
function Hn(e) {
  return e();
}
function Wt(e) {
  for (var t = 0; t < e.length; t++) e[t]();
}
function ut() {
  var e, t, n = new Promise((r, s) => {
    e = r, t = s;
  });
  return { promise: n, resolve: e, reject: t };
}
const b = 2, Re = 4, Se = 8, F = 16, L = 32, te = 64, He = 128, O = 512, m = 1024, T = 2048, j = 4096, D = 8192, $ = 16384, Ue = 32768, be = 65536, Pe = 1 << 17, ct = 1 << 18, de = 1 << 19, _t = 1 << 20, ce = 32768, Ce = 1 << 21, Be = 1 << 22, Y = 1 << 23, X = Symbol("$state"), Un = Symbol("legacy props"), Bn = Symbol(""), re = new class extends Error {
  constructor() {
    super(...arguments);
    __publicField(this, "name", "StaleReactionError");
    __publicField(this, "message", "The reaction that called `getAbortSignal()` was re-run or destroyed");
  }
}(), Ve = 3, vt = 8;
function dt(e) {
  throw new Error("https://svelte.dev/e/lifecycle_outside_component");
}
function Jt() {
  throw new Error("https://svelte.dev/e/async_derived_orphan");
}
function Qt(e) {
  throw new Error("https://svelte.dev/e/effect_in_teardown");
}
function en() {
  throw new Error("https://svelte.dev/e/effect_in_unowned_derived");
}
function tn(e) {
  throw new Error("https://svelte.dev/e/effect_orphan");
}
function nn() {
  throw new Error("https://svelte.dev/e/effect_update_depth_exceeded");
}
function rn() {
  throw new Error("https://svelte.dev/e/experimental_async_fork");
}
function sn() {
  throw new Error("https://svelte.dev/e/fork_discarded");
}
function fn() {
  throw new Error("https://svelte.dev/e/fork_timing");
}
function Gn() {
  throw new Error("https://svelte.dev/e/hydration_failed");
}
function Kn(e) {
  throw new Error("https://svelte.dev/e/props_invalid_value");
}
function an() {
  throw new Error("https://svelte.dev/e/state_descriptors_fixed");
}
function on() {
  throw new Error("https://svelte.dev/e/state_prototype_fixed");
}
function ln() {
  throw new Error("https://svelte.dev/e/state_unsafe_mutation");
}
function zn() {
  throw new Error("https://svelte.dev/e/svelte_boundary_reset_onerror");
}
const $n = 1, Xn = 2, Zn = 4, Wn = 8, Jn = 16, Qn = 1, er = 2, tr = 4, nr = 8, rr = 16, sr = 1, fr = 2, ir = 4, ar = 1, or = 2, un = "[", cn = "[!", _n = "]", Ge = {}, E = Symbol(), lr = "http://www.w3.org/1999/xhtml", ur = "@attach";
function Ke(e) {
  console.warn("https://svelte.dev/e/hydration_mismatch");
}
function cr() {
  console.warn("https://svelte.dev/e/select_multiple_invalid_value");
}
function _r() {
  console.warn("https://svelte.dev/e/svelte_boundary_reset_noop");
}
let J = false;
function vr(e) {
  J = e;
}
let R;
function se(e) {
  if (e === null) throw Ke(), Ge;
  return R = e;
}
function dr() {
  return se(V(R));
}
function hr(e) {
  if (J) {
    if (V(R) !== null) throw Ke(), Ge;
    R = e;
  }
}
function pr(e = 1) {
  if (J) {
    for (var t = e, n = R; t--; ) n = V(n);
    R = n;
  }
}
function wr(e = true) {
  for (var t = 0, n = R; ; ) {
    if (n.nodeType === vt) {
      var r = n.data;
      if (r === _n) {
        if (t === 0) return n;
        t -= 1;
      } else (r === un || r === cn) && (t += 1);
    }
    var s = V(n);
    e && n.remove(), n = s;
  }
}
function yr(e) {
  if (!e || e.nodeType !== vt) throw Ke(), Ge;
  return e.data;
}
function ht(e) {
  return e === this.v;
}
function pt(e, t) {
  return e != e ? t == t : e !== t || e !== null && typeof e == "object" || typeof e == "function";
}
function wt(e) {
  return !pt(e, this.v);
}
let he = false;
function br() {
  he = true;
}
let p = null;
function Ee(e) {
  p = e;
}
function Er(e) {
  return ze().get(e);
}
function mr(e, t) {
  return ze().set(e, t), t;
}
function gr(e) {
  return ze().has(e);
}
function Tr(e, t = false, n) {
  p = { p, i: false, c: null, e: null, s: e, x: null, l: he && !t ? { s: null, u: null, $: [] } : null };
}
function Ar(e) {
  var t = p, n = t.e;
  if (n !== null) {
    t.e = null;
    for (var r of n) Dt(r);
  }
  return t.i = true, p = t.p, {};
}
function pe() {
  return !he || p !== null && p.l === null;
}
function ze(e) {
  return p === null && dt(), p.c ?? (p.c = new Map(vn(p) || void 0));
}
function vn(e) {
  let t = e.p;
  for (; t !== null; ) {
    const n = t.c;
    if (n !== null) return n;
    t = t.p;
  }
  return null;
}
let K = [];
function yt() {
  var e = K;
  K = [], Wt(e);
}
function bt(e) {
  if (K.length === 0 && !ue) {
    var t = K;
    queueMicrotask(() => {
      t === K && yt();
    });
  }
  K.push(e);
}
function dn() {
  for (; K.length > 0; ) yt();
}
function hn(e) {
  var t = h;
  if (t === null) return _.f |= Y, e;
  if ((t.f & Ue) === 0) {
    if ((t.f & He) === 0) throw e;
    t.b.error(e);
  } else me(e, t);
}
function me(e, t) {
  for (; t !== null; ) {
    if ((t.f & He) !== 0) try {
      t.b.error(e);
      return;
    } catch (n) {
      e = n;
    }
    t = t.parent;
  }
  throw e;
}
const z = /* @__PURE__ */ new Set();
let w = null, Ie = null, x = null, N = [], ke = null, Me = false, ue = false;
const _U = class _U {
  constructor() {
    __privateAdd(this, _U_instances);
    __publicField(this, "committed", false);
    __publicField(this, "current", /* @__PURE__ */ new Map());
    __publicField(this, "previous", /* @__PURE__ */ new Map());
    __privateAdd(this, _r2, /* @__PURE__ */ new Set());
    __privateAdd(this, _s, /* @__PURE__ */ new Set());
    __privateAdd(this, _t2, 0);
    __privateAdd(this, _n2, 0);
    __privateAdd(this, _a, null);
    __privateAdd(this, _f, []);
    __privateAdd(this, _i, []);
    __publicField(this, "skipped_effects", /* @__PURE__ */ new Set());
    __publicField(this, "is_fork", false);
  }
  process(t) {
    var _a2;
    N = [], Ie = null, this.apply();
    var n = { parent: null, effect: null, effects: [], render_effects: [], block_effects: [] };
    for (const r of t) __privateMethod(this, _U_instances, o_fn).call(this, r, n);
    this.is_fork || __privateMethod(this, _U_instances, l_fn).call(this), __privateGet(this, _n2) > 0 || this.is_fork ? (__privateMethod(this, _U_instances, e_fn).call(this, n.effects), __privateMethod(this, _U_instances, e_fn).call(this, n.render_effects), __privateMethod(this, _U_instances, e_fn).call(this, n.block_effects)) : (Ie = this, w = null, nt(n.render_effects), nt(n.effects), Ie = null, (_a2 = __privateGet(this, _a)) == null ? void 0 : _a2.resolve()), x = null;
  }
  capture(t, n) {
    this.previous.has(t) || this.previous.set(t, n), (t.f & Y) === 0 && (this.current.set(t, t.v), x == null ? void 0 : x.set(t, t.v));
  }
  activate() {
    w = this, this.apply();
  }
  deactivate() {
    w === this && (w = null, x = null);
  }
  flush() {
    if (this.activate(), N.length > 0) {
      if (Le(), w !== null && w !== this) return;
    } else __privateGet(this, _t2) === 0 && this.process([]);
    this.deactivate();
  }
  discard() {
    for (const t of __privateGet(this, _s)) t(this);
    __privateGet(this, _s).clear();
  }
  increment(t) {
    __privateSet(this, _t2, __privateGet(this, _t2) + 1), t && __privateSet(this, _n2, __privateGet(this, _n2) + 1);
  }
  decrement(t) {
    __privateSet(this, _t2, __privateGet(this, _t2) - 1), t && __privateSet(this, _n2, __privateGet(this, _n2) - 1), this.revive();
  }
  revive() {
    for (const t of __privateGet(this, _f)) y(t, T), Q(t);
    for (const t of __privateGet(this, _i)) y(t, j), Q(t);
    __privateSet(this, _f, []), __privateSet(this, _i, []), this.flush();
  }
  oncommit(t) {
    __privateGet(this, _r2).add(t);
  }
  ondiscard(t) {
    __privateGet(this, _s).add(t);
  }
  settled() {
    return (__privateGet(this, _a) ?? __privateSet(this, _a, ut())).promise;
  }
  static ensure() {
    if (w === null) {
      const t = w = new _U();
      z.add(w), ue || _U.enqueue(() => {
        w === t && t.flush();
      });
    }
    return w;
  }
  static enqueue(t) {
    bt(t);
  }
  apply() {
  }
};
_r2 = new WeakMap();
_s = new WeakMap();
_t2 = new WeakMap();
_n2 = new WeakMap();
_a = new WeakMap();
_f = new WeakMap();
_i = new WeakMap();
_U_instances = new WeakSet();
o_fn = function(t, n) {
  var _a2;
  t.f ^= m;
  for (var r = t.first; r !== null; ) {
    var s = r.f, f = (s & (L | te)) !== 0, o = f && (s & m) !== 0, l = o || (s & D) !== 0 || this.skipped_effects.has(r);
    if ((r.f & He) !== 0 && ((_a2 = r.b) == null ? void 0 : _a2.is_pending()) && (n = { parent: n, effect: r, effects: [], render_effects: [], block_effects: [] }), !l && r.fn !== null) {
      f ? r.f ^= m : (s & Re) !== 0 ? n.effects.push(r) : we(r) && ((r.f & F) !== 0 && n.block_effects.push(r), ve(r));
      var a = r.first;
      if (a !== null) {
        r = a;
        continue;
      }
    }
    var i = r.parent;
    for (r = r.next; r === null && i !== null; ) i === n.effect && (__privateMethod(this, _U_instances, e_fn).call(this, n.effects), __privateMethod(this, _U_instances, e_fn).call(this, n.render_effects), __privateMethod(this, _U_instances, e_fn).call(this, n.block_effects), n = n.parent), r = i.next, i = i.parent;
  }
};
e_fn = function(t) {
  for (const n of t) ((n.f & T) !== 0 ? __privateGet(this, _f) : __privateGet(this, _i)).push(n), y(n, m);
};
l_fn = function() {
  if (__privateGet(this, _n2) === 0) {
    for (const t of __privateGet(this, _r2)) t();
    __privateGet(this, _r2).clear();
  }
  __privateGet(this, _t2) === 0 && __privateMethod(this, _U_instances, u_fn).call(this);
};
u_fn = function() {
  var _a2;
  if (z.size > 1) {
    this.previous.clear();
    var t = x, n = true, r = { parent: null, effect: null, effects: [], render_effects: [], block_effects: [] };
    for (const s of z) {
      if (s === this) {
        n = false;
        continue;
      }
      const f = [];
      for (const [l, a] of this.current) {
        if (s.current.has(l)) if (n && a !== s.current.get(l)) s.current.set(l, a);
        else continue;
        f.push(l);
      }
      if (f.length === 0) continue;
      const o = [...s.current.keys()].filter((l) => !this.current.has(l));
      if (o.length > 0) {
        const l = /* @__PURE__ */ new Set(), a = /* @__PURE__ */ new Map();
        for (const i of f) Et(i, o, l, a);
        if (N.length > 0) {
          w = s, s.apply();
          for (const i of N) __privateMethod(_a2 = s, _U_instances, o_fn).call(_a2, i, r);
          N = [], s.deactivate();
        }
      }
    }
    w = null, x = t;
  }
  this.committed = true, z.delete(this);
};
let U = _U;
function Fe(e) {
  var t = ue;
  ue = true;
  try {
    var n;
    for (e && (w !== null && Le(), n = e()); ; ) {
      if (dn(), N.length === 0 && (w == null ? void 0 : w.flush(), N.length === 0)) return ke = null, n;
      Le();
    }
  } finally {
    ue = t;
  }
}
function Le() {
  var e = Z;
  Me = true;
  try {
    var t = 0;
    for (it(true); N.length > 0; ) {
      var n = U.ensure();
      if (t++ > 1e3) {
        var r, s;
        pn();
      }
      n.process(N), H.clear();
    }
  } finally {
    Me = false, it(e), ke = null;
  }
}
function pn() {
  try {
    nn();
  } catch (e) {
    me(e, ke);
  }
}
let C = null;
function nt(e) {
  var t = e.length;
  if (t !== 0) {
    for (var n = 0; n < t; ) {
      var r = e[n++];
      if ((r.f & ($ | D)) === 0 && we(r) && (C = /* @__PURE__ */ new Set(), ve(r), r.deps === null && r.first === null && r.nodes_start === null && (r.teardown === null && r.ac === null ? Mt(r) : r.fn = null), (C == null ? void 0 : C.size) > 0)) {
        H.clear();
        for (const s of C) {
          if ((s.f & ($ | D)) !== 0) continue;
          const f = [s];
          let o = s.parent;
          for (; o !== null; ) C.has(o) && (C.delete(o), f.push(o)), o = o.parent;
          for (let l = f.length - 1; l >= 0; l--) {
            const a = f[l];
            (a.f & ($ | D)) === 0 && ve(a);
          }
        }
        C.clear();
      }
    }
    C = null;
  }
}
function Et(e, t, n, r) {
  if (!n.has(e) && (n.add(e), e.reactions !== null)) for (const s of e.reactions) {
    const f = s.f;
    (f & b) !== 0 ? Et(s, t, n, r) : (f & (Be | F)) !== 0 && (f & T) === 0 && gt(s, t, r) && (y(s, T), Q(s));
  }
}
function mt(e, t) {
  if (e.reactions !== null) for (const n of e.reactions) {
    const r = n.f;
    (r & b) !== 0 ? mt(n, t) : (r & Pe) !== 0 && (y(n, T), t.add(n));
  }
}
function gt(e, t, n) {
  const r = n.get(e);
  if (r !== void 0) return r;
  if (e.deps !== null) for (const s of e.deps) {
    if (t.includes(s)) return true;
    if ((s.f & b) !== 0 && gt(s, t, n)) return n.set(s, true), true;
  }
  return n.set(e, false), false;
}
function Q(e) {
  for (var t = ke = e; t.parent !== null; ) {
    t = t.parent;
    var n = t.f;
    if (Me && t === h && (n & F) !== 0 && (n & ct) === 0) return;
    if ((n & (te | L)) !== 0) {
      if ((n & m) === 0) return;
      t.f ^= m;
    }
  }
  N.push(t);
}
function xr(e) {
  rn(), w !== null && fn();
  var t = U.ensure();
  t.is_fork = true;
  var n = false, r = t.settled();
  Fe(e);
  for (var [s, f] of t.previous) s.v = f;
  return { commit: async () => {
    if (n) {
      await r;
      return;
    }
    z.has(t) || sn(), n = true, t.is_fork = false;
    for (var [o, l] of t.current) o.v = l;
    Fe(() => {
      var a = /* @__PURE__ */ new Set();
      for (var i of t.current.keys()) mt(i, a);
      gn(a), Rt();
    }), t.revive(), await r;
  }, discard: () => {
    !n && z.has(t) && (z.delete(t), t.discard());
  } };
}
function wn(e, t, n, r) {
  const s = pe() ? $e : En;
  if (n.length === 0 && e.length === 0) {
    r(t.map(s));
    return;
  }
  var f = w, o = h, l = yn();
  function a() {
    Promise.all(n.map((i) => bn(i))).then((i) => {
      l();
      try {
        r([...t.map(s), ...i]);
      } catch (u) {
        (o.f & $) === 0 && me(u, o);
      }
      f == null ? void 0 : f.deactivate(), ge();
    }).catch((i) => {
      me(i, o);
    });
  }
  e.length > 0 ? Promise.all(e).then(() => {
    l();
    try {
      return a();
    } finally {
      f == null ? void 0 : f.deactivate(), ge();
    }
  }) : a();
}
function yn() {
  var e = h, t = _, n = p, r = w;
  return function(f = true) {
    fe(e), B(t), Ee(n), f && (r == null ? void 0 : r.activate());
  };
}
function ge() {
  fe(null), B(null), Ee(null);
}
function $e(e) {
  var t = b | T, n = _ !== null && (_.f & b) !== 0 ? _ : null;
  return h !== null && (h.f |= de), { ctx: p, deps: null, effects: null, equals: ht, f: t, fn: e, reactions: null, rv: 0, v: E, wv: 0, parent: n ?? h, ac: null };
}
function bn(e, t) {
  let n = h;
  n === null && Jt();
  var r = n.b, s = void 0, f = Ze(E), o = !_, l = /* @__PURE__ */ new Map();
  return kn(() => {
    var _a2;
    var a = ut();
    s = a.promise;
    try {
      Promise.resolve(e()).then(a.resolve, a.reject).then(() => {
        i === w && i.committed && i.deactivate(), ge();
      });
    } catch (c) {
      a.reject(c), ge();
    }
    var i = w;
    if (o) {
      var u = !r.is_pending();
      r.update_pending_count(1), i.increment(u), (_a2 = l.get(i)) == null ? void 0 : _a2.reject(re), l.delete(i), l.set(i, a);
    }
    const v = (c, d = void 0) => {
      if (i.activate(), d) d !== re && (f.f |= Y, je(f, d));
      else {
        (f.f & Y) !== 0 && (f.f ^= Y), je(f, c);
        for (const [k, Oe] of l) {
          if (l.delete(k), k === i) break;
          Oe.reject(re);
        }
      }
      o && (r.update_pending_count(-1), i.decrement(u));
    };
    a.promise.then(v, (c) => v(null, c || "unknown"));
  }), It(() => {
    for (const a of l.values()) a.reject(re);
  }), new Promise((a) => {
    function i(u) {
      function v() {
        u === s ? a(f) : i(s);
      }
      u.then(v, v);
    }
    i(s);
  });
}
function Rr(e) {
  const t = $e(e);
  return jt(t), t;
}
function En(e) {
  const t = $e(e);
  return t.equals = wt, t;
}
function Tt(e) {
  var t = e.effects;
  if (t !== null) {
    e.effects = null;
    for (var n = 0; n < t.length; n += 1) ee(t[n]);
  }
}
function mn(e) {
  for (var t = e.parent; t !== null; ) {
    if ((t.f & b) === 0) return t;
    t = t.parent;
  }
  return null;
}
function Xe(e) {
  var t, n = h;
  fe(mn(e));
  try {
    e.f &= ~ce, Tt(e), t = Ut(e);
  } finally {
    fe(n);
  }
  return t;
}
function At(e) {
  var t = Xe(e);
  if (e.equals(t) || (e.v = t, e.wv = Yt()), !ie) if (x !== null) Je() && x.set(e, e.v);
  else {
    var n = (e.f & O) === 0 ? j : m;
    y(e, n);
  }
}
let Te = /* @__PURE__ */ new Set();
const H = /* @__PURE__ */ new Map();
function gn(e) {
  Te = e;
}
let xt = false;
function Ze(e, t) {
  var n = { f: 0, v: e, reactions: null, equals: ht, rv: 0, wv: 0 };
  return n;
}
function q(e, t) {
  const n = Ze(e);
  return jt(n), n;
}
function Sr(e, t = false, n = true) {
  var _a2;
  const r = Ze(e);
  return t || (r.equals = wt), he && n && p !== null && p.l !== null && ((_a2 = p.l).s ?? (_a2.s = [])).push(r), r;
}
function G(e, t, n = false) {
  _ !== null && (!I || (_.f & Pe) !== 0) && pe() && (_.f & (b | F | Be | Pe)) !== 0 && !(M == null ? void 0 : M.includes(e)) && ln();
  let r = n ? ae(t) : t;
  return je(e, r);
}
function je(e, t) {
  if (!e.equals(t)) {
    var n = e.v;
    ie ? H.set(e, t) : H.set(e, n), e.v = t;
    var r = U.ensure();
    r.capture(e, n), (e.f & b) !== 0 && ((e.f & T) !== 0 && Xe(e), y(e, (e.f & O) !== 0 ? m : j)), e.wv = Yt(), St(e, T), pe() && h !== null && (h.f & m) !== 0 && (h.f & (L | te)) === 0 && (S === null ? Pn([e]) : S.push(e)), !r.is_fork && Te.size > 0 && !xt && Rt();
  }
  return t;
}
function Rt() {
  xt = false;
  const e = Array.from(Te);
  for (const t of e) (t.f & m) !== 0 && y(t, j), we(t) && ve(t);
  Te.clear();
}
function De(e) {
  G(e, e.v + 1);
}
function St(e, t) {
  var n = e.reactions;
  if (n !== null) for (var r = pe(), s = n.length, f = 0; f < s; f++) {
    var o = n[f], l = o.f;
    if (!(!r && o === h)) {
      var a = (l & T) === 0;
      if (a && y(o, t), (l & b) !== 0) {
        var i = o;
        x == null ? void 0 : x.delete(i), (l & ce) === 0 && (l & O && (o.f |= ce), St(i, j));
      } else a && ((l & F) !== 0 && C !== null && C.add(o), Q(o));
    }
  }
}
function ae(e) {
  if (typeof e != "object" || e === null || X in e) return e;
  const t = lt(e);
  if (t !== Xt && t !== Zt) return e;
  var n = /* @__PURE__ */ new Map(), r = Kt(e), s = q(0), f = W, o = (l) => {
    if (W === f) return l();
    var a = _, i = W;
    B(null), ot(f);
    var u = l();
    return B(a), ot(i), u;
  };
  return r && n.set("length", q(e.length)), new Proxy(e, { defineProperty(l, a, i) {
    (!("value" in i) || i.configurable === false || i.enumerable === false || i.writable === false) && an();
    var u = n.get(a);
    return u === void 0 ? u = o(() => {
      var v = q(i.value);
      return n.set(a, v), v;
    }) : G(u, i.value, true), true;
  }, deleteProperty(l, a) {
    var i = n.get(a);
    if (i === void 0) {
      if (a in l) {
        const u = o(() => q(E));
        n.set(a, u), De(s);
      }
    } else G(i, E), De(s);
    return true;
  }, get(l, a, i) {
    var _a2;
    if (a === X) return e;
    var u = n.get(a), v = a in l;
    if (u === void 0 && (!v || ((_a2 = le(l, a)) == null ? void 0 : _a2.writable)) && (u = o(() => {
      var d = ae(v ? l[a] : E), k = q(d);
      return k;
    }), n.set(a, u)), u !== void 0) {
      var c = oe(u);
      return c === E ? void 0 : c;
    }
    return Reflect.get(l, a, i);
  }, getOwnPropertyDescriptor(l, a) {
    var i = Reflect.getOwnPropertyDescriptor(l, a);
    if (i && "value" in i) {
      var u = n.get(a);
      u && (i.value = oe(u));
    } else if (i === void 0) {
      var v = n.get(a), c = v == null ? void 0 : v.v;
      if (v !== void 0 && c !== E) return { enumerable: true, configurable: true, value: c, writable: true };
    }
    return i;
  }, has(l, a) {
    var _a2;
    if (a === X) return true;
    var i = n.get(a), u = i !== void 0 && i.v !== E || Reflect.has(l, a);
    if (i !== void 0 || h !== null && (!u || ((_a2 = le(l, a)) == null ? void 0 : _a2.writable))) {
      i === void 0 && (i = o(() => {
        var c = u ? ae(l[a]) : E, d = q(c);
        return d;
      }), n.set(a, i));
      var v = oe(i);
      if (v === E) return false;
    }
    return u;
  }, set(l, a, i, u) {
    var _a2;
    var v = n.get(a), c = a in l;
    if (r && a === "length") for (var d = i; d < v.v; d += 1) {
      var k = n.get(d + "");
      k !== void 0 ? G(k, E) : d in l && (k = o(() => q(E)), n.set(d + "", k));
    }
    if (v === void 0) (!c || ((_a2 = le(l, a)) == null ? void 0 : _a2.writable)) && (v = o(() => q(void 0)), G(v, ae(i)), n.set(a, v));
    else {
      c = v.v !== E;
      var Oe = o(() => ae(i));
      G(v, Oe);
    }
    var Qe = Reflect.getOwnPropertyDescriptor(l, a);
    if ((Qe == null ? void 0 : Qe.set) && Qe.set.call(u, i), !c) {
      if (r && typeof a == "string") {
        var et = n.get("length"), Ne = Number(a);
        Number.isInteger(Ne) && Ne >= et.v && G(et, Ne + 1);
      }
      De(s);
    }
    return true;
  }, ownKeys(l) {
    oe(s);
    var a = Reflect.ownKeys(l).filter((v) => {
      var c = n.get(v);
      return c === void 0 || c.v !== E;
    });
    for (var [i, u] of n) u.v !== E && !(i in l) && a.push(i);
    return a;
  }, setPrototypeOf() {
    on();
  } });
}
function rt(e) {
  try {
    if (e !== null && typeof e == "object" && X in e) return e[X];
  } catch {
  }
  return e;
}
function kr(e, t) {
  return Object.is(rt(e), rt(t));
}
var st, Tn, An, kt, Ot;
function Or() {
  if (st === void 0) {
    st = window, Tn = document, An = /Firefox/.test(navigator.userAgent);
    var e = Element.prototype, t = Node.prototype, n = Text.prototype;
    kt = le(t, "firstChild").get, Ot = le(t, "nextSibling").get, tt(e) && (e.__click = void 0, e.__className = void 0, e.__attributes = null, e.__style = void 0, e.__e = void 0), tt(n) && (n.__t = void 0);
  }
}
function Ae(e = "") {
  return document.createTextNode(e);
}
function qe(e) {
  return kt.call(e);
}
function V(e) {
  return Ot.call(e);
}
function Nr(e, t) {
  if (!J) return qe(e);
  var n = qe(R);
  if (n === null) n = R.appendChild(Ae());
  else if (t && n.nodeType !== Ve) {
    var r = Ae();
    return n == null ? void 0 : n.before(r), se(r), r;
  }
  return se(n), n;
}
function Ir(e, t = false) {
  if (!J) {
    var n = qe(e);
    return n instanceof Comment && n.data === "" ? V(n) : n;
  }
  if (t && (R == null ? void 0 : R.nodeType) !== Ve) {
    var r = Ae();
    return R == null ? void 0 : R.before(r), se(r), r;
  }
  return R;
}
function Dr(e, t = 1, n = false) {
  let r = J ? R : e;
  for (var s; t--; ) s = r, r = V(r);
  if (!J) return r;
  if (n && (r == null ? void 0 : r.nodeType) !== Ve) {
    var f = Ae();
    return r === null ? s == null ? void 0 : s.after(f) : r.before(f), se(f), f;
  }
  return se(r), r;
}
function Pr(e) {
  e.textContent = "";
}
function Cr() {
  return false;
}
function Mr(e, t) {
  if (t) {
    const n = document.body;
    e.autofocus = true, bt(() => {
      document.activeElement === n && e.focus();
    });
  }
}
let ft = false;
function xn() {
  ft || (ft = true, document.addEventListener("reset", (e) => {
    Promise.resolve().then(() => {
      var _a2;
      if (!e.defaultPrevented) for (const t of e.target.elements) (_a2 = t.__on_r) == null ? void 0 : _a2.call(t);
    });
  }, { capture: true }));
}
function Fr(e, t, n, r = true) {
  r && n();
  for (var s of t) e.addEventListener(s, n);
  It(() => {
    for (var f of t) e.removeEventListener(f, n);
  });
}
function We(e) {
  var t = _, n = h;
  B(null), fe(null);
  try {
    return e();
  } finally {
    B(t), fe(n);
  }
}
function Lr(e, t, n, r = n) {
  e.addEventListener(t, () => We(n));
  const s = e.__on_r;
  s ? e.__on_r = () => {
    s(), r(true);
  } : e.__on_r = () => r(true), xn();
}
function Nt(e) {
  h === null && (_ === null && tn(), en()), ie && Qt();
}
function Rn(e, t) {
  var n = t.last;
  n === null ? t.last = t.first = e : (n.next = e, e.prev = n, t.last = e);
}
function P(e, t, n, r = true) {
  var s = h;
  s !== null && (s.f & D) !== 0 && (e |= D);
  var f = { ctx: p, deps: null, nodes_start: null, nodes_end: null, f: e | T | O, first: null, fn: t, last: null, next: null, parent: s, b: s && s.b, prev: null, teardown: null, transitions: null, wv: 0, ac: null };
  if (n) try {
    ve(f), f.f |= Ue;
  } catch (a) {
    throw ee(f), a;
  }
  else t !== null && Q(f);
  if (r) {
    var o = f;
    if (n && o.deps === null && o.teardown === null && o.nodes_start === null && o.first === o.last && (o.f & de) === 0 && (o = o.first, (e & F) !== 0 && (e & be) !== 0 && o !== null && (o.f |= be)), o !== null && (o.parent = s, s !== null && Rn(o, s), _ !== null && (_.f & b) !== 0 && (e & te) === 0)) {
      var l = _;
      (l.effects ?? (l.effects = [])).push(o);
    }
  }
  return f;
}
function Je() {
  return _ !== null && !I;
}
function It(e) {
  const t = P(Se, null, false);
  return y(t, m), t.teardown = e, t;
}
function Sn(e) {
  Nt();
  var t = h.f, n = !_ && (t & L) !== 0 && (t & Ue) === 0;
  if (n) {
    var r = p;
    (r.e ?? (r.e = [])).push(e);
  } else return Dt(e);
}
function Dt(e) {
  return P(Re | _t, e, false);
}
function jr(e) {
  return Nt(), P(Se | _t, e, true);
}
function qr(e) {
  U.ensure();
  const t = P(te | de, e, true);
  return (n = {}) => new Promise((r) => {
    n.outro ? In(t, () => {
      ee(t), r(void 0);
    }) : (ee(t), r(void 0));
  });
}
function Yr(e) {
  return P(Re, e, false);
}
function kn(e) {
  return P(Be | de, e, true);
}
function Hr(e, t = 0) {
  return P(Se | t, e, true);
}
function Ur(e, t = [], n = [], r = [], s = false) {
  wn(r, t, n, (f) => {
    P(s ? Re : Se, () => e(...f.map(oe)), true);
  });
}
function Br(e, t = 0) {
  var n = P(F | t, e, true);
  return n;
}
function Vr(e, t = true) {
  return P(L | de, e, true, t);
}
function Pt(e) {
  var t = e.teardown;
  if (t !== null) {
    const n = ie, r = _;
    at(true), B(null);
    try {
      t.call(null);
    } finally {
      at(n), B(r);
    }
  }
}
function Ct(e, t = false) {
  var n = e.first;
  for (e.first = e.last = null; n !== null; ) {
    const s = n.ac;
    s !== null && We(() => {
      s.abort(re);
    });
    var r = n.next;
    (n.f & te) !== 0 ? n.parent = null : ee(n, t), n = r;
  }
}
function On(e) {
  for (var t = e.first; t !== null; ) {
    var n = t.next;
    (t.f & L) === 0 && ee(t), t = n;
  }
}
function ee(e, t = true) {
  var n = false;
  (t || (e.f & ct) !== 0) && e.nodes_start !== null && e.nodes_end !== null && (Nn(e.nodes_start, e.nodes_end), n = true), Ct(e, t && !n), xe(e, 0), y(e, $);
  var r = e.transitions;
  if (r !== null) for (const f of r) f.stop();
  Pt(e);
  var s = e.parent;
  s !== null && s.first !== null && Mt(e), e.next = e.prev = e.teardown = e.ctx = e.deps = e.fn = e.nodes_start = e.nodes_end = e.ac = null;
}
function Nn(e, t) {
  for (; e !== null; ) {
    var n = e === t ? null : V(e);
    e.remove(), e = n;
  }
}
function Mt(e) {
  var t = e.parent, n = e.prev, r = e.next;
  n !== null && (n.next = r), r !== null && (r.prev = n), t !== null && (t.first === e && (t.first = r), t.last === e && (t.last = n));
}
function In(e, t, n = true) {
  var r = [];
  Ft(e, r, true), Dn(r, () => {
    n && ee(e), t && t();
  });
}
function Dn(e, t) {
  var n = e.length;
  if (n > 0) {
    var r = () => --n || t();
    for (var s of e) s.out(r);
  } else t();
}
function Ft(e, t, n) {
  if ((e.f & D) === 0) {
    if (e.f ^= D, e.transitions !== null) for (const o of e.transitions) (o.is_global || n) && t.push(o);
    for (var r = e.first; r !== null; ) {
      var s = r.next, f = (r.f & be) !== 0 || (r.f & L) !== 0 && (e.f & F) !== 0;
      Ft(r, t, f ? n : false), r = s;
    }
  }
}
function Gr(e) {
  Lt(e, true);
}
function Lt(e, t) {
  if ((e.f & D) !== 0) {
    e.f ^= D, (e.f & m) === 0 && (y(e, T), Q(e));
    for (var n = e.first; n !== null; ) {
      var r = n.next, s = (n.f & be) !== 0 || (n.f & L) !== 0;
      Lt(n, s ? t : false), n = r;
    }
    if (e.transitions !== null) for (const f of e.transitions) (f.is_global || t) && f.in();
  }
}
function Kr(e, t) {
  for (var n = e.nodes_start, r = e.nodes_end; n !== null; ) {
    var s = n === r ? null : V(n);
    t.append(n), n = s;
  }
}
let Z = false;
function it(e) {
  Z = e;
}
let ie = false;
function at(e) {
  ie = e;
}
let _ = null, I = false;
function B(e) {
  _ = e;
}
let h = null;
function fe(e) {
  h = e;
}
let M = null;
function jt(e) {
  _ !== null && (M === null ? M = [e] : M.push(e));
}
let g = null, A = 0, S = null;
function Pn(e) {
  S = e;
}
let qt = 1, _e = 0, W = _e;
function ot(e) {
  W = e;
}
function Yt() {
  return ++qt;
}
function we(e) {
  var t = e.f;
  if ((t & T) !== 0) return true;
  if (t & b && (e.f &= ~ce), (t & j) !== 0) {
    var n = e.deps;
    if (n !== null) for (var r = n.length, s = 0; s < r; s++) {
      var f = n[s];
      if (we(f) && At(f), f.wv > e.wv) return true;
    }
    (t & O) !== 0 && x === null && y(e, m);
  }
  return false;
}
function Ht(e, t, n = true) {
  var r = e.reactions;
  if (r !== null && !(M == null ? void 0 : M.includes(e))) for (var s = 0; s < r.length; s++) {
    var f = r[s];
    (f.f & b) !== 0 ? Ht(f, t, false) : t === f && (n ? y(f, T) : (f.f & m) !== 0 && y(f, j), Q(f));
  }
}
function Ut(e) {
  var _a2;
  var t = g, n = A, r = S, s = _, f = M, o = p, l = I, a = W, i = e.f;
  g = null, A = 0, S = null, _ = (i & (L | te)) === 0 ? e : null, M = null, Ee(e.ctx), I = false, W = ++_e, e.ac !== null && (We(() => {
    e.ac.abort(re);
  }), e.ac = null);
  try {
    e.f |= Ce;
    var u = e.fn, v = u(), c = e.deps;
    if (g !== null) {
      var d;
      if (xe(e, A), c !== null && A > 0) for (c.length = A + g.length, d = 0; d < g.length; d++) c[A + d] = g[d];
      else e.deps = c = g;
      if (Z && Je() && (e.f & O) !== 0) for (d = A; d < c.length; d++) ((_a2 = c[d]).reactions ?? (_a2.reactions = [])).push(e);
    } else c !== null && A < c.length && (xe(e, A), c.length = A);
    if (pe() && S !== null && !I && c !== null && (e.f & (b | j | T)) === 0) for (d = 0; d < S.length; d++) Ht(S[d], e);
    return s !== null && s !== e && (_e++, S !== null && (r === null ? r = S : r.push(...S))), (e.f & Y) !== 0 && (e.f ^= Y), v;
  } catch (k) {
    return hn(k);
  } finally {
    e.f ^= Ce, g = t, A = n, S = r, _ = s, M = f, Ee(o), I = l, W = a;
  }
}
function Cn(e, t) {
  let n = t.reactions;
  if (n !== null) {
    var r = zt.call(n, e);
    if (r !== -1) {
      var s = n.length - 1;
      s === 0 ? n = t.reactions = null : (n[r] = n[s], n.pop());
    }
  }
  n === null && (t.f & b) !== 0 && (g === null || !g.includes(t)) && (y(t, j), (t.f & O) !== 0 && (t.f ^= O, t.f &= ~ce), Tt(t), xe(t, 0));
}
function xe(e, t) {
  var n = e.deps;
  if (n !== null) for (var r = t; r < n.length; r++) Cn(e, n[r]);
}
function ve(e) {
  var t = e.f;
  if ((t & $) === 0) {
    y(e, m);
    var n = h, r = Z;
    h = e, Z = true;
    try {
      (t & F) !== 0 ? On(e) : Ct(e), Pt(e);
      var s = Ut(e);
      e.teardown = typeof s == "function" ? s : null, e.wv = qt;
      var f;
    } finally {
      Z = r, h = n;
    }
  }
}
async function zr() {
  await Promise.resolve(), Fe();
}
function $r() {
  return U.ensure().settled();
}
function oe(e) {
  var t = e.f, n = (t & b) !== 0;
  if (_ !== null && !I) {
    var r = h !== null && (h.f & $) !== 0;
    if (!r && !(M == null ? void 0 : M.includes(e))) {
      var s = _.deps;
      if ((_.f & Ce) !== 0) e.rv < _e && (e.rv = _e, g === null && s !== null && s[A] === e ? A++ : g === null ? g = [e] : g.includes(e) || g.push(e));
      else {
        (_.deps ?? (_.deps = [])).push(e);
        var f = e.reactions;
        f === null ? e.reactions = [_] : f.includes(_) || f.push(_);
      }
    }
  }
  if (ie) {
    if (H.has(e)) return H.get(e);
    if (n) {
      var o = e, l = o.v;
      return ((o.f & m) === 0 && o.reactions !== null || Vt(o)) && (l = Xe(o)), H.set(o, l), l;
    }
  } else if (n) {
    if (o = e, x == null ? void 0 : x.has(o)) return x.get(o);
    we(o) && At(o), Z && Je() && (o.f & O) === 0 && Bt(o);
  } else if (x == null ? void 0 : x.has(e)) return x.get(e);
  if ((e.f & Y) !== 0) throw e.v;
  return e.v;
}
function Bt(e) {
  if (e.deps !== null) {
    e.f ^= O;
    for (const t of e.deps) (t.reactions ?? (t.reactions = [])).push(e), (t.f & b) !== 0 && (t.f & O) === 0 && Bt(t);
  }
}
function Vt(e) {
  if (e.v === E) return true;
  if (e.deps === null) return false;
  for (const t of e.deps) if (H.has(t) || (t.f & b) !== 0 && Vt(t)) return true;
  return false;
}
function Gt(e) {
  var t = I;
  try {
    return I = true, e();
  } finally {
    I = t;
  }
}
const Mn = -7169;
function y(e, t) {
  e.f = e.f & Mn | t;
}
function Xr(e) {
  if (!(typeof e != "object" || !e || e instanceof EventTarget)) {
    if (X in e) Ye(e);
    else if (!Array.isArray(e)) for (let t in e) {
      const n = e[t];
      typeof n == "object" && n && X in n && Ye(n);
    }
  }
}
function Ye(e, t = /* @__PURE__ */ new Set()) {
  if (typeof e == "object" && e !== null && !(e instanceof EventTarget) && !t.has(e)) {
    t.add(e), e instanceof Date && e.getTime();
    for (let r in e) try {
      Ye(e[r], t);
    } catch {
    }
    const n = lt(e);
    if (n !== Object.prototype && n !== Array.prototype && n !== Map.prototype && n !== Set.prototype && n !== Date.prototype) {
      const r = $t(n);
      for (let s in r) {
        const f = r[s].get;
        if (f) try {
          f.call(e);
        } catch {
        }
      }
    }
  }
}
function Fn(e, t, n) {
  if (e == null) return t(void 0), ye;
  const r = Gt(() => e.subscribe(t, n));
  return r.unsubscribe ? () => r.unsubscribe() : r;
}
const ne = [];
function Zr(e, t = ye) {
  let n = null;
  const r = /* @__PURE__ */ new Set();
  function s(l) {
    if (pt(e, l) && (e = l, n)) {
      const a = !ne.length;
      for (const i of r) i[1](), ne.push(i, e);
      if (a) {
        for (let i = 0; i < ne.length; i += 2) ne[i][0](ne[i + 1]);
        ne.length = 0;
      }
    }
  }
  function f(l) {
    s(l(e));
  }
  function o(l, a = ye) {
    const i = [l, a];
    return r.add(i), r.size === 1 && (n = t(s, f) || ye), l(e), () => {
      r.delete(i), r.size === 0 && n && (n(), n = null);
    };
  }
  return { set: s, update: f, subscribe: o };
}
function Wr(e) {
  let t;
  return Fn(e, (n) => t = n)(), t;
}
function Jr(e) {
  p === null && dt(), he && p.l !== null ? Ln(p).m.push(e) : Sn(() => {
    const t = Gt(e);
    if (typeof t == "function") return t;
  });
}
function Ln(e) {
  var t = e.l;
  return t.u ?? (t.u = { a: [], b: [], m: [] });
}
export {
  $,
  Cr as A,
  Br as B,
  dr as C,
  yr as D,
  be as E,
  wr as F,
  se as G,
  cn as H,
  vr as I,
  Yr as J,
  Hr as K,
  bt as L,
  ye as M,
  Sr as N,
  Fn as O,
  Wr as P,
  It as Q,
  qn as R,
  X as S,
  G as T,
  le as U,
  Kn as V,
  tr as W,
  En as X,
  ae as Y,
  ie as Z,
  h as _,
  Sn as a,
  $t as a$,
  nr as a0,
  he as a1,
  er as a2,
  Qn as a3,
  rr as a4,
  Un as a5,
  Fe as a6,
  Jr as a7,
  q as a8,
  zr as a9,
  un as aA,
  V as aB,
  Ge as aC,
  Gn as aD,
  Pr as aE,
  jn as aF,
  qr as aG,
  _n as aH,
  Ke as aI,
  Kt as aJ,
  Xn as aK,
  $n as aL,
  Jn as aM,
  D as aN,
  Ft as aO,
  Dn as aP,
  Zn as aQ,
  Wn as aR,
  ct as aS,
  cr as aT,
  kr as aU,
  Lr as aV,
  Ie as aW,
  wn as aX,
  lr as aY,
  lt as aZ,
  ur as a_,
  Rr as aa,
  Je as ab,
  Ze as ac,
  De as ad,
  vt as ae,
  U as af,
  fe as ag,
  B as ah,
  Ee as ai,
  hn as aj,
  _ as ak,
  je as al,
  pr as am,
  me as an,
  zn as ao,
  de as ap,
  He as aq,
  _r as ar,
  We as as,
  qe as at,
  An as au,
  ar as av,
  or as aw,
  Ue as ax,
  Ve as ay,
  Or as az,
  Gt as b,
  Mr as b0,
  E as b1,
  xn as b2,
  Bn as b3,
  F as b4,
  ir as b5,
  sr as b6,
  fr as b7,
  Yn as b8,
  Zr as b9,
  Tn as ba,
  gr as bb,
  Er as bc,
  mr as bd,
  Fr as be,
  xr as bf,
  $r as bg,
  p as c,
  Hn as d,
  br as e,
  Xr as f,
  oe as g,
  $e as h,
  Ir as i,
  Ar as j,
  Nr as k,
  hr as l,
  w as m,
  Gr as n,
  ee as o,
  Tr as p,
  In as q,
  Wt as r,
  Dr as s,
  Ur as t,
  jr as u,
  Ae as v,
  Vr as w,
  J as x,
  R as y,
  Kr as z
};
