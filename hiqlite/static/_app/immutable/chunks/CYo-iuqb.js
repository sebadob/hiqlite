var __defProp = Object.defineProperty;
var __defNormalProp = (obj, key, value) => key in obj ? __defProp(obj, key, { enumerable: true, configurable: true, writable: true, value }) : obj[key] = value;
var __publicField = (obj, key, value) => __defNormalProp(obj, typeof key !== "symbol" ? key + "" : key, value);
var Fe = Array.isArray, Gt = Array.prototype.indexOf, Kt = Array.from, zt = Object.defineProperty, ie = Object.getOwnPropertyDescriptor, Xt = Object.getOwnPropertyDescriptors, Zt = Object.prototype, Jt = Array.prototype, st = Object.getPrototypeOf, Ze = Object.isExtensible;
function sr(e) {
  return typeof e == "function";
}
const oe = () => {
};
function lr(e) {
  return e();
}
function lt(e) {
  for (var t = 0; t < e.length; t++) e[t]();
}
const N = 2, ut = 4, be = 8, qe = 16, q = 32, te = 64, je = 128, T = 256, ge = 512, O = 1024, B = 2048, z = 4096, ee = 8192, He = 16384, it = 32768, Ve = 65536, Je = 1 << 17, ot = 1 << 18, ft = 1 << 19, Re = 1 << 20, G = Symbol("$state"), ur = Symbol("legacy props"), ir = Symbol(""), ct = new class extends Error {
  constructor() {
    super(...arguments);
    __publicField(this, "name", "StaleReactionError");
    __publicField(this, "message", "The reaction that called `getAbortSignal()` was re-run or destroyed");
  }
}(), Te = 3, ce = 8;
function _t(e) {
  return e === this.v;
}
function vt(e, t) {
  return e != e ? t == t : e !== t || e !== null && typeof e == "object" || typeof e == "function";
}
function dt(e) {
  return !vt(e, this.v);
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
  throw new Error("https://svelte.dev/e/get_abort_signal_outside_reaction");
}
function an() {
  throw new Error("https://svelte.dev/e/hydration_failed");
}
function pt(e) {
  throw new Error("https://svelte.dev/e/lifecycle_legacy_only");
}
function fr(e) {
  throw new Error("https://svelte.dev/e/props_invalid_value");
}
function sn() {
  throw new Error("https://svelte.dev/e/state_descriptors_fixed");
}
function ln() {
  throw new Error("https://svelte.dev/e/state_prototype_fixed");
}
function un() {
  throw new Error("https://svelte.dev/e/state_unsafe_mutation");
}
let he = false;
function cr() {
  he = true;
}
const _r = 1, vr = 2, dr = 4, pr = 8, hr = 16, yr = 1, wr = 2, gr = 4, Er = 8, mr = 16, br = 1, Tr = 2, Ar = 4, on = 1, fn = 2, Ye = "[", cn = "[!", ht = "]", Q = {}, m = Symbol(), xr = "http://www.w3.org/1999/xhtml", Sr = "@attach";
function ne(e) {
  throw new Error("https://svelte.dev/e/lifecycle_outside_component");
}
let d = null;
function Qe(e) {
  d = e;
}
function _n(e) {
  return xe().get(e);
}
function vn(e, t) {
  return xe().set(e, t), t;
}
function dn(e) {
  return xe().has(e);
}
function pn() {
  return xe();
}
function hn(e, t = false, n) {
  var r = d = { p: d, c: null, d: false, e: null, m: false, s: e, x: null, l: null };
  he && !t && (d.l = { s: null, u: null, r1: [], r2: Ue(false) }), Ne(() => {
    r.d = true;
  });
}
function yn(e) {
  const t = d;
  if (t !== null) {
    const o = t.e;
    if (o !== null) {
      var n = h, r = _;
      t.e = null;
      try {
        for (var a = 0; a < o.length; a++) {
          var s = o[a];
          $(s.effect), P(s.reaction), xt(s.fn);
        }
      } finally {
        $(n), P(r);
      }
    }
    d = t.p, t.m = true;
  }
  return {};
}
function Ae() {
  return !he || d !== null && d.l === null;
}
function xe(e) {
  return d === null && ne(), d.c ?? (d.c = new Map(wn(d) || void 0));
}
function wn(e) {
  let t = e.p;
  for (; t !== null; ) {
    const n = t.c;
    if (n !== null) return n;
    t = t.p;
  }
  return null;
}
function se(e) {
  if (typeof e != "object" || e === null || G in e) return e;
  const t = st(e);
  if (t !== Zt && t !== Jt) return e;
  var n = /* @__PURE__ */ new Map(), r = Fe(e), a = H(0), s = _, o = (u) => {
    var l = _;
    P(s);
    var i = u();
    return P(l), i;
  };
  return r && n.set("length", H(e.length)), new Proxy(e, { defineProperty(u, l, i) {
    (!("value" in i) || i.configurable === false || i.enumerable === false || i.writable === false) && sn();
    var v = n.get(l);
    return v === void 0 ? v = o(() => {
      var c = H(i.value);
      return n.set(l, c), c;
    }) : V(v, i.value, true), true;
  }, deleteProperty(u, l) {
    var i = n.get(l);
    if (i === void 0) {
      if (l in u) {
        const f = o(() => H(m));
        n.set(l, f), ke(a);
      }
    } else {
      if (r && typeof l == "string") {
        var v = n.get("length"), c = Number(l);
        Number.isInteger(c) && c < v.v && V(v, c);
      }
      V(i, m), ke(a);
    }
    return true;
  }, get(u, l, i) {
    var _a;
    if (l === G) return e;
    var v = n.get(l), c = l in u;
    if (v === void 0 && (!c || ((_a = ie(u, l)) == null ? void 0 : _a.writable)) && (v = o(() => {
      var y = se(c ? u[l] : m), E = H(y);
      return E;
    }), n.set(l, v)), v !== void 0) {
      var f = le(v);
      return f === m ? void 0 : f;
    }
    return Reflect.get(u, l, i);
  }, getOwnPropertyDescriptor(u, l) {
    var i = Reflect.getOwnPropertyDescriptor(u, l);
    if (i && "value" in i) {
      var v = n.get(l);
      v && (i.value = le(v));
    } else if (i === void 0) {
      var c = n.get(l), f = c == null ? void 0 : c.v;
      if (c !== void 0 && f !== m) return { enumerable: true, configurable: true, value: f, writable: true };
    }
    return i;
  }, has(u, l) {
    var _a;
    if (l === G) return true;
    var i = n.get(l), v = i !== void 0 && i.v !== m || Reflect.has(u, l);
    if (i !== void 0 || h !== null && (!v || ((_a = ie(u, l)) == null ? void 0 : _a.writable))) {
      i === void 0 && (i = o(() => {
        var f = v ? se(u[l]) : m, y = H(f);
        return y;
      }), n.set(l, i));
      var c = le(i);
      if (c === m) return false;
    }
    return v;
  }, set(u, l, i, v) {
    var _a;
    var c = n.get(l), f = l in u;
    if (r && l === "length") for (var y = i; y < c.v; y += 1) {
      var E = n.get(y + "");
      E !== void 0 ? V(E, m) : y in u && (E = o(() => H(m)), n.set(y + "", E));
    }
    if (c === void 0) (!f || ((_a = ie(u, l)) == null ? void 0 : _a.writable)) && (c = o(() => H(void 0)), V(c, se(i)), n.set(l, c));
    else {
      f = c.v !== m;
      var M = o(() => se(i));
      V(c, M);
    }
    var ye = Reflect.getOwnPropertyDescriptor(u, l);
    if ((ye == null ? void 0 : ye.set) && ye.set.call(v, i), !f) {
      if (r && typeof l == "string") {
        var we = n.get("length"), j = Number(l);
        Number.isInteger(j) && j >= we.v && V(we, j + 1);
      }
      ke(a);
    }
    return true;
  }, ownKeys(u) {
    le(a);
    var l = Reflect.ownKeys(u).filter((c) => {
      var f = n.get(c);
      return f === void 0 || f.v !== m;
    });
    for (var [i, v] of n) v.v !== m && !(i in u) && l.push(i);
    return l;
  }, setPrototypeOf() {
    ln();
  } });
}
function ke(e, t = 1) {
  V(e, e.v + t);
}
function et(e) {
  try {
    if (e !== null && typeof e == "object" && G in e) return e[G];
  } catch {
  }
  return e;
}
function Nr(e, t) {
  return Object.is(et(e), et(t));
}
function Be(e) {
  var t = N | B, n = _ !== null && (_.f & N) !== 0 ? _ : null;
  return h === null || n !== null && (n.f & T) !== 0 ? t |= T : h.f |= ft, { ctx: d, deps: null, effects: null, equals: _t, f: t, fn: e, reactions: null, rv: 0, v: null, wv: 0, parent: n ?? h, ac: null };
}
function Or(e) {
  const t = Be(e);
  return Mt(t), t;
}
function Ir(e) {
  const t = Be(e);
  return t.equals = dt, t;
}
function yt(e) {
  var t = e.effects;
  if (t !== null) {
    e.effects = null;
    for (var n = 0; n < t.length; n += 1) U(t[n]);
  }
}
function gn(e) {
  for (var t = e.parent; t !== null; ) {
    if ((t.f & N) === 0) return t;
    t = t.parent;
  }
  return null;
}
function wt(e) {
  var t, n = h;
  $(gn(e));
  try {
    yt(e), t = Ht(e);
  } finally {
    $(n);
  }
  return t;
}
function gt(e) {
  var t = wt(e);
  if (e.equals(t) || (e.v = t, e.wv = qt()), !ae) {
    var n = (Y || (e.f & T) !== 0) && e.deps !== null ? z : O;
    L(e, n);
  }
}
const _e = /* @__PURE__ */ new Map();
function Ue(e, t) {
  var n = { f: 0, v: e, reactions: null, equals: _t, rv: 0, wv: 0 };
  return n;
}
function H(e, t) {
  const n = Ue(e);
  return Mt(n), n;
}
function kr(e, t = false, n = true) {
  var _a;
  const r = Ue(e);
  return t || (r.equals = dt), he && n && d !== null && d.l !== null && ((_a = d.l).s ?? (_a.s = [])).push(r), r;
}
function V(e, t, n = false) {
  _ !== null && (!R || (_.f & Je) !== 0) && Ae() && (_.f & (N | qe | Je)) !== 0 && !((S == null ? void 0 : S[1].includes(e)) && S[0] === _) && un();
  let r = n ? se(t) : t;
  return En(e, r);
}
function En(e, t) {
  if (!e.equals(t)) {
    var n = e.v;
    ae ? _e.set(e, t) : _e.set(e, n), e.v = t, (e.f & N) !== 0 && ((e.f & B) !== 0 && wt(e), L(e, (e.f & T) === 0 ? O : z)), e.wv = qt(), Et(e, B), Ae() && h !== null && (h.f & O) !== 0 && (h.f & (q | te)) === 0 && (x === null ? Pn([e]) : x.push(e));
  }
  return t;
}
function Et(e, t) {
  var n = e.reactions;
  if (n !== null) for (var r = Ae(), a = n.length, s = 0; s < a; s++) {
    var o = n[s], u = o.f;
    (u & B) === 0 && (!r && o === h || (L(o, t), (u & (O | T)) !== 0 && ((u & N) !== 0 ? Et(o, z) : Ke(o))));
  }
}
function Se(e) {
  console.warn("https://svelte.dev/e/hydration_mismatch");
}
function Rr() {
  console.warn("https://svelte.dev/e/select_multiple_invalid_value");
}
let w = false;
function J(e) {
  w = e;
}
let p;
function k(e) {
  if (e === null) throw Se(), Q;
  return p = e;
}
function $e() {
  return k(D(p));
}
function Cr(e) {
  if (w) {
    if (D(p) !== null) throw Se(), Q;
    p = e;
  }
}
function Dr(e = 1) {
  if (w) {
    for (var t = e, n = p; t--; ) n = D(n);
    p = n;
  }
}
function Pr() {
  for (var e = 0, t = p; ; ) {
    if (t.nodeType === ce) {
      var n = t.data;
      if (n === ht) {
        if (e === 0) return t;
        e -= 1;
      } else (n === Ye || n === cn) && (e += 1);
    }
    var r = D(t);
    t.remove(), t = r;
  }
}
function Lr(e) {
  if (!e || e.nodeType !== ce) throw Se(), Q;
  return e.data;
}
var tt, mn, mt, bt, Tt;
function Ce() {
  if (tt === void 0) {
    tt = window, mn = document, mt = /Firefox/.test(navigator.userAgent);
    var e = Element.prototype, t = Node.prototype, n = Text.prototype;
    bt = ie(t, "firstChild").get, Tt = ie(t, "nextSibling").get, Ze(e) && (e.__click = void 0, e.__className = void 0, e.__attributes = null, e.__style = void 0, e.__e = void 0), Ze(n) && (n.__t = void 0);
  }
}
function F(e = "") {
  return document.createTextNode(e);
}
function C(e) {
  return bt.call(e);
}
function D(e) {
  return Tt.call(e);
}
function Mr(e, t) {
  if (!w) return C(e);
  var n = C(p);
  if (n === null) n = p.appendChild(F());
  else if (t && n.nodeType !== Te) {
    var r = F();
    return n == null ? void 0 : n.before(r), k(r), r;
  }
  return k(n), n;
}
function Fr(e, t) {
  if (!w) {
    var n = C(e);
    return n instanceof Comment && n.data === "" ? D(n) : n;
  }
  if (t && (p == null ? void 0 : p.nodeType) !== Te) {
    var r = F();
    return p == null ? void 0 : p.before(r), k(r), r;
  }
  return p;
}
function qr(e, t = 1, n = false) {
  let r = w ? p : e;
  for (var a; t--; ) a = r, r = D(r);
  if (!w) return r;
  if (n && (r == null ? void 0 : r.nodeType) !== Te) {
    var s = F();
    return r === null ? a == null ? void 0 : a.after(s) : r.before(s), k(s), s;
  }
  return k(r), r;
}
function bn(e) {
  e.textContent = "";
}
function At(e) {
  h === null && _ === null && tn(), _ !== null && (_.f & T) !== 0 && h === null && en(), ae && Qt();
}
function Tn(e, t) {
  var n = t.last;
  n === null ? t.last = t.first = e : (n.next = e, e.prev = n, t.last = e);
}
function re(e, t, n, r = true) {
  var a = h, s = { ctx: d, deps: null, nodes_start: null, nodes_end: null, f: e | B, first: null, fn: t, last: null, next: null, parent: a, b: a && a.b, prev: null, teardown: null, transitions: null, wv: 0, ac: null };
  if (n) try {
    Ge(s), s.f |= it;
  } catch (l) {
    throw U(s), l;
  }
  else t !== null && Ke(s);
  var o = n && s.deps === null && s.first === null && s.nodes_start === null && s.teardown === null && (s.f & (ft | je)) === 0;
  if (!o && r && (a !== null && Tn(s, a), _ !== null && (_.f & N) !== 0)) {
    var u = _;
    (u.effects ?? (u.effects = [])).push(s);
  }
  return s;
}
function Ne(e) {
  const t = re(be, null, false);
  return L(t, O), t.teardown = e, t;
}
function An(e) {
  At();
  var t = h !== null && (h.f & q) !== 0 && d !== null && !d.m;
  if (t) {
    var n = d;
    (n.e ?? (n.e = [])).push({ fn: e, effect: h, reaction: _ });
  } else {
    var r = xt(e);
    return r;
  }
}
function jr(e) {
  return At(), Sn(e);
}
function xn(e) {
  const t = re(te, e, true);
  return (n = {}) => new Promise((r) => {
    n.outro ? In(t, () => {
      U(t), r(void 0);
    }) : (U(t), r(void 0));
  });
}
function xt(e) {
  return re(ut, e, false);
}
function Sn(e) {
  return re(be, e, true);
}
function Hr(e, t = [], n = Be) {
  const r = t.map(n);
  return We(() => e(...r.map(le)));
}
function We(e, t = 0) {
  var n = re(be | qe | t, e, true);
  return n;
}
function St(e, t = true) {
  return re(be | q, e, true, t);
}
function Nt(e) {
  var t = e.teardown;
  if (t !== null) {
    const n = ae, r = _;
    nt(true), P(null);
    try {
      t.call(null);
    } finally {
      nt(n), P(r);
    }
  }
}
function Ot(e, t = false) {
  var _a;
  var n = e.first;
  for (e.first = e.last = null; n !== null; ) {
    (_a = n.ac) == null ? void 0 : _a.abort(ct);
    var r = n.next;
    (n.f & te) !== 0 ? n.parent = null : U(n, t), n = r;
  }
}
function Nn(e) {
  for (var t = e.first; t !== null; ) {
    var n = t.next;
    (t.f & q) === 0 && U(t), t = n;
  }
}
function U(e, t = true) {
  var n = false;
  (t || (e.f & ot) !== 0) && e.nodes_start !== null && e.nodes_end !== null && (On(e.nodes_start, e.nodes_end), n = true), Ot(e, t && !n), me(e, 0), L(e, He);
  var r = e.transitions;
  if (r !== null) for (const s of r) s.stop();
  Nt(e);
  var a = e.parent;
  a !== null && a.first !== null && It(e), e.next = e.prev = e.teardown = e.ctx = e.deps = e.fn = e.nodes_start = e.nodes_end = e.ac = null;
}
function On(e, t) {
  for (; e !== null; ) {
    var n = e === t ? null : D(e);
    e.remove(), e = n;
  }
}
function It(e) {
  var t = e.parent, n = e.prev, r = e.next;
  n !== null && (n.next = r), r !== null && (r.prev = n), t !== null && (t.first === e && (t.first = r), t.last === e && (t.last = n));
}
function In(e, t) {
  var n = [];
  kt(e, n, true), kn(n, () => {
    U(e), t && t();
  });
}
function kn(e, t) {
  var n = e.length;
  if (n > 0) {
    var r = () => --n || t();
    for (var a of e) a.out(r);
  } else t();
}
function kt(e, t, n) {
  if ((e.f & ee) === 0) {
    if (e.f ^= ee, e.transitions !== null) for (const o of e.transitions) (o.is_global || n) && t.push(o);
    for (var r = e.first; r !== null; ) {
      var a = r.next, s = (r.f & Ve) !== 0 || (r.f & q) !== 0;
      kt(r, t, s ? n : false), r = a;
    }
  }
}
function Vr(e) {
  Rt(e, true);
}
function Rt(e, t) {
  if ((e.f & ee) !== 0) {
    e.f ^= ee;
    for (var n = e.first; n !== null; ) {
      var r = n.next, a = (n.f & Ve) !== 0 || (n.f & q) !== 0;
      Rt(n, a ? t : false), n = r;
    }
    if (e.transitions !== null) for (const s of e.transitions) (s.is_global || t) && s.in();
  }
}
const Rn = typeof requestIdleCallback > "u" ? (e) => setTimeout(e, 1) : requestIdleCallback;
let ve = [], de = [];
function Ct() {
  var e = ve;
  ve = [], lt(e);
}
function Dt() {
  var e = de;
  de = [], lt(e);
}
function Pt(e) {
  ve.length === 0 && queueMicrotask(Ct), ve.push(e);
}
function Yr(e) {
  de.length === 0 && Rn(Dt), de.push(e);
}
function Cn() {
  ve.length > 0 && Ct(), de.length > 0 && Dt();
}
function Dn(e) {
  var t = h;
  if ((t.f & it) === 0) {
    if ((t.f & je) === 0) throw e;
    t.fn(e);
  } else Lt(e, t);
}
function Lt(e, t) {
  for (; t !== null; ) {
    if ((t.f & je) !== 0) try {
      t.b.error(e);
      return;
    } catch {
    }
    t = t.parent;
  }
  throw e;
}
let W = false, pe = null, K = false, ae = false;
function nt(e) {
  ae = e;
}
let fe = [];
let _ = null, R = false;
function P(e) {
  _ = e;
}
let h = null;
function $(e) {
  h = e;
}
let S = null;
function Mt(e) {
  _ !== null && _.f & Re && (S === null ? S = [_, [e]] : S[1].push(e));
}
let g = null, b = 0, x = null;
function Pn(e) {
  x = e;
}
let Ft = 1, Ee = 0, Y = false;
function qt() {
  return ++Ft;
}
function Oe(e) {
  var _a;
  var t = e.f;
  if ((t & B) !== 0) return true;
  if ((t & z) !== 0) {
    var n = e.deps, r = (t & T) !== 0;
    if (n !== null) {
      var a, s, o = (t & ge) !== 0, u = r && h !== null && !Y, l = n.length;
      if (o || u) {
        var i = e, v = i.parent;
        for (a = 0; a < l; a++) s = n[a], (o || !((_a = s == null ? void 0 : s.reactions) == null ? void 0 : _a.includes(i))) && (s.reactions ?? (s.reactions = [])).push(i);
        o && (i.f ^= ge), u && v !== null && (v.f & T) === 0 && (i.f ^= T);
      }
      for (a = 0; a < l; a++) if (s = n[a], Oe(s) && gt(s), s.wv > e.wv) return true;
    }
    (!r || h !== null && !Y) && L(e, O);
  }
  return false;
}
function jt(e, t, n = true) {
  var r = e.reactions;
  if (r !== null) for (var a = 0; a < r.length; a++) {
    var s = r[a];
    (S == null ? void 0 : S[1].includes(e)) && S[0] === _ || ((s.f & N) !== 0 ? jt(s, t, false) : t === s && (n ? L(s, B) : (s.f & O) !== 0 && L(s, z), Ke(s)));
  }
}
function Ht(e) {
  var _a;
  var t = g, n = b, r = x, a = _, s = Y, o = S, u = d, l = R, i = e.f;
  g = null, b = 0, x = null, Y = (i & T) !== 0 && (R || !K || _ === null), _ = (i & (q | te)) === 0 ? e : null, S = null, Qe(e.ctx), R = false, Ee++, e.f |= Re, e.ac !== null && (e.ac.abort(ct), e.ac = null);
  try {
    var v = (0, e.fn)(), c = e.deps;
    if (g !== null) {
      var f;
      if (me(e, b), c !== null && b > 0) for (c.length = b + g.length, f = 0; f < g.length; f++) c[b + f] = g[f];
      else e.deps = c = g;
      if (!Y || (i & N) !== 0 && e.reactions !== null) for (f = b; f < c.length; f++) ((_a = c[f]).reactions ?? (_a.reactions = [])).push(e);
    } else c !== null && b < c.length && (me(e, b), c.length = b);
    if (Ae() && x !== null && !R && c !== null && (e.f & (N | z | B)) === 0) for (f = 0; f < x.length; f++) jt(x[f], e);
    return a !== null && a !== e && (Ee++, x !== null && (r === null ? r = x : r.push(...x))), v;
  } catch (y) {
    Dn(y);
  } finally {
    g = t, b = n, x = r, _ = a, Y = s, S = o, Qe(u), R = l, e.f ^= Re;
  }
}
function Ln(e, t) {
  let n = t.reactions;
  if (n !== null) {
    var r = Gt.call(n, e);
    if (r !== -1) {
      var a = n.length - 1;
      a === 0 ? n = t.reactions = null : (n[r] = n[a], n.pop());
    }
  }
  n === null && (t.f & N) !== 0 && (g === null || !g.includes(t)) && (L(t, z), (t.f & (T | ge)) === 0 && (t.f ^= ge), yt(t), me(t, 0));
}
function me(e, t) {
  var n = e.deps;
  if (n !== null) for (var r = t; r < n.length; r++) Ln(e, n[r]);
}
function Ge(e) {
  var t = e.f;
  if ((t & He) === 0) {
    L(e, O);
    var n = h, r = K;
    h = e, K = true;
    try {
      (t & qe) !== 0 ? Nn(e) : Ot(e), Nt(e);
      var a = Ht(e);
      e.teardown = typeof a == "function" ? a : null, e.wv = Ft;
      var s;
    } finally {
      K = r, h = n;
    }
  }
}
function Mn() {
  try {
    nn();
  } catch (e) {
    if (pe !== null) Lt(e, pe);
    else throw e;
  }
}
function De() {
  var e = K;
  try {
    var t = 0;
    for (K = true; fe.length > 0; ) {
      t++ > 1e3 && Mn();
      var n = fe, r = n.length;
      fe = [];
      for (var a = 0; a < r; a++) {
        var s = qn(n[a]);
        Fn(s);
      }
      _e.clear();
    }
  } finally {
    W = false, K = e, pe = null;
  }
}
function Fn(e) {
  var t = e.length;
  if (t !== 0) for (var n = 0; n < t; n++) {
    var r = e[n];
    (r.f & (He | ee)) === 0 && Oe(r) && (Ge(r), r.deps === null && r.first === null && r.nodes_start === null && (r.teardown === null ? It(r) : r.fn = null));
  }
}
function Ke(e) {
  W || (W = true, queueMicrotask(De));
  for (var t = pe = e; t.parent !== null; ) {
    t = t.parent;
    var n = t.f;
    if ((n & (te | q)) !== 0) {
      if ((n & O) === 0) return;
      t.f ^= O;
    }
  }
  fe.push(t);
}
function qn(e) {
  for (var t = [], n = e; n !== null; ) {
    var r = n.f, a = (r & (q | te)) !== 0, s = a && (r & O) !== 0;
    if (!s && (r & ee) === 0) {
      (r & ut) !== 0 ? t.push(n) : a ? n.f ^= O : Oe(n) && Ge(n);
      var o = n.first;
      if (o !== null) {
        n = o;
        continue;
      }
    }
    var u = n.parent;
    for (n = n.next; n === null && u !== null; ) n = u.next, u = u.parent;
  }
  return t;
}
function Vt(e) {
  var t;
  for (e && (W = true, De(), W = true, t = e()); ; ) {
    if (Cn(), fe.length === 0) return W = false, pe = null, t;
    W = true, De();
  }
}
async function jn() {
  await Promise.resolve(), Vt();
}
function le(e) {
  var t = e.f, n = (t & N) !== 0;
  if (_ !== null && !R) {
    if (!(S == null ? void 0 : S[1].includes(e)) || S[0] !== _) {
      var r = _.deps;
      e.rv < Ee && (e.rv = Ee, g === null && r !== null && r[b] === e ? b++ : g === null ? g = [e] : (!Y || !g.includes(e)) && g.push(e));
    }
  } else if (n && e.deps === null && e.effects === null) {
    var a = e, s = a.parent;
    s !== null && (s.f & T) === 0 && (a.f ^= T);
  }
  return n && (a = e, Oe(a) && gt(a)), ae && _e.has(e) ? _e.get(e) : e.v;
}
function Ie(e) {
  var t = R;
  try {
    return R = true, e();
  } finally {
    R = t;
  }
}
const Hn = -7169;
function L(e, t) {
  e.f = e.f & Hn | t;
}
function Br(e) {
  if (!(typeof e != "object" || !e || e instanceof EventTarget)) {
    if (G in e) Pe(e);
    else if (!Array.isArray(e)) for (let t in e) {
      const n = e[t];
      typeof n == "object" && n && G in n && Pe(n);
    }
  }
}
function Pe(e, t = /* @__PURE__ */ new Set()) {
  if (typeof e == "object" && e !== null && !(e instanceof EventTarget) && !t.has(e)) {
    t.add(e), e instanceof Date && e.getTime();
    for (let r in e) try {
      Pe(e[r], t);
    } catch {
    }
    const n = st(e);
    if (n !== Object.prototype && n !== Array.prototype && n !== Map.prototype && n !== Set.prototype && n !== Date.prototype) {
      const r = Xt(n);
      for (let a in r) {
        const s = r[a].get;
        if (s) try {
          s.call(e);
        } catch {
        }
      }
    }
  }
}
function Ur(e) {
  return e.endsWith("capture") && e !== "gotpointercapture" && e !== "lostpointercapture";
}
const Vn = ["beforeinput", "click", "change", "dblclick", "contextmenu", "focusin", "focusout", "input", "keydown", "keyup", "mousedown", "mousemove", "mouseout", "mouseover", "mouseup", "pointerdown", "pointermove", "pointerout", "pointerover", "pointerup", "touchend", "touchmove", "touchstart"];
function $r(e) {
  return Vn.includes(e);
}
const Yn = { formnovalidate: "formNoValidate", ismap: "isMap", nomodule: "noModule", playsinline: "playsInline", readonly: "readOnly", defaultvalue: "defaultValue", defaultchecked: "defaultChecked", srcobject: "srcObject", novalidate: "noValidate", allowfullscreen: "allowFullscreen", disablepictureinpicture: "disablePictureInPicture", disableremoteplayback: "disableRemotePlayback" };
function Wr(e) {
  return e = e.toLowerCase(), Yn[e] ?? e;
}
const Bn = ["touchstart", "touchmove"];
function Un(e) {
  return Bn.includes(e);
}
function Gr(e, t) {
  if (t) {
    const n = document.body;
    e.autofocus = true, Pt(() => {
      document.activeElement === n && e.focus();
    });
  }
}
let rt = false;
function $n() {
  rt || (rt = true, document.addEventListener("reset", (e) => {
    Promise.resolve().then(() => {
      var _a;
      if (!e.defaultPrevented) for (const t of e.target.elements) (_a = t.__on_r) == null ? void 0 : _a.call(t);
    });
  }, { capture: true }));
}
function Kr(e, t, n, r = true) {
  r && n();
  for (var a of t) e.addEventListener(a, n);
  Ne(() => {
    for (var s of t) e.removeEventListener(s, n);
  });
}
function Yt(e) {
  var t = _, n = h;
  P(null), $(null);
  try {
    return e();
  } finally {
    P(t), $(n);
  }
}
function zr(e, t, n, r = n) {
  e.addEventListener(t, () => Yt(n));
  const a = e.__on_r;
  a ? e.__on_r = () => {
    a(), r(true);
  } : e.__on_r = () => r(true), $n();
}
const Bt = /* @__PURE__ */ new Set(), Le = /* @__PURE__ */ new Set();
function Wn(e, t, n, r = {}) {
  function a(s) {
    if (r.capture || ue.call(t, s), !s.cancelBubble) return Yt(() => n == null ? void 0 : n.call(this, s));
  }
  return e.startsWith("pointer") || e.startsWith("touch") || e === "wheel" ? Pt(() => {
    t.addEventListener(e, a, r);
  }) : t.addEventListener(e, a, r), a;
}
function Xr(e, t, n, r, a) {
  var s = { capture: r, passive: a }, o = Wn(e, t, n, s);
  (t === document.body || t === window || t === document || t instanceof HTMLMediaElement) && Ne(() => {
    t.removeEventListener(e, o, s);
  });
}
function Zr(e) {
  for (var t = 0; t < e.length; t++) Bt.add(e[t]);
  for (var n of Le) n(e);
}
function ue(e) {
  var _a;
  var t = this, n = t.ownerDocument, r = e.type, a = ((_a = e.composedPath) == null ? void 0 : _a.call(e)) || [], s = a[0] || e.target, o = 0, u = e.__root;
  if (u) {
    var l = a.indexOf(u);
    if (l !== -1 && (t === document || t === window)) {
      e.__root = t;
      return;
    }
    var i = a.indexOf(t);
    if (i === -1) return;
    l <= i && (o = l);
  }
  if (s = a[o] || e.target, s !== t) {
    zt(e, "currentTarget", { configurable: true, get() {
      return s || n;
    } });
    var v = _, c = h;
    P(null), $(null);
    try {
      for (var f, y = []; s !== null; ) {
        var E = s.assignedSlot || s.parentNode || s.host || null;
        try {
          var M = s["__" + r];
          if (M != null && (!s.disabled || e.target === s)) if (Fe(M)) {
            var [ye, ...we] = M;
            ye.apply(s, [e, ...we]);
          } else M.call(s, e);
        } catch (j) {
          f ? y.push(j) : f = j;
        }
        if (e.cancelBubble || E === t || E === null) break;
        s = E;
      }
      if (f) {
        for (let j of y) queueMicrotask(() => {
          throw j;
        });
        throw f;
      }
    } finally {
      e.__root = t, delete e.currentTarget, P(v), $(c);
    }
  }
}
let A;
function Gn() {
  A = void 0;
}
function Jr(e) {
  let t = null, n = w;
  var r;
  if (w) {
    for (t = p, A === void 0 && (A = C(document.head)); A !== null && (A.nodeType !== ce || A.data !== Ye); ) A = D(A);
    A === null ? J(false) : A = k(D(A));
  }
  w || (r = document.head.appendChild(F()));
  try {
    We(() => e(r), ot);
  } finally {
    n && (J(true), A = p, k(t));
  }
}
function ze(e) {
  var t = document.createElement("template");
  return t.innerHTML = e.replaceAll("<!>", "<!---->"), t.content;
}
function I(e, t) {
  var n = h;
  n.nodes_start === null && (n.nodes_start = e, n.nodes_end = t);
}
function Qr(e, t) {
  var n = (t & on) !== 0, r = (t & fn) !== 0, a, s = !e.startsWith("<!>");
  return () => {
    if (w) return I(p, null), p;
    a === void 0 && (a = ze(s ? e : "<!>" + e), n || (a = C(a)));
    var o = r || mt ? document.importNode(a, true) : a.cloneNode(true);
    if (n) {
      var u = C(o), l = o.lastChild;
      I(u, l);
    } else I(o, o);
    return o;
  };
}
function Kn(e, t, n = "svg") {
  var r = !e.startsWith("<!>"), a = `<${n}>${r ? e : "<!>" + e}</${n}>`, s;
  return () => {
    if (w) return I(p, null), p;
    if (!s) {
      var o = ze(a), u = C(o);
      s = C(u);
    }
    var l = s.cloneNode(true);
    return I(l, l), l;
  };
}
function ea(e, t) {
  return Kn(e, t, "svg");
}
function ta(e = "") {
  if (!w) {
    var t = F(e + "");
    return I(t, t), t;
  }
  var n = p;
  return n.nodeType !== Te && (n.before(n = F()), k(n)), I(n, n), n;
}
function na() {
  if (w) return I(p, null), p;
  var e = document.createDocumentFragment(), t = document.createComment(""), n = F();
  return e.append(t, n), I(t, n), e;
}
function ra(e, t) {
  if (w) {
    h.nodes_end = p, $e();
    return;
  }
  e !== null && e.before(t);
}
let at = true;
function aa(e, t) {
  var n = t == null ? "" : typeof t == "object" ? t + "" : t;
  n !== (e.__t ?? (e.__t = e.nodeValue)) && (e.__t = n, e.nodeValue = n + "");
}
function Ut(e, t) {
  return $t(e, t);
}
function zn(e, t) {
  Ce(), t.intro = t.intro ?? false;
  const n = t.target, r = w, a = p;
  try {
    for (var s = C(n); s && (s.nodeType !== ce || s.data !== Ye); ) s = D(s);
    if (!s) throw Q;
    J(true), k(s), $e();
    const o = $t(e, { ...t, anchor: s });
    if (p === null || p.nodeType !== ce || p.data !== ht) throw Se(), Q;
    return J(false), o;
  } catch (o) {
    if (o === Q) return t.recover === false && an(), Ce(), bn(n), J(false), Ut(e, t);
    throw o;
  } finally {
    J(r), k(a), Gn();
  }
}
const X = /* @__PURE__ */ new Map();
function $t(e, { target: t, anchor: n, props: r = {}, events: a, context: s, intro: o = true }) {
  Ce();
  var u = /* @__PURE__ */ new Set(), l = (c) => {
    for (var f = 0; f < c.length; f++) {
      var y = c[f];
      if (!u.has(y)) {
        u.add(y);
        var E = Un(y);
        t.addEventListener(y, ue, { passive: E });
        var M = X.get(y);
        M === void 0 ? (document.addEventListener(y, ue, { passive: E }), X.set(y, 1)) : X.set(y, M + 1);
      }
    }
  };
  l(Kt(Bt)), Le.add(l);
  var i = void 0, v = xn(() => {
    var c = n ?? t.appendChild(F());
    return St(() => {
      if (s) {
        hn({});
        var f = d;
        f.c = s;
      }
      a && (r.$$events = a), w && I(c, null), at = o, i = e(c, r) || {}, at = true, w && (h.nodes_end = p), s && yn();
    }), () => {
      var _a;
      for (var f of u) {
        t.removeEventListener(f, ue);
        var y = X.get(f);
        --y === 0 ? (document.removeEventListener(f, ue), X.delete(f)) : X.set(f, y);
      }
      Le.delete(l), c !== n && ((_a = c.parentNode) == null ? void 0 : _a.removeChild(c));
    };
  });
  return Me.set(i, v), i;
}
let Me = /* @__PURE__ */ new WeakMap();
function Xn(e, t) {
  const n = Me.get(e);
  return n ? (Me.delete(e), n(t)) : Promise.resolve();
}
function sa(e, t, ...n) {
  var r = e, a = oe, s;
  We(() => {
    a !== (a = t()) && (s && (U(s), s = null), s = St(() => a(r, ...n)));
  }, Ve), w && (r = p);
}
function Zn(e) {
  return (t, ...n) => {
    var _a;
    var r = e(...n), a;
    if (w) a = p, $e();
    else {
      var s = r.render().trim(), o = ze(s);
      a = C(o), t.before(a);
    }
    const u = (_a = r.setup) == null ? void 0 : _a.call(r, a);
    I(a, a), typeof u == "function" && Ne(u);
  };
}
function Jn(e, t, n) {
  if (e == null) return t(void 0), oe;
  const r = Ie(() => e.subscribe(t, n));
  return r.unsubscribe ? () => r.unsubscribe() : r;
}
const Z = [];
function la(e, t = oe) {
  let n = null;
  const r = /* @__PURE__ */ new Set();
  function a(u) {
    if (vt(e, u) && (e = u, n)) {
      const l = !Z.length;
      for (const i of r) i[1](), Z.push(i, e);
      if (l) {
        for (let i = 0; i < Z.length; i += 2) Z[i][0](Z[i + 1]);
        Z.length = 0;
      }
    }
  }
  function s(u) {
    a(u(e));
  }
  function o(u, l = oe) {
    const i = [u, l];
    return r.add(i), r.size === 1 && (n = t(a, s) || oe), u(e), () => {
      r.delete(i), r.size === 0 && n && (n(), n = null);
    };
  }
  return { set: a, update: s, subscribe: o };
}
function ua(e) {
  let t;
  return Jn(e, (n) => t = n)(), t;
}
function Qn() {
  return _ === null && rn(), (_.ac ?? (_.ac = new AbortController())).signal;
}
function Wt(e) {
  d === null && ne(), he && d.l !== null ? Xe(d).m.push(e) : An(() => {
    const t = Ie(e);
    if (typeof t == "function") return t;
  });
}
function er(e) {
  d === null && ne(), Wt(() => () => Ie(e));
}
function tr(e, t, { bubbles: n = false, cancelable: r = false } = {}) {
  return new CustomEvent(e, { detail: t, bubbles: n, cancelable: r });
}
function nr() {
  const e = d;
  return e === null && ne(), (t, n, r) => {
    var _a;
    const a = (_a = e.s.$$events) == null ? void 0 : _a[t];
    if (a) {
      const s = Fe(a) ? a.slice() : [a], o = tr(t, n, r);
      for (const u of s) u.call(e.x, o);
      return !o.defaultPrevented;
    }
    return true;
  };
}
function rr(e) {
  d === null && ne(), d.l === null && pt(), Xe(d).b.push(e);
}
function ar(e) {
  d === null && ne(), d.l === null && pt(), Xe(d).a.push(e);
}
function Xe(e) {
  var t = e.l;
  return t.u ?? (t.u = { a: [], b: [], m: [] });
}
const ia = Object.freeze(Object.defineProperty({ __proto__: null, afterUpdate: ar, beforeUpdate: rr, createEventDispatcher: nr, createRawSnippet: Zn, flushSync: Vt, getAbortSignal: Qn, getAllContexts: pn, getContext: _n, hasContext: dn, hydrate: zn, mount: Ut, onDestroy: er, onMount: Wt, setContext: vn, tick: jn, unmount: Xn, untrack: Ie }, Symbol.toStringTag, { value: "Module" }));
export {
  wr as $,
  k as A,
  J as B,
  Vr as C,
  St as D,
  Ve as E,
  In as F,
  p as G,
  Ye as H,
  xt as I,
  Sn as J,
  Pt as K,
  oe as L,
  kr as M,
  Jn as N,
  ua as O,
  Ne as P,
  zt as Q,
  V as R,
  G as S,
  ie as T,
  m as U,
  fr as V,
  gr as W,
  Ir as X,
  se as Y,
  Er as Z,
  he as _,
  An as a,
  ea as a$,
  yr as a0,
  mr as a1,
  ur as a2,
  zn as a3,
  Ut as a4,
  Vt as a5,
  Xn as a6,
  Wt as a7,
  H as a8,
  jn as a9,
  xr as aA,
  st as aB,
  Sr as aC,
  Xt as aD,
  Ur as aE,
  Wn as aF,
  Zr as aG,
  Gr as aH,
  Wr as aI,
  ir as aJ,
  $r as aK,
  Yr as aL,
  $n as aM,
  at as aN,
  qe as aO,
  it as aP,
  Ar as aQ,
  br as aR,
  Tr as aS,
  Yt as aT,
  sr as aU,
  P as aV,
  $ as aW,
  _ as aX,
  Ae as aY,
  sa as aZ,
  la as a_,
  na as aa,
  Or as ab,
  ta as ac,
  ce as ad,
  ht as ae,
  ee as af,
  Kt as ag,
  h as ah,
  En as ai,
  Ue as aj,
  Fe as ak,
  vr as al,
  kt as am,
  bn as an,
  kn as ao,
  U as ap,
  _r as aq,
  hr as ar,
  D as as,
  C as at,
  F as au,
  dr as av,
  pr as aw,
  Rr as ax,
  Nr as ay,
  zr as az,
  Ie as b,
  Xr as b0,
  Jr as b1,
  mn as b2,
  Dr as b3,
  dn as b4,
  _n as b5,
  vn as b6,
  Kr as b7,
  ia as b8,
  d as c,
  lr as d,
  cr as e,
  Br as f,
  le as g,
  Be as h,
  Qr as i,
  Fr as j,
  ra as k,
  yn as l,
  Mr as m,
  Cr as n,
  aa as o,
  hn as p,
  We as q,
  lt as r,
  qr as s,
  Hr as t,
  jr as u,
  w as v,
  $e as w,
  Lr as x,
  cn as y,
  Pr as z
};
