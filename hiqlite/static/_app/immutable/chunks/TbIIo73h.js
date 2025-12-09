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
var _r2, _s, _t2, _n2, _a, _f, _i, _B_instances, o_fn, e_fn, l_fn, u_fn, c_fn;
var zt = Array.isArray, $t = Array.prototype.indexOf, jn = Array.from, qn = Object.defineProperty, ue = Object.getOwnPropertyDescriptor, Xt = Object.getOwnPropertyDescriptors, Zt = Object.prototype, Wt = Array.prototype, lt = Object.getPrototypeOf, nt = Object.isExtensible;
function Yn(e) {
  return typeof e == "function";
}
const ye = () => {
};
function Hn(e) {
  return e();
}
function Jt(e) {
  for (var t = 0; t < e.length; t++) e[t]();
}
function ut() {
  var e, t, n = new Promise((r, s) => {
    e = r, t = s;
  });
  return { promise: n, resolve: e, reject: t };
}
const y = 2, Ue = 4, Se = 8, ct = 1 << 24, F = 16, L = 32, ne = 64, Be = 128, N = 512, m = 1024, A = 2048, j = 4096, D = 8192, Y = 16384, Ve = 32768, Ee = 65536, Pe = 1 << 17, _t = 1 << 18, de = 1 << 19, vt = 1 << 20, Un = 1 << 25, J = 32768, Me = 1 << 21, Ge = 1 << 22, H = 1 << 23, X = Symbol("$state"), Bn = Symbol("legacy props"), Vn = Symbol(""), se = new class extends Error {
  constructor() {
    super(...arguments);
    __publicField(this, "name", "StaleReactionError");
    __publicField(this, "message", "The reaction that called `getAbortSignal()` was re-run or destroyed");
  }
}(), Ke = 3, dt = 8;
function Qt(e) {
  throw new Error("https://svelte.dev/e/experimental_async_required");
}
function ht(e) {
  throw new Error("https://svelte.dev/e/lifecycle_outside_component");
}
function en() {
  throw new Error("https://svelte.dev/e/async_derived_orphan");
}
function tn(e) {
  throw new Error("https://svelte.dev/e/effect_in_teardown");
}
function nn() {
  throw new Error("https://svelte.dev/e/effect_in_unowned_derived");
}
function rn(e) {
  throw new Error("https://svelte.dev/e/effect_orphan");
}
function sn() {
  throw new Error("https://svelte.dev/e/effect_update_depth_exceeded");
}
function fn() {
  throw new Error("https://svelte.dev/e/fork_discarded");
}
function an() {
  throw new Error("https://svelte.dev/e/fork_timing");
}
function Kn() {
  throw new Error("https://svelte.dev/e/hydration_failed");
}
function zn(e) {
  throw new Error("https://svelte.dev/e/props_invalid_value");
}
function on() {
  throw new Error("https://svelte.dev/e/state_descriptors_fixed");
}
function ln() {
  throw new Error("https://svelte.dev/e/state_prototype_fixed");
}
function un() {
  throw new Error("https://svelte.dev/e/state_unsafe_mutation");
}
function $n() {
  throw new Error("https://svelte.dev/e/svelte_boundary_reset_onerror");
}
const Xn = 1, Zn = 2, Wn = 4, Jn = 8, Qn = 16, er = 1, tr = 2, nr = 4, rr = 8, sr = 16, fr = 1, ir = 2, ar = 4, or = 1, lr = 2, cn = "[", _n = "[!", vn = "]", ze = {}, b = Symbol(), ur = "http://www.w3.org/1999/xhtml", cr = "@attach";
function $e(e) {
  console.warn("https://svelte.dev/e/hydration_mismatch");
}
function _r() {
  console.warn("https://svelte.dev/e/select_multiple_invalid_value");
}
function vr() {
  console.warn("https://svelte.dev/e/svelte_boundary_reset_noop");
}
let Q = false;
function dr(e) {
  Q = e;
}
let k;
function fe(e) {
  if (e === null) throw $e(), ze;
  return k = e;
}
function hr() {
  return fe(G(k));
}
function pr(e) {
  if (Q) {
    if (G(k) !== null) throw $e(), ze;
    k = e;
  }
}
function wr(e = 1) {
  if (Q) {
    for (var t = e, n = k; t--; ) n = G(n);
    k = n;
  }
}
function yr(e = true) {
  for (var t = 0, n = k; ; ) {
    if (n.nodeType === dt) {
      var r = n.data;
      if (r === vn) {
        if (t === 0) return n;
        t -= 1;
      } else (r === cn || r === _n) && (t += 1);
    }
    var s = G(n);
    e && n.remove(), n = s;
  }
}
function Er(e) {
  if (!e || e.nodeType !== dt) throw $e(), ze;
  return e.data;
}
function pt(e) {
  return e === this.v;
}
function wt(e, t) {
  return e != e ? t == t : e !== t || e !== null && typeof e == "object" || typeof e == "function";
}
function yt(e) {
  return !wt(e, this.v);
}
let he = false;
function br() {
  he = true;
}
let w = null;
function be(e) {
  w = e;
}
function mr(e) {
  return Xe().get(e);
}
function gr(e, t) {
  return Xe().set(e, t), t;
}
function Tr(e) {
  return Xe().has(e);
}
function Ar(e, t = false, n) {
  w = { p: w, i: false, c: null, e: null, s: e, x: null, l: he && !t ? { s: null, u: null, $: [] } : null };
}
function xr(e) {
  var t = w, n = t.e;
  if (n !== null) {
    t.e = null;
    for (var r of n) Dt(r);
  }
  return t.i = true, w = t.p, {};
}
function pe() {
  return !he || w !== null && w.l === null;
}
function Xe(e) {
  return w === null && ht(), w.c ?? (w.c = new Map(dn(w) || void 0));
}
function dn(e) {
  let t = e.p;
  for (; t !== null; ) {
    const n = t.c;
    if (n !== null) return n;
    t = t.p;
  }
  return null;
}
let z = [];
function Et() {
  var e = z;
  z = [], Jt(e);
}
function bt(e) {
  if (z.length === 0 && !ce) {
    var t = z;
    queueMicrotask(() => {
      t === z && Et();
    });
  }
  z.push(e);
}
function hn() {
  for (; z.length > 0; ) Et();
}
function pn(e) {
  var t = h;
  if (t === null) return _.f |= H, e;
  if ((t.f & Ve) === 0) {
    if ((t.f & Be) === 0) throw e;
    t.b.error(e);
  } else me(e, t);
}
function me(e, t) {
  for (; t !== null; ) {
    if ((t.f & Be) !== 0) try {
      t.b.error(e);
      return;
    } catch (n) {
      e = n;
    }
    t = t.parent;
  }
  throw e;
}
const $ = /* @__PURE__ */ new Set();
let p = null, Ce = null, T = null, S = [], Oe = null, Fe = false, ce = false;
const _B = class _B {
  constructor() {
    __privateAdd(this, _B_instances);
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
  is_deferred() {
    return this.is_fork || __privateGet(this, _n2) > 0;
  }
  process(t) {
    var _a2;
    S = [], Ce = null, this.apply();
    var n = { parent: null, effect: null, effects: [], render_effects: [], block_effects: [] };
    for (const r of t) __privateMethod(this, _B_instances, o_fn).call(this, r, n);
    this.is_fork || __privateMethod(this, _B_instances, u_fn).call(this), this.is_deferred() ? (__privateMethod(this, _B_instances, e_fn).call(this, n.effects), __privateMethod(this, _B_instances, e_fn).call(this, n.render_effects), __privateMethod(this, _B_instances, e_fn).call(this, n.block_effects)) : (Ce = this, p = null, rt(n.render_effects), rt(n.effects), Ce = null, (_a2 = __privateGet(this, _a)) == null ? void 0 : _a2.resolve()), T = null;
  }
  capture(t, n) {
    this.previous.has(t) || this.previous.set(t, n), (t.f & H) === 0 && (this.current.set(t, t.v), T == null ? void 0 : T.set(t, t.v));
  }
  activate() {
    p = this, this.apply();
  }
  deactivate() {
    p === this && (p = null, T = null);
  }
  flush() {
    if (this.activate(), S.length > 0) {
      if (je(), p !== null && p !== this) return;
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
    for (const t of __privateGet(this, _f)) E(t, A), ee(t);
    for (const t of __privateGet(this, _i)) E(t, j), ee(t);
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
    if (p === null) {
      const t = p = new _B();
      $.add(p), ce || _B.enqueue(() => {
        p === t && t.flush();
      });
    }
    return p;
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
_B_instances = new WeakSet();
o_fn = function(t, n) {
  var _a2;
  t.f ^= m;
  for (var r = t.first; r !== null; ) {
    var s = r.f, f = (s & (L | ne)) !== 0, l = f && (s & m) !== 0, o = l || (s & D) !== 0 || this.skipped_effects.has(r);
    if ((r.f & Be) !== 0 && ((_a2 = r.b) == null ? void 0 : _a2.is_pending()) && (n = { parent: n, effect: r, effects: [], render_effects: [], block_effects: [] }), !o && r.fn !== null) {
      f ? r.f ^= m : (s & Ue) !== 0 ? n.effects.push(r) : we(r) && ((r.f & F) !== 0 && n.block_effects.push(r), ve(r));
      var a = r.first;
      if (a !== null) {
        r = a;
        continue;
      }
    }
    var i = r.parent;
    for (r = r.next; r === null && i !== null; ) i === n.effect && (__privateMethod(this, _B_instances, e_fn).call(this, n.effects), __privateMethod(this, _B_instances, e_fn).call(this, n.render_effects), __privateMethod(this, _B_instances, e_fn).call(this, n.block_effects), n = n.parent), r = i.next, i = i.parent;
  }
};
e_fn = function(t) {
  for (const n of t) ((n.f & A) !== 0 ? __privateGet(this, _f) : __privateGet(this, _i)).push(n), __privateMethod(this, _B_instances, l_fn).call(this, n.deps), E(n, m);
};
l_fn = function(t) {
  if (t !== null) for (const n of t) (n.f & y) === 0 || (n.f & J) === 0 || (n.f ^= J, __privateMethod(this, _B_instances, l_fn).call(this, n.deps));
};
u_fn = function() {
  if (__privateGet(this, _n2) === 0) {
    for (const t of __privateGet(this, _r2)) t();
    __privateGet(this, _r2).clear();
  }
  __privateGet(this, _t2) === 0 && __privateMethod(this, _B_instances, c_fn).call(this);
};
c_fn = function() {
  var _a2;
  if ($.size > 1) {
    this.previous.clear();
    var t = T, n = true, r = { parent: null, effect: null, effects: [], render_effects: [], block_effects: [] };
    for (const f of $) {
      if (f === this) {
        n = false;
        continue;
      }
      const l = [];
      for (const [a, i] of this.current) {
        if (f.current.has(a)) if (n && i !== f.current.get(a)) f.current.set(a, i);
        else continue;
        l.push(a);
      }
      if (l.length === 0) continue;
      const o = [...f.current.keys()].filter((a) => !this.current.has(a));
      if (o.length > 0) {
        var s = S;
        S = [];
        const a = /* @__PURE__ */ new Set(), i = /* @__PURE__ */ new Map();
        for (const u of l) mt(u, o, a, i);
        if (S.length > 0) {
          p = f, f.apply();
          for (const u of S) __privateMethod(_a2 = f, _B_instances, o_fn).call(_a2, u, r);
          f.deactivate();
        }
        S = s;
      }
    }
    p = null, T = t;
  }
  this.committed = true, $.delete(this);
};
let B = _B;
function Le(e) {
  var t = ce;
  ce = true;
  try {
    var n;
    for (e && (p !== null && je(), n = e()); ; ) {
      if (hn(), S.length === 0 && (p == null ? void 0 : p.flush(), S.length === 0)) return Oe = null, n;
      je();
    }
  } finally {
    ce = t;
  }
}
function je() {
  var e = Z;
  Fe = true;
  var t = null;
  try {
    var n = 0;
    for (ke(true); S.length > 0; ) {
      var r = B.ensure();
      if (n++ > 1e3) {
        var s, f;
        wn();
      }
      r.process(S), U.clear();
    }
  } finally {
    Fe = false, ke(e), Oe = null;
  }
}
function wn() {
  try {
    sn();
  } catch (e) {
    me(e, Oe);
  }
}
let P = null;
function rt(e) {
  var t = e.length;
  if (t !== 0) {
    for (var n = 0; n < t; ) {
      var r = e[n++];
      if ((r.f & (Y | D)) === 0 && we(r) && (P = /* @__PURE__ */ new Set(), ve(r), r.deps === null && r.first === null && r.nodes === null && (r.teardown === null && r.ac === null ? Ft(r) : r.fn = null), (P == null ? void 0 : P.size) > 0)) {
        U.clear();
        for (const s of P) {
          if ((s.f & (Y | D)) !== 0) continue;
          const f = [s];
          let l = s.parent;
          for (; l !== null; ) P.has(l) && (P.delete(l), f.push(l)), l = l.parent;
          for (let o = f.length - 1; o >= 0; o--) {
            const a = f[o];
            (a.f & (Y | D)) === 0 && ve(a);
          }
        }
        P.clear();
      }
    }
    P = null;
  }
}
function mt(e, t, n, r) {
  if (!n.has(e) && (n.add(e), e.reactions !== null)) for (const s of e.reactions) {
    const f = s.f;
    (f & y) !== 0 ? mt(s, t, n, r) : (f & (Ge | F)) !== 0 && (f & A) === 0 && Tt(s, t, r) && (E(s, A), ee(s));
  }
}
function gt(e, t) {
  if (e.reactions !== null) for (const n of e.reactions) {
    const r = n.f;
    (r & y) !== 0 ? gt(n, t) : (r & Pe) !== 0 && (E(n, A), t.add(n));
  }
}
function Tt(e, t, n) {
  const r = n.get(e);
  if (r !== void 0) return r;
  if (e.deps !== null) for (const s of e.deps) {
    if (t.includes(s)) return true;
    if ((s.f & y) !== 0 && Tt(s, t, n)) return n.set(s, true), true;
  }
  return n.set(e, false), false;
}
function ee(e) {
  for (var t = Oe = e; t.parent !== null; ) {
    t = t.parent;
    var n = t.f;
    if (Fe && t === h && (n & F) !== 0 && (n & _t) === 0) return;
    if ((n & (ne | L)) !== 0) {
      if ((n & m) === 0) return;
      t.f ^= m;
    }
  }
  S.push(t);
}
function kr(e) {
  Qt(), p !== null && an();
  var t = B.ensure();
  t.is_fork = true, T = /* @__PURE__ */ new Map();
  var n = false, r = t.settled();
  Le(e), T = null;
  for (var [s, f] of t.previous) s.v = f;
  return { commit: async () => {
    if (n) {
      await r;
      return;
    }
    $.has(t) || fn(), n = true, t.is_fork = false;
    for (var [l, o] of t.current) l.v = o;
    Le(() => {
      var a = /* @__PURE__ */ new Set();
      for (var i of t.current.keys()) gt(i, a);
      Tn(a), Rt();
    }), t.revive(), await r;
  }, discard: () => {
    !n && $.has(t) && ($.delete(t), t.discard());
  } };
}
function yn(e, t, n, r) {
  const s = pe() ? Ze : mn;
  if (n.length === 0 && e.length === 0) {
    r(t.map(s));
    return;
  }
  var f = p, l = h, o = En();
  function a() {
    Promise.all(n.map((i) => bn(i))).then((i) => {
      o();
      try {
        r([...t.map(s), ...i]);
      } catch (u) {
        (l.f & Y) === 0 && me(u, l);
      }
      f == null ? void 0 : f.deactivate(), ge();
    }).catch((i) => {
      me(i, l);
    });
  }
  e.length > 0 ? Promise.all(e).then(() => {
    o();
    try {
      return a();
    } finally {
      f == null ? void 0 : f.deactivate(), ge();
    }
  }) : a();
}
function En() {
  var e = h, t = _, n = w, r = p;
  return function(f = true) {
    ie(e), V(t), be(n), f && (r == null ? void 0 : r.activate());
  };
}
function ge() {
  ie(null), V(null), be(null);
}
function Ze(e) {
  var t = y | A, n = _ !== null && (_.f & y) !== 0 ? _ : null;
  return h !== null && (h.f |= de), { ctx: w, deps: null, effects: null, equals: pt, f: t, fn: e, reactions: null, rv: 0, v: b, wv: 0, parent: n ?? h, ac: null };
}
function bn(e, t) {
  let n = h;
  n === null && en();
  var r = n.b, s = void 0, f = Je(b), l = !_, o = /* @__PURE__ */ new Map();
  return On(() => {
    var _a2;
    var a = ut();
    s = a.promise;
    try {
      Promise.resolve(e()).then(a.resolve, a.reject).then(() => {
        i === p && i.committed && i.deactivate(), ge();
      });
    } catch (c) {
      a.reject(c), ge();
    }
    var i = p;
    if (l) {
      var u = !r.is_pending();
      r.update_pending_count(1), i.increment(u), (_a2 = o.get(i)) == null ? void 0 : _a2.reject(se), o.delete(i), o.set(i, a);
    }
    const v = (c, d = void 0) => {
      if (i.activate(), d) d !== se && (f.f |= H, qe(f, d));
      else {
        (f.f & H) !== 0 && (f.f ^= H), qe(f, c);
        for (const [O, Ne] of o) {
          if (o.delete(O), O === i) break;
          Ne.reject(se);
        }
      }
      l && (r.update_pending_count(-1), i.decrement(u));
    };
    a.promise.then(v, (c) => v(null, c || "unknown"));
  }), Ct(() => {
    for (const a of o.values()) a.reject(se);
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
  const t = Ze(e);
  return qt(t), t;
}
function mn(e) {
  const t = Ze(e);
  return t.equals = yt, t;
}
function At(e) {
  var t = e.effects;
  if (t !== null) {
    e.effects = null;
    for (var n = 0; n < t.length; n += 1) te(t[n]);
  }
}
function gn(e) {
  for (var t = e.parent; t !== null; ) {
    if ((t.f & y) === 0) return (t.f & Y) === 0 ? t : null;
    t = t.parent;
  }
  return null;
}
function We(e) {
  var t, n = h;
  ie(gn(e));
  try {
    e.f &= ~J, At(e), t = Bt(e);
  } finally {
    ie(n);
  }
  return t;
}
function xt(e) {
  var t = We(e);
  if (e.equals(t) || ((p == null ? void 0 : p.is_fork) || (e.v = t), e.wv = Ht()), !ae) if (T !== null) (xe() || (p == null ? void 0 : p.is_fork)) && T.set(e, t);
  else {
    var n = (e.f & N) === 0 ? j : m;
    E(e, n);
  }
}
let Te = /* @__PURE__ */ new Set();
const U = /* @__PURE__ */ new Map();
function Tn(e) {
  Te = e;
}
let kt = false;
function Je(e, t) {
  var n = { f: 0, v: e, reactions: null, equals: pt, rv: 0, wv: 0 };
  return n;
}
function q(e, t) {
  const n = Je(e);
  return qt(n), n;
}
function Sr(e, t = false, n = true) {
  var _a2;
  const r = Je(e);
  return t || (r.equals = yt), he && n && w !== null && w.l !== null && ((_a2 = w.l).s ?? (_a2.s = [])).push(r), r;
}
function K(e, t, n = false) {
  _ !== null && (!C || (_.f & Pe) !== 0) && pe() && (_.f & (y | F | Ge | Pe)) !== 0 && !(M == null ? void 0 : M.includes(e)) && un();
  let r = n ? oe(t) : t;
  return qe(e, r);
}
function qe(e, t) {
  if (!e.equals(t)) {
    var n = e.v;
    ae ? U.set(e, t) : U.set(e, n), e.v = t;
    var r = B.ensure();
    r.capture(e, n), (e.f & y) !== 0 && ((e.f & A) !== 0 && We(e), E(e, (e.f & N) !== 0 ? m : j)), e.wv = Ht(), St(e, A), pe() && h !== null && (h.f & m) !== 0 && (h.f & (L | ne)) === 0 && (R === null ? Dn([e]) : R.push(e)), !r.is_fork && Te.size > 0 && !kt && Rt();
  }
  return t;
}
function Rt() {
  kt = false;
  var e = Z;
  ke(true);
  const t = Array.from(Te);
  try {
    for (const n of t) (n.f & m) !== 0 && E(n, j), we(n) && ve(n);
  } finally {
    ke(e);
  }
  Te.clear();
}
function De(e) {
  K(e, e.v + 1);
}
function St(e, t) {
  var n = e.reactions;
  if (n !== null) for (var r = pe(), s = n.length, f = 0; f < s; f++) {
    var l = n[f], o = l.f;
    if (!(!r && l === h)) {
      var a = (o & A) === 0;
      if (a && E(l, t), (o & y) !== 0) {
        var i = l;
        T == null ? void 0 : T.delete(i), (o & J) === 0 && (o & N && (l.f |= J), St(i, j));
      } else a && ((o & F) !== 0 && P !== null && P.add(l), ee(l));
    }
  }
}
function oe(e) {
  if (typeof e != "object" || e === null || X in e) return e;
  const t = lt(e);
  if (t !== Zt && t !== Wt) return e;
  var n = /* @__PURE__ */ new Map(), r = zt(e), s = q(0), f = W, l = (o) => {
    if (W === f) return o();
    var a = _, i = W;
    V(null), ot(f);
    var u = o();
    return V(a), ot(i), u;
  };
  return r && n.set("length", q(e.length)), new Proxy(e, { defineProperty(o, a, i) {
    (!("value" in i) || i.configurable === false || i.enumerable === false || i.writable === false) && on();
    var u = n.get(a);
    return u === void 0 ? u = l(() => {
      var v = q(i.value);
      return n.set(a, v), v;
    }) : K(u, i.value, true), true;
  }, deleteProperty(o, a) {
    var i = n.get(a);
    if (i === void 0) {
      if (a in o) {
        const u = l(() => q(b));
        n.set(a, u), De(s);
      }
    } else K(i, b), De(s);
    return true;
  }, get(o, a, i) {
    var _a2;
    if (a === X) return e;
    var u = n.get(a), v = a in o;
    if (u === void 0 && (!v || ((_a2 = ue(o, a)) == null ? void 0 : _a2.writable)) && (u = l(() => {
      var d = oe(v ? o[a] : b), O = q(d);
      return O;
    }), n.set(a, u)), u !== void 0) {
      var c = le(u);
      return c === b ? void 0 : c;
    }
    return Reflect.get(o, a, i);
  }, getOwnPropertyDescriptor(o, a) {
    var i = Reflect.getOwnPropertyDescriptor(o, a);
    if (i && "value" in i) {
      var u = n.get(a);
      u && (i.value = le(u));
    } else if (i === void 0) {
      var v = n.get(a), c = v == null ? void 0 : v.v;
      if (v !== void 0 && c !== b) return { enumerable: true, configurable: true, value: c, writable: true };
    }
    return i;
  }, has(o, a) {
    var _a2;
    if (a === X) return true;
    var i = n.get(a), u = i !== void 0 && i.v !== b || Reflect.has(o, a);
    if (i !== void 0 || h !== null && (!u || ((_a2 = ue(o, a)) == null ? void 0 : _a2.writable))) {
      i === void 0 && (i = l(() => {
        var c = u ? oe(o[a]) : b, d = q(c);
        return d;
      }), n.set(a, i));
      var v = le(i);
      if (v === b) return false;
    }
    return u;
  }, set(o, a, i, u) {
    var _a2;
    var v = n.get(a), c = a in o;
    if (r && a === "length") for (var d = i; d < v.v; d += 1) {
      var O = n.get(d + "");
      O !== void 0 ? K(O, b) : d in o && (O = l(() => q(b)), n.set(d + "", O));
    }
    if (v === void 0) (!c || ((_a2 = ue(o, a)) == null ? void 0 : _a2.writable)) && (v = l(() => q(void 0)), K(v, oe(i)), n.set(a, v));
    else {
      c = v.v !== b;
      var Ne = l(() => oe(i));
      K(v, Ne);
    }
    var et = Reflect.getOwnPropertyDescriptor(o, a);
    if ((et == null ? void 0 : et.set) && et.set.call(u, i), !c) {
      if (r && typeof a == "string") {
        var tt = n.get("length"), Ie = Number(a);
        Number.isInteger(Ie) && Ie >= tt.v && K(tt, Ie + 1);
      }
      De(s);
    }
    return true;
  }, ownKeys(o) {
    le(s);
    var a = Reflect.ownKeys(o).filter((v) => {
      var c = n.get(v);
      return c === void 0 || c.v !== b;
    });
    for (var [i, u] of n) u.v !== b && !(i in o) && a.push(i);
    return a;
  }, setPrototypeOf() {
    ln();
  } });
}
function st(e) {
  try {
    if (e !== null && typeof e == "object" && X in e) return e[X];
  } catch {
  }
  return e;
}
function Or(e, t) {
  return Object.is(st(e), st(t));
}
var ft, An, xn, Ot, Nt;
function Nr() {
  if (ft === void 0) {
    ft = window, An = document, xn = /Firefox/.test(navigator.userAgent);
    var e = Element.prototype, t = Node.prototype, n = Text.prototype;
    Ot = ue(t, "firstChild").get, Nt = ue(t, "nextSibling").get, nt(e) && (e.__click = void 0, e.__className = void 0, e.__attributes = null, e.__style = void 0, e.__e = void 0), nt(n) && (n.__t = void 0);
  }
}
function Ae(e = "") {
  return document.createTextNode(e);
}
function Ye(e) {
  return Ot.call(e);
}
function G(e) {
  return Nt.call(e);
}
function Ir(e, t) {
  if (!Q) return Ye(e);
  var n = Ye(k);
  if (n === null) n = k.appendChild(Ae());
  else if (t && n.nodeType !== Ke) {
    var r = Ae();
    return n == null ? void 0 : n.before(r), fe(r), r;
  }
  return fe(n), n;
}
function Cr(e, t = false) {
  if (!Q) {
    var n = Ye(e);
    return n instanceof Comment && n.data === "" ? G(n) : n;
  }
  if (t && (k == null ? void 0 : k.nodeType) !== Ke) {
    var r = Ae();
    return k == null ? void 0 : k.before(r), fe(r), r;
  }
  return k;
}
function Dr(e, t = 1, n = false) {
  let r = Q ? k : e;
  for (var s; t--; ) s = r, r = G(r);
  if (!Q) return r;
  if (n && (r == null ? void 0 : r.nodeType) !== Ke) {
    var f = Ae();
    return r === null ? s == null ? void 0 : s.after(f) : r.before(f), fe(f), f;
  }
  return fe(r), r;
}
function Pr(e) {
  e.textContent = "";
}
function Mr() {
  return false;
}
function Fr(e, t) {
  if (t) {
    const n = document.body;
    e.autofocus = true, bt(() => {
      document.activeElement === n && e.focus();
    });
  }
}
let it = false;
function kn() {
  it || (it = true, document.addEventListener("reset", (e) => {
    Promise.resolve().then(() => {
      var _a2;
      if (!e.defaultPrevented) for (const t of e.target.elements) (_a2 = t.__on_r) == null ? void 0 : _a2.call(t);
    });
  }, { capture: true }));
}
function Lr(e, t, n, r = true) {
  r && n();
  for (var s of t) e.addEventListener(s, n);
  Ct(() => {
    for (var f of t) e.removeEventListener(f, n);
  });
}
function Qe(e) {
  var t = _, n = h;
  V(null), ie(null);
  try {
    return e();
  } finally {
    V(t), ie(n);
  }
}
function jr(e, t, n, r = n) {
  e.addEventListener(t, () => Qe(n));
  const s = e.__on_r;
  s ? e.__on_r = () => {
    s(), r(true);
  } : e.__on_r = () => r(true), kn();
}
function It(e) {
  h === null && (_ === null && rn(), nn()), ae && tn();
}
function Rn(e, t) {
  var n = t.last;
  n === null ? t.last = t.first = e : (n.next = e, e.prev = n, t.last = e);
}
function I(e, t, n) {
  var r = h;
  r !== null && (r.f & D) !== 0 && (e |= D);
  var s = { ctx: w, deps: null, nodes: null, f: e | A | N, first: null, fn: t, last: null, next: null, parent: r, b: r && r.b, prev: null, teardown: null, wv: 0, ac: null };
  if (n) try {
    ve(s), s.f |= Ve;
  } catch (o) {
    throw te(s), o;
  }
  else t !== null && ee(s);
  var f = s;
  if (n && f.deps === null && f.teardown === null && f.nodes === null && f.first === f.last && (f.f & de) === 0 && (f = f.first, (e & F) !== 0 && (e & Ee) !== 0 && f !== null && (f.f |= Ee)), f !== null && (f.parent = r, r !== null && Rn(f, r), _ !== null && (_.f & y) !== 0 && (e & ne) === 0)) {
    var l = _;
    (l.effects ?? (l.effects = [])).push(f);
  }
  return s;
}
function xe() {
  return _ !== null && !C;
}
function Ct(e) {
  const t = I(Se, null, false);
  return E(t, m), t.teardown = e, t;
}
function Sn(e) {
  It();
  var t = h.f, n = !_ && (t & L) !== 0 && (t & Ve) === 0;
  if (n) {
    var r = w;
    (r.e ?? (r.e = [])).push(e);
  } else return Dt(e);
}
function Dt(e) {
  return I(Ue | vt, e, false);
}
function qr(e) {
  return It(), I(Se | vt, e, true);
}
function Yr(e) {
  B.ensure();
  const t = I(ne | de, e, true);
  return (n = {}) => new Promise((r) => {
    n.outro ? Cn(t, () => {
      te(t), r(void 0);
    }) : (te(t), r(void 0));
  });
}
function Hr(e) {
  return I(Ue, e, false);
}
function On(e) {
  return I(Ge | de, e, true);
}
function Ur(e, t = 0) {
  return I(Se | t, e, true);
}
function Br(e, t = [], n = [], r = []) {
  yn(r, t, n, (s) => {
    I(Se, () => e(...s.map(le)), true);
  });
}
function Vr(e, t = 0) {
  var n = I(F | t, e, true);
  return n;
}
function Gr(e, t = 0) {
  var n = I(ct | t, e, true);
  return n;
}
function Kr(e) {
  return I(L | de, e, true);
}
function Pt(e) {
  var t = e.teardown;
  if (t !== null) {
    const n = ae, r = _;
    at(true), V(null);
    try {
      t.call(null);
    } finally {
      at(n), V(r);
    }
  }
}
function Mt(e, t = false) {
  var n = e.first;
  for (e.first = e.last = null; n !== null; ) {
    const s = n.ac;
    s !== null && Qe(() => {
      s.abort(se);
    });
    var r = n.next;
    (n.f & ne) !== 0 ? n.parent = null : te(n, t), n = r;
  }
}
function Nn(e) {
  for (var t = e.first; t !== null; ) {
    var n = t.next;
    (t.f & L) === 0 && te(t), t = n;
  }
}
function te(e, t = true) {
  var n = false;
  (t || (e.f & _t) !== 0) && e.nodes !== null && e.nodes.end !== null && (In(e.nodes.start, e.nodes.end), n = true), Mt(e, t && !n), Re(e, 0), E(e, Y);
  var r = e.nodes && e.nodes.t;
  if (r !== null) for (const f of r) f.stop();
  Pt(e);
  var s = e.parent;
  s !== null && s.first !== null && Ft(e), e.next = e.prev = e.teardown = e.ctx = e.deps = e.fn = e.nodes = e.ac = null;
}
function In(e, t) {
  for (; e !== null; ) {
    var n = e === t ? null : G(e);
    e.remove(), e = n;
  }
}
function Ft(e) {
  var t = e.parent, n = e.prev, r = e.next;
  n !== null && (n.next = r), r !== null && (r.prev = n), t !== null && (t.first === e && (t.first = r), t.last === e && (t.last = n));
}
function Cn(e, t, n = true) {
  var r = [];
  Lt(e, r, true);
  var s = () => {
    n && te(e), t && t();
  }, f = r.length;
  if (f > 0) {
    var l = () => --f || s();
    for (var o of r) o.out(l);
  } else s();
}
function Lt(e, t, n) {
  if ((e.f & D) === 0) {
    e.f ^= D;
    var r = e.nodes && e.nodes.t;
    if (r !== null) for (const o of r) (o.is_global || n) && t.push(o);
    for (var s = e.first; s !== null; ) {
      var f = s.next, l = (s.f & Ee) !== 0 || (s.f & L) !== 0 && (e.f & F) !== 0;
      Lt(s, t, l ? n : false), s = f;
    }
  }
}
function zr(e) {
  jt(e, true);
}
function jt(e, t) {
  if ((e.f & D) !== 0) {
    e.f ^= D, (e.f & m) === 0 && (E(e, A), ee(e));
    for (var n = e.first; n !== null; ) {
      var r = n.next, s = (n.f & Ee) !== 0 || (n.f & L) !== 0;
      jt(n, s ? t : false), n = r;
    }
    var f = e.nodes && e.nodes.t;
    if (f !== null) for (const l of f) (l.is_global || t) && l.in();
  }
}
function $r(e, t) {
  if (e.nodes) for (var n = e.nodes.start, r = e.nodes.end; n !== null; ) {
    var s = n === r ? null : G(n);
    t.append(n), n = s;
  }
}
let Z = false;
function ke(e) {
  Z = e;
}
let ae = false;
function at(e) {
  ae = e;
}
let _ = null, C = false;
function V(e) {
  _ = e;
}
let h = null;
function ie(e) {
  h = e;
}
let M = null;
function qt(e) {
  _ !== null && (M === null ? M = [e] : M.push(e));
}
let g = null, x = 0, R = null;
function Dn(e) {
  R = e;
}
let Yt = 1, _e = 0, W = _e;
function ot(e) {
  W = e;
}
function Ht() {
  return ++Yt;
}
function we(e) {
  var t = e.f;
  if ((t & A) !== 0) return true;
  if (t & y && (e.f &= ~J), (t & j) !== 0) {
    var n = e.deps;
    if (n !== null) for (var r = n.length, s = 0; s < r; s++) {
      var f = n[s];
      if (we(f) && xt(f), f.wv > e.wv) return true;
    }
    (t & N) !== 0 && T === null && E(e, m);
  }
  return false;
}
function Ut(e, t, n = true) {
  var r = e.reactions;
  if (r !== null && !(M == null ? void 0 : M.includes(e))) for (var s = 0; s < r.length; s++) {
    var f = r[s];
    (f.f & y) !== 0 ? Ut(f, t, false) : t === f && (n ? E(f, A) : (f.f & m) !== 0 && E(f, j), ee(f));
  }
}
function Bt(e) {
  var _a2;
  var t = g, n = x, r = R, s = _, f = M, l = w, o = C, a = W, i = e.f;
  g = null, x = 0, R = null, _ = (i & (L | ne)) === 0 ? e : null, M = null, be(e.ctx), C = false, W = ++_e, e.ac !== null && (Qe(() => {
    e.ac.abort(se);
  }), e.ac = null);
  try {
    e.f |= Me;
    var u = e.fn, v = u(), c = e.deps;
    if (g !== null) {
      var d;
      if (Re(e, x), c !== null && x > 0) for (c.length = x + g.length, d = 0; d < g.length; d++) c[x + d] = g[d];
      else e.deps = c = g;
      if (xe() && (e.f & N) !== 0) for (d = x; d < c.length; d++) ((_a2 = c[d]).reactions ?? (_a2.reactions = [])).push(e);
    } else c !== null && x < c.length && (Re(e, x), c.length = x);
    if (pe() && R !== null && !C && c !== null && (e.f & (y | j | A)) === 0) for (d = 0; d < R.length; d++) Ut(R[d], e);
    return s !== null && s !== e && (_e++, R !== null && (r === null ? r = R : r.push(...R))), (e.f & H) !== 0 && (e.f ^= H), v;
  } catch (O) {
    return pn(O);
  } finally {
    e.f ^= Me, g = t, x = n, R = r, _ = s, M = f, be(l), C = o, W = a;
  }
}
function Pn(e, t) {
  let n = t.reactions;
  if (n !== null) {
    var r = $t.call(n, e);
    if (r !== -1) {
      var s = n.length - 1;
      s === 0 ? n = t.reactions = null : (n[r] = n[s], n.pop());
    }
  }
  n === null && (t.f & y) !== 0 && (g === null || !g.includes(t)) && (E(t, j), (t.f & N) !== 0 && (t.f ^= N, t.f &= ~J), At(t), Re(t, 0));
}
function Re(e, t) {
  var n = e.deps;
  if (n !== null) for (var r = t; r < n.length; r++) Pn(e, n[r]);
}
function ve(e) {
  var t = e.f;
  if ((t & Y) === 0) {
    E(e, m);
    var n = h, r = Z;
    h = e, Z = true;
    try {
      (t & (F | ct)) !== 0 ? Nn(e) : Mt(e), Pt(e);
      var s = Bt(e);
      e.teardown = typeof s == "function" ? s : null, e.wv = Yt;
      var f;
    } finally {
      Z = r, h = n;
    }
  }
}
async function Xr() {
  await Promise.resolve(), Le();
}
function Zr() {
  return B.ensure().settled();
}
function le(e) {
  var t = e.f, n = (t & y) !== 0;
  if (_ !== null && !C) {
    var r = h !== null && (h.f & Y) !== 0;
    if (!r && !(M == null ? void 0 : M.includes(e))) {
      var s = _.deps;
      if ((_.f & Me) !== 0) e.rv < _e && (e.rv = _e, g === null && s !== null && s[x] === e ? x++ : g === null ? g = [e] : g.includes(e) || g.push(e));
      else {
        (_.deps ?? (_.deps = [])).push(e);
        var f = e.reactions;
        f === null ? e.reactions = [_] : f.includes(_) || f.push(_);
      }
    }
  }
  if (ae) {
    if (U.has(e)) return U.get(e);
    if (n) {
      var l = e, o = l.v;
      return ((l.f & m) === 0 && l.reactions !== null || Gt(l)) && (o = We(l)), U.set(l, o), o;
    }
  } else n && (!(T == null ? void 0 : T.has(e)) || (p == null ? void 0 : p.is_fork) && !xe()) && (l = e, we(l) && xt(l), Z && xe() && (l.f & N) === 0 && Vt(l));
  if (T == null ? void 0 : T.has(e)) return T.get(e);
  if ((e.f & H) !== 0) throw e.v;
  return e.v;
}
function Vt(e) {
  if (e.deps !== null) {
    e.f ^= N;
    for (const t of e.deps) (t.reactions ?? (t.reactions = [])).push(e), (t.f & y) !== 0 && (t.f & N) === 0 && Vt(t);
  }
}
function Gt(e) {
  if (e.v === b) return true;
  if (e.deps === null) return false;
  for (const t of e.deps) if (U.has(t) || (t.f & y) !== 0 && Gt(t)) return true;
  return false;
}
function Kt(e) {
  var t = C;
  try {
    return C = true, e();
  } finally {
    C = t;
  }
}
const Mn = -7169;
function E(e, t) {
  e.f = e.f & Mn | t;
}
function Wr(e) {
  if (!(typeof e != "object" || !e || e instanceof EventTarget)) {
    if (X in e) He(e);
    else if (!Array.isArray(e)) for (let t in e) {
      const n = e[t];
      typeof n == "object" && n && X in n && He(n);
    }
  }
}
function He(e, t = /* @__PURE__ */ new Set()) {
  if (typeof e == "object" && e !== null && !(e instanceof EventTarget) && !t.has(e)) {
    t.add(e), e instanceof Date && e.getTime();
    for (let r in e) try {
      He(e[r], t);
    } catch {
    }
    const n = lt(e);
    if (n !== Object.prototype && n !== Array.prototype && n !== Map.prototype && n !== Set.prototype && n !== Date.prototype) {
      const r = Xt(n);
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
  const r = Kt(() => e.subscribe(t, n));
  return r.unsubscribe ? () => r.unsubscribe() : r;
}
const re = [];
function Jr(e, t = ye) {
  let n = null;
  const r = /* @__PURE__ */ new Set();
  function s(o) {
    if (wt(e, o) && (e = o, n)) {
      const a = !re.length;
      for (const i of r) i[1](), re.push(i, e);
      if (a) {
        for (let i = 0; i < re.length; i += 2) re[i][0](re[i + 1]);
        re.length = 0;
      }
    }
  }
  function f(o) {
    s(o(e));
  }
  function l(o, a = ye) {
    const i = [o, a];
    return r.add(i), r.size === 1 && (n = t(s, f) || ye), o(e), () => {
      r.delete(i), r.size === 0 && n && (n(), n = null);
    };
  }
  return { set: s, update: f, subscribe: l };
}
function Qr(e) {
  let t;
  return Fn(e, (n) => t = n)(), t;
}
function es(e) {
  w === null && ht(), he && w.l !== null ? Ln(w).m.push(e) : Sn(() => {
    const t = Kt(e);
    if (typeof t == "function") return t;
  });
}
function Ln(e) {
  var t = e.l;
  return t.u ?? (t.u = { a: [], b: [], m: [] });
}
export {
  Y as $,
  Mr as A,
  Vr as B,
  hr as C,
  Er as D,
  Ee as E,
  yr as F,
  fe as G,
  _n as H,
  dr as I,
  Hr as J,
  Ur as K,
  bt as L,
  ye as M,
  Sr as N,
  Fn as O,
  Qr as P,
  Ct as Q,
  qn as R,
  X as S,
  K as T,
  ue as U,
  zn as V,
  nr as W,
  mn as X,
  oe as Y,
  ae as Z,
  h as _,
  Sn as a,
  Ce as a$,
  rr as a0,
  he as a1,
  tr as a2,
  er as a3,
  sr as a4,
  Bn as a5,
  Le as a6,
  es as a7,
  q as a8,
  Xr as a9,
  cn as aA,
  G as aB,
  ze as aC,
  Kn as aD,
  Pr as aE,
  jn as aF,
  Yr as aG,
  vn as aH,
  $e as aI,
  Un as aJ,
  zt as aK,
  Xn as aL,
  Qn as aM,
  Zn as aN,
  D as aO,
  Wn as aP,
  Jn as aQ,
  F as aR,
  ar as aS,
  fr as aT,
  ir as aU,
  Yn as aV,
  _t as aW,
  Gr as aX,
  _r as aY,
  Or as aZ,
  jr as a_,
  Rr as aa,
  xe as ab,
  Je as ac,
  De as ad,
  dt as ae,
  B as af,
  ie as ag,
  V as ah,
  be as ai,
  pn as aj,
  _ as ak,
  qe as al,
  wr as am,
  me as an,
  $n as ao,
  de as ap,
  Be as aq,
  vr as ar,
  Qe as as,
  Ye as at,
  xn as au,
  or as av,
  lr as aw,
  Ve as ax,
  Ke as ay,
  Nr as az,
  Kt as b,
  yn as b0,
  ur as b1,
  lt as b2,
  cr as b3,
  Xt as b4,
  Fr as b5,
  b as b6,
  kn as b7,
  Vn as b8,
  Jr as b9,
  An as ba,
  Tr as bb,
  mr as bc,
  gr as bd,
  Lr as be,
  kr as bf,
  Zr as bg,
  w as c,
  Hn as d,
  br as e,
  Wr as f,
  le as g,
  Ze as h,
  Cr as i,
  xr as j,
  Ir as k,
  pr as l,
  p as m,
  zr as n,
  te as o,
  Ar as p,
  Cn as q,
  Jt as r,
  Dr as s,
  Br as t,
  qr as u,
  Ae as v,
  Kr as w,
  Q as x,
  k as y,
  $r as z
};
