var hn = Array.isArray, yn = Array.prototype.indexOf, se = Array.from, ae = Object.defineProperty, $ = Object.getOwnPropertyDescriptor, wn = Object.getOwnPropertyDescriptors, En = Object.prototype, gn = Array.prototype, Mt = Object.getPrototypeOf, Nt = Object.isExtensible;
function le(t) {
  return typeof t == "function";
}
const et = () => {
};
function ie(t) {
  return t();
}
function Lt(t) {
  for (var n = 0; n < t.length; n++) t[n]();
}
const T = 2, qt = 4, _t = 8, bt = 16, R = 32, U = 64, st = 128, g = 256, at = 512, y = 1024, O = 2048, M = 4096, Y = 8192, vt = 16384, mn = 32768, jt = 65536, ue = 1 << 17, bn = 1 << 19, Yt = 1 << 20, Et = 1 << 21, k = Symbol("$state"), fe = Symbol("legacy props"), oe = Symbol("");
function Ht(t) {
  return t === this.v;
}
function Bt(t, n) {
  return t != t ? n == n : t !== n || t !== null && typeof t == "object" || typeof t == "function";
}
function Ut(t) {
  return !Bt(t, this.v);
}
function Tn(t) {
  throw new Error("https://svelte.dev/e/effect_in_teardown");
}
function xn() {
  throw new Error("https://svelte.dev/e/effect_in_unowned_derived");
}
function An(t) {
  throw new Error("https://svelte.dev/e/effect_orphan");
}
function In() {
  throw new Error("https://svelte.dev/e/effect_update_depth_exceeded");
}
function ce() {
  throw new Error("https://svelte.dev/e/hydration_failed");
}
function _e(t) {
  throw new Error("https://svelte.dev/e/props_invalid_value");
}
function On() {
  throw new Error("https://svelte.dev/e/state_descriptors_fixed");
}
function Rn() {
  throw new Error("https://svelte.dev/e/state_prototype_fixed");
}
function Nn() {
  throw new Error("https://svelte.dev/e/state_unsafe_mutation");
}
let W = false;
function ve() {
  W = true;
}
const pe = 1, de = 2, he = 4, ye = 8, we = 16, Ee = 1, ge = 2, me = 4, be = 8, Te = 16, xe = 1, Ae = 2, Ie = 4, Oe = 1, Re = 2, Dn = "[", Sn = "[!", kn = "]", Vt = {}, w = Symbol(), Ne = "http://www.w3.org/1999/xhtml";
function Gt(t) {
  throw new Error("https://svelte.dev/e/lifecycle_outside_component");
}
let p = null;
function Dt(t) {
  p = t;
}
function De(t) {
  return Tt().get(t);
}
function Se(t, n) {
  return Tt().set(t, n), n;
}
function ke(t) {
  return Tt().has(t);
}
function Ce(t, n = false, e) {
  var r = p = { p, c: null, d: false, e: null, m: false, s: t, x: null, l: null };
  W && !n && (p.l = { s: null, u: null, r1: [], r2: At(false) }), jn(() => {
    r.d = true;
  });
}
function Pe(t) {
  const n = p;
  if (n !== null) {
    const f = n.e;
    if (f !== null) {
      var e = d, r = _;
      n.e = null;
      try {
        for (var s = 0; s < f.length; s++) {
          var a = f[s];
          ft(a.effect), B(a.reaction), tn(a.fn);
        }
      } finally {
        ft(e), B(r);
      }
    }
    p = n.p, n.m = true;
  }
  return {};
}
function pt() {
  return !W || p !== null && p.l === null;
}
function Tt(t) {
  return p === null && Gt(), p.c ?? (p.c = new Map(Cn(p) || void 0));
}
function Cn(t) {
  let n = t.p;
  for (; n !== null; ) {
    const e = n.c;
    if (e !== null) return e;
    n = n.p;
  }
  return null;
}
function j(t) {
  if (typeof t != "object" || t === null || k in t) return t;
  const n = Mt(t);
  if (n !== En && n !== gn) return t;
  var e = /* @__PURE__ */ new Map(), r = hn(t), s = N(0), a = _, f = (u) => {
    var l = _;
    B(a);
    var i = u();
    return B(l), i;
  };
  return r && e.set("length", N(t.length)), new Proxy(t, { defineProperty(u, l, i) {
    (!("value" in i) || i.configurable === false || i.enumerable === false || i.writable === false) && On();
    var c = e.get(l);
    return c === void 0 ? (c = f(() => N(i.value)), e.set(l, c)) : D(c, f(() => j(i.value))), true;
  }, deleteProperty(u, l) {
    var i = e.get(l);
    if (i === void 0) l in u && (e.set(l, f(() => N(w))), wt(s));
    else {
      if (r && typeof l == "string") {
        var c = e.get("length"), o = Number(l);
        Number.isInteger(o) && o < c.v && D(c, o);
      }
      D(i, w), wt(s);
    }
    return true;
  }, get(u, l, i) {
    var _a;
    if (l === k) return t;
    var c = e.get(l), o = l in u;
    if (c === void 0 && (!o || ((_a = $(u, l)) == null ? void 0 : _a.writable)) && (c = f(() => N(j(o ? u[l] : w))), e.set(l, c)), c !== void 0) {
      var v = K(c);
      return v === w ? void 0 : v;
    }
    return Reflect.get(u, l, i);
  }, getOwnPropertyDescriptor(u, l) {
    var i = Reflect.getOwnPropertyDescriptor(u, l);
    if (i && "value" in i) {
      var c = e.get(l);
      c && (i.value = K(c));
    } else if (i === void 0) {
      var o = e.get(l), v = o == null ? void 0 : o.v;
      if (o !== void 0 && v !== w) return { enumerable: true, configurable: true, value: v, writable: true };
    }
    return i;
  }, has(u, l) {
    var _a;
    if (l === k) return true;
    var i = e.get(l), c = i !== void 0 && i.v !== w || Reflect.has(u, l);
    if (i !== void 0 || d !== null && (!c || ((_a = $(u, l)) == null ? void 0 : _a.writable))) {
      i === void 0 && (i = f(() => N(c ? j(u[l]) : w)), e.set(l, i));
      var o = K(i);
      if (o === w) return false;
    }
    return c;
  }, set(u, l, i, c) {
    var _a;
    var o = e.get(l), v = l in u;
    if (r && l === "length") for (var G = i; G < o.v; G += 1) {
      var nt = e.get(G + "");
      nt !== void 0 ? D(nt, w) : G in u && (nt = f(() => N(w)), e.set(G + "", nt));
    }
    o === void 0 ? (!v || ((_a = $(u, l)) == null ? void 0 : _a.writable)) && (o = f(() => N(void 0)), D(o, f(() => j(i))), e.set(l, o)) : (v = o.v !== w, D(o, f(() => j(i))));
    var Ot = Reflect.getOwnPropertyDescriptor(u, l);
    if ((Ot == null ? void 0 : Ot.set) && Ot.set.call(c, i), !v) {
      if (r && typeof l == "string") {
        var Rt = e.get("length"), yt = Number(l);
        Number.isInteger(yt) && yt >= Rt.v && D(Rt, yt + 1);
      }
      wt(s);
    }
    return true;
  }, ownKeys(u) {
    K(s);
    var l = Reflect.ownKeys(u).filter((o) => {
      var v = e.get(o);
      return v === void 0 || v.v !== w;
    });
    for (var [i, c] of e) c.v !== w && !(i in u) && l.push(i);
    return l;
  }, setPrototypeOf() {
    Rn();
  } });
}
function wt(t, n = 1) {
  D(t, t.v + n);
}
function St(t) {
  try {
    if (t !== null && typeof t == "object" && k in t) return t[k];
  } catch {
  }
  return t;
}
function Fe(t, n) {
  return Object.is(St(t), St(n));
}
function xt(t) {
  var n = T | O, e = _ !== null && (_.f & T) !== 0 ? _ : null;
  return d === null || e !== null && (e.f & g) !== 0 ? n |= g : d.f |= Yt, { ctx: p, deps: null, effects: null, equals: Ht, f: n, fn: t, reactions: null, rv: 0, v: null, wv: 0, parent: e ?? d };
}
function Me(t) {
  const n = xt(t);
  return fn(n), n;
}
function Le(t) {
  const n = xt(t);
  return n.equals = Ut, n;
}
function Kt(t) {
  var n = t.effects;
  if (n !== null) {
    t.effects = null;
    for (var e = 0; e < n.length; e += 1) F(n[e]);
  }
}
function Pn(t) {
  for (var n = t.parent; n !== null; ) {
    if ((n.f & T) === 0) return n;
    n = n.parent;
  }
  return null;
}
function $t(t) {
  var n, e = d;
  ft(Pn(t));
  try {
    Kt(t), n = vn(t);
  } finally {
    ft(e);
  }
  return n;
}
function zt(t) {
  var n = $t(t), e = (S || (t.f & g) !== 0) && t.deps !== null ? M : y;
  x(t, e), t.equals(n) || (t.v = n, t.wv = cn());
}
const Z = /* @__PURE__ */ new Map();
function At(t, n) {
  var e = { f: 0, v: t, reactions: null, equals: Ht, rv: 0, wv: 0 };
  return e;
}
function N(t, n) {
  const e = At(t);
  return fn(e), e;
}
function qe(t, n = false) {
  var _a;
  const e = At(t);
  return n || (e.equals = Ut), W && p !== null && p.l !== null && ((_a = p.l).s ?? (_a.s = [])).push(e), e;
}
function D(t, n, e = false) {
  _ !== null && !A && pt() && (_.f & (T | bt)) !== 0 && !(I == null ? void 0 : I.includes(t)) && Nn();
  let r = e ? j(n) : n;
  return Fn(t, r);
}
function Fn(t, n) {
  if (!t.equals(n)) {
    var e = t.v;
    X ? Z.set(t, n) : Z.set(t, e), t.v = n, (t.f & T) !== 0 && ((t.f & O) !== 0 && $t(t), x(t, (t.f & g) === 0 ? y : M)), t.wv = cn(), Zt(t, O), pt() && d !== null && (d.f & y) !== 0 && (d.f & (R | U)) === 0 && (b === null ? zn([t]) : b.push(t));
  }
  return n;
}
function Zt(t, n) {
  var e = t.reactions;
  if (e !== null) for (var r = pt(), s = e.length, a = 0; a < s; a++) {
    var f = e[a], u = f.f;
    (u & O) === 0 && (!r && f === d || (x(f, n), (u & (y | g)) !== 0 && ((u & T) !== 0 ? Zt(f, M) : ht(f))));
  }
}
function Jt(t) {
  console.warn("https://svelte.dev/e/hydration_mismatch");
}
let P = false;
function je(t) {
  P = t;
}
let m;
function H(t) {
  if (t === null) throw Jt(), Vt;
  return m = t;
}
function Ye() {
  return H(L(m));
}
function He(t) {
  if (P) {
    if (L(m) !== null) throw Jt(), Vt;
    m = t;
  }
}
function Be(t = 1) {
  if (P) {
    for (var n = t, e = m; n--; ) e = L(e);
    m = e;
  }
}
function Ue() {
  for (var t = 0, n = m; ; ) {
    if (n.nodeType === 8) {
      var e = n.data;
      if (e === kn) {
        if (t === 0) return n;
        t -= 1;
      } else (e === Dn || e === Sn) && (t += 1);
    }
    var r = L(n);
    n.remove(), n = r;
  }
}
var kt, Mn, Ln, Qt, Wt;
function Ve() {
  if (kt === void 0) {
    kt = window, Mn = document, Ln = /Firefox/.test(navigator.userAgent);
    var t = Element.prototype, n = Node.prototype, e = Text.prototype;
    Qt = $(n, "firstChild").get, Wt = $(n, "nextSibling").get, Nt(t) && (t.__click = void 0, t.__className = void 0, t.__attributes = null, t.__style = void 0, t.__e = void 0), Nt(e) && (e.__t = void 0);
  }
}
function lt(t = "") {
  return document.createTextNode(t);
}
function gt(t) {
  return Qt.call(t);
}
function L(t) {
  return Wt.call(t);
}
function Ge(t, n) {
  if (!P) return gt(t);
  var e = gt(m);
  if (e === null) e = m.appendChild(lt());
  else if (n && e.nodeType !== 3) {
    var r = lt();
    return e == null ? void 0 : e.before(r), H(r), r;
  }
  return H(e), e;
}
function Ke(t, n) {
  if (!P) {
    var e = gt(t);
    return e instanceof Comment && e.data === "" ? L(e) : e;
  }
  if (n && (m == null ? void 0 : m.nodeType) !== 3) {
    var r = lt();
    return m == null ? void 0 : m.before(r), H(r), r;
  }
  return m;
}
function $e(t, n = 1, e = false) {
  let r = P ? m : t;
  for (var s; n--; ) s = r, r = L(r);
  if (!P) return r;
  var a = r == null ? void 0 : r.nodeType;
  if (e && a !== 3) {
    var f = lt();
    return r === null ? s == null ? void 0 : s.after(f) : r.before(f), H(f), f;
  }
  return H(r), r;
}
function ze(t) {
  t.textContent = "";
}
function Xt(t) {
  d === null && _ === null && An(), _ !== null && (_.f & g) !== 0 && d === null && xn(), X && Tn();
}
function qn(t, n) {
  var e = n.last;
  e === null ? n.last = n.first = t : (e.next = t, t.prev = e, n.last = t);
}
function V(t, n, e, r = true) {
  var s = d, a = { ctx: p, deps: null, nodes_start: null, nodes_end: null, f: t | O, first: null, fn: n, last: null, next: null, parent: s, prev: null, teardown: null, transitions: null, wv: 0 };
  if (e) try {
    It(a), a.f |= mn;
  } catch (l) {
    throw F(a), l;
  }
  else n !== null && ht(a);
  var f = e && a.deps === null && a.first === null && a.nodes_start === null && a.teardown === null && (a.f & (Yt | st)) === 0;
  if (!f && r && (s !== null && qn(a, s), _ !== null && (_.f & T) !== 0)) {
    var u = _;
    (u.effects ?? (u.effects = [])).push(a);
  }
  return a;
}
function jn(t) {
  const n = V(_t, null, false);
  return x(n, y), n.teardown = t, n;
}
function Yn(t) {
  Xt();
  var n = d !== null && (d.f & R) !== 0 && p !== null && !p.m;
  if (n) {
    var e = p;
    (e.e ?? (e.e = [])).push({ fn: t, effect: d, reaction: _ });
  } else {
    var r = tn(t);
    return r;
  }
}
function Ze(t) {
  return Xt(), Hn(t);
}
function Je(t) {
  const n = V(U, t, true);
  return (e = {}) => new Promise((r) => {
    e.outro ? Gn(n, () => {
      F(n), r(void 0);
    }) : (F(n), r(void 0));
  });
}
function tn(t) {
  return V(qt, t, false);
}
function Hn(t) {
  return V(_t, t, true);
}
function Qe(t, n = [], e = xt) {
  const r = n.map(e);
  return Bn(() => t(...r.map(K)));
}
function Bn(t, n = 0) {
  return V(_t | bt | n, t, true);
}
function We(t, n = true) {
  return V(_t | R, t, true, n);
}
function nn(t) {
  var n = t.teardown;
  if (n !== null) {
    const e = X, r = _;
    Pt(true), B(null);
    try {
      n.call(null);
    } finally {
      Pt(e), B(r);
    }
  }
}
function en(t, n = false) {
  var e = t.first;
  for (t.first = t.last = null; e !== null; ) {
    var r = e.next;
    (e.f & U) !== 0 ? e.parent = null : F(e, n), e = r;
  }
}
function Un(t) {
  for (var n = t.first; n !== null; ) {
    var e = n.next;
    (n.f & R) === 0 && F(n), n = e;
  }
}
function F(t, n = true) {
  var e = false;
  (n || (t.f & bn) !== 0) && t.nodes_start !== null && (Vn(t.nodes_start, t.nodes_end), e = true), en(t, n && !e), ct(t, 0), x(t, vt);
  var r = t.transitions;
  if (r !== null) for (const a of r) a.stop();
  nn(t);
  var s = t.parent;
  s !== null && s.first !== null && rn(t), t.next = t.prev = t.teardown = t.ctx = t.deps = t.fn = t.nodes_start = t.nodes_end = null;
}
function Vn(t, n) {
  for (; t !== null; ) {
    var e = t === n ? null : L(t);
    t.remove(), t = e;
  }
}
function rn(t) {
  var n = t.parent, e = t.prev, r = t.next;
  e !== null && (e.next = r), r !== null && (r.prev = e), n !== null && (n.first === t && (n.first = r), n.last === t && (n.last = e));
}
function Gn(t, n) {
  var e = [];
  sn(t, e, true), Kn(e, () => {
    F(t), n && n();
  });
}
function Kn(t, n) {
  var e = t.length;
  if (e > 0) {
    var r = () => --e || n();
    for (var s of t) s.out(r);
  } else n();
}
function sn(t, n, e) {
  if ((t.f & Y) === 0) {
    if (t.f ^= Y, t.transitions !== null) for (const f of t.transitions) (f.is_global || e) && n.push(f);
    for (var r = t.first; r !== null; ) {
      var s = r.next, a = (r.f & jt) !== 0 || (r.f & R) !== 0;
      sn(r, n, a ? e : false), r = s;
    }
  }
}
function Xe(t) {
  an(t, true);
}
function an(t, n) {
  if ((t.f & Y) !== 0) {
    t.f ^= Y, (t.f & y) === 0 && (t.f ^= y), tt(t) && (x(t, O), ht(t));
    for (var e = t.first; e !== null; ) {
      var r = e.next, s = (e.f & jt) !== 0 || (e.f & R) !== 0;
      an(e, s ? n : false), e = r;
    }
    if (t.transitions !== null) for (const a of t.transitions) (a.is_global || n) && a.in();
  }
}
const $n = typeof requestIdleCallback > "u" ? (t) => setTimeout(t, 1) : requestIdleCallback;
let J = [], Q = [];
function ln() {
  var t = J;
  J = [], Lt(t);
}
function un() {
  var t = Q;
  Q = [], Lt(t);
}
function tr(t) {
  J.length === 0 && queueMicrotask(ln), J.push(t);
}
function nr(t) {
  Q.length === 0 && $n(un), Q.push(t);
}
function Ct() {
  J.length > 0 && ln(), Q.length > 0 && un();
}
let rt = false, it = false, ut = null, C = false, X = false;
function Pt(t) {
  X = t;
}
let z = [];
let _ = null, A = false;
function B(t) {
  _ = t;
}
let d = null;
function ft(t) {
  d = t;
}
let I = null;
function fn(t) {
  _ !== null && _.f & Et && (I === null ? I = [t] : I.push(t));
}
let h = null, E = 0, b = null;
function zn(t) {
  b = t;
}
let on = 1, ot = 0, S = false;
function cn() {
  return ++on;
}
function tt(t) {
  var _a;
  var n = t.f;
  if ((n & O) !== 0) return true;
  if ((n & M) !== 0) {
    var e = t.deps, r = (n & g) !== 0;
    if (e !== null) {
      var s, a, f = (n & at) !== 0, u = r && d !== null && !S, l = e.length;
      if (f || u) {
        var i = t, c = i.parent;
        for (s = 0; s < l; s++) a = e[s], (f || !((_a = a == null ? void 0 : a.reactions) == null ? void 0 : _a.includes(i))) && (a.reactions ?? (a.reactions = [])).push(i);
        f && (i.f ^= at), u && c !== null && (c.f & g) === 0 && (i.f ^= g);
      }
      for (s = 0; s < l; s++) if (a = e[s], tt(a) && zt(a), a.wv > t.wv) return true;
    }
    (!r || d !== null && !S) && x(t, y);
  }
  return false;
}
function Zn(t, n) {
  for (var e = n; e !== null; ) {
    if ((e.f & st) !== 0) try {
      e.fn(t);
      return;
    } catch {
      e.f ^= st;
    }
    e = e.parent;
  }
  throw rt = false, t;
}
function Ft(t) {
  return (t.f & vt) === 0 && (t.parent === null || (t.parent.f & st) === 0);
}
function dt(t, n, e, r) {
  if (rt) {
    if (e === null && (rt = false), Ft(n)) throw t;
    return;
  }
  if (e !== null && (rt = true), Zn(t, n), Ft(n)) throw t;
}
function _n(t, n, e = true) {
  var r = t.reactions;
  if (r !== null) for (var s = 0; s < r.length; s++) {
    var a = r[s];
    (I == null ? void 0 : I.includes(t)) || ((a.f & T) !== 0 ? _n(a, n, false) : n === a && (e ? x(a, O) : (a.f & y) !== 0 && x(a, M), ht(a)));
  }
}
function vn(t) {
  var _a;
  var n = h, e = E, r = b, s = _, a = S, f = I, u = p, l = A, i = t.f;
  h = null, E = 0, b = null, S = (i & g) !== 0 && (A || !C || _ === null), _ = (i & (R | U)) === 0 ? t : null, I = null, Dt(t.ctx), A = false, ot++, t.f |= Et;
  try {
    var c = (0, t.fn)(), o = t.deps;
    if (h !== null) {
      var v;
      if (ct(t, E), o !== null && E > 0) for (o.length = E + h.length, v = 0; v < h.length; v++) o[E + v] = h[v];
      else t.deps = o = h;
      if (!S) for (v = E; v < o.length; v++) ((_a = o[v]).reactions ?? (_a.reactions = [])).push(t);
    } else o !== null && E < o.length && (ct(t, E), o.length = E);
    if (pt() && b !== null && !A && o !== null && (t.f & (T | M | O)) === 0) for (v = 0; v < b.length; v++) _n(b[v], t);
    return s !== null && s !== t && (ot++, b !== null && (r === null ? r = b : r.push(...b))), c;
  } finally {
    h = n, E = e, b = r, _ = s, S = a, I = f, Dt(u), A = l, t.f ^= Et;
  }
}
function Jn(t, n) {
  let e = n.reactions;
  if (e !== null) {
    var r = yn.call(e, t);
    if (r !== -1) {
      var s = e.length - 1;
      s === 0 ? e = n.reactions = null : (e[r] = e[s], e.pop());
    }
  }
  e === null && (n.f & T) !== 0 && (h === null || !h.includes(n)) && (x(n, M), (n.f & (g | at)) === 0 && (n.f ^= at), Kt(n), ct(n, 0));
}
function ct(t, n) {
  var e = t.deps;
  if (e !== null) for (var r = n; r < e.length; r++) Jn(t, e[r]);
}
function It(t) {
  var n = t.f;
  if ((n & vt) === 0) {
    x(t, y);
    var e = d, r = p, s = C;
    d = t, C = true;
    try {
      (n & bt) !== 0 ? Un(t) : en(t), nn(t);
      var a = vn(t);
      t.teardown = typeof a == "function" ? a : null, t.wv = on;
      var f = t.deps, u;
    } catch (l) {
      dt(l, t, e, r || t.ctx);
    } finally {
      C = s, d = e;
    }
  }
}
function Qn() {
  try {
    In();
  } catch (t) {
    if (ut !== null) dt(t, ut, null);
    else throw t;
  }
}
function pn() {
  var t = C;
  try {
    var n = 0;
    for (C = true; z.length > 0; ) {
      n++ > 1e3 && Qn();
      var e = z, r = e.length;
      z = [];
      for (var s = 0; s < r; s++) {
        var a = Xn(e[s]);
        Wn(a);
      }
      Z.clear();
    }
  } finally {
    it = false, C = t, ut = null;
  }
}
function Wn(t) {
  var n = t.length;
  if (n !== 0) for (var e = 0; e < n; e++) {
    var r = t[e];
    if ((r.f & (vt | Y)) === 0) try {
      tt(r) && (It(r), r.deps === null && r.first === null && r.nodes_start === null && (r.teardown === null ? rn(r) : r.fn = null));
    } catch (s) {
      dt(s, r, null, r.ctx);
    }
  }
}
function ht(t) {
  it || (it = true, queueMicrotask(pn));
  for (var n = ut = t; n.parent !== null; ) {
    n = n.parent;
    var e = n.f;
    if ((e & (U | R)) !== 0) {
      if ((e & y) === 0) return;
      n.f ^= y;
    }
  }
  z.push(n);
}
function Xn(t) {
  for (var n = [], e = t; e !== null; ) {
    var r = e.f, s = (r & (R | U)) !== 0, a = s && (r & y) !== 0;
    if (!a && (r & Y) === 0) {
      if ((r & qt) !== 0) n.push(e);
      else if (s) e.f ^= y;
      else {
        var f = _;
        try {
          _ = e, tt(e) && It(e);
        } catch (i) {
          dt(i, e, null, e.ctx);
        } finally {
          _ = f;
        }
      }
      var u = e.first;
      if (u !== null) {
        e = u;
        continue;
      }
    }
    var l = e.parent;
    for (e = e.next; e === null && l !== null; ) e = l.next, l = l.parent;
  }
  return n;
}
function te(t) {
  var n;
  for (Ct(); z.length > 0; ) it = true, pn(), Ct();
  return n;
}
async function er() {
  await Promise.resolve(), te();
}
function K(t) {
  var n = t.f, e = (n & T) !== 0;
  if (_ !== null && !A) {
    if (!(I == null ? void 0 : I.includes(t))) {
      var r = _.deps;
      t.rv < ot && (t.rv = ot, h === null && r !== null && r[E] === t ? E++ : h === null ? h = [t] : (!S || !h.includes(t)) && h.push(t));
    }
  } else if (e && t.deps === null && t.effects === null) {
    var s = t, a = s.parent;
    a !== null && (a.f & g) === 0 && (s.f ^= g);
  }
  return e && (s = t, tt(s) && zt(s)), X && Z.has(t) ? Z.get(t) : t.v;
}
function dn(t) {
  var n = A;
  try {
    return A = true, t();
  } finally {
    A = n;
  }
}
const ne = -7169;
function x(t, n) {
  t.f = t.f & ne | n;
}
function rr(t) {
  if (!(typeof t != "object" || !t || t instanceof EventTarget)) {
    if (k in t) mt(t);
    else if (!Array.isArray(t)) for (let n in t) {
      const e = t[n];
      typeof e == "object" && e && k in e && mt(e);
    }
  }
}
function mt(t, n = /* @__PURE__ */ new Set()) {
  if (typeof t == "object" && t !== null && !(t instanceof EventTarget) && !n.has(t)) {
    n.add(t), t instanceof Date && t.getTime();
    for (let r in t) try {
      mt(t[r], n);
    } catch {
    }
    const e = Mt(t);
    if (e !== Object.prototype && e !== Array.prototype && e !== Map.prototype && e !== Set.prototype && e !== Date.prototype) {
      const r = wn(e);
      for (let s in r) {
        const a = r[s].get;
        if (a) try {
          a.call(t);
        } catch {
        }
      }
    }
  }
}
function ee(t, n, e) {
  if (t == null) return n(void 0), et;
  const r = dn(() => t.subscribe(n, e));
  return r.unsubscribe ? () => r.unsubscribe() : r;
}
const q = [];
function sr(t, n = et) {
  let e = null;
  const r = /* @__PURE__ */ new Set();
  function s(u) {
    if (Bt(t, u) && (t = u, e)) {
      const l = !q.length;
      for (const i of r) i[1](), q.push(i, t);
      if (l) {
        for (let i = 0; i < q.length; i += 2) q[i][0](q[i + 1]);
        q.length = 0;
      }
    }
  }
  function a(u) {
    s(u(t));
  }
  function f(u, l = et) {
    const i = [u, l];
    return r.add(i), r.size === 1 && (e = n(s, a) || et), u(t), () => {
      r.delete(i), r.size === 0 && e && (e(), e = null);
    };
  }
  return { set: s, update: a, subscribe: f };
}
function ar(t) {
  let n;
  return ee(t, (e) => n = e)(), n;
}
function lr(t) {
  p === null && Gt(), W && p.l !== null ? re(p).m.push(t) : Yn(() => {
    const n = dn(t);
    if (typeof n == "function") return n;
  });
}
function re(t) {
  var n = t.l;
  return n.u ?? (n.u = { a: [], b: [], m: [] });
}
export {
  Ee as $,
  Gn as A,
  m as B,
  tn as C,
  Hn as D,
  jt as E,
  tr as F,
  et as G,
  Dn as H,
  qe as I,
  ee as J,
  ar as K,
  jn as L,
  ae as M,
  D as N,
  $ as O,
  _e as P,
  ue as Q,
  Le as R,
  k as S,
  me as T,
  w as U,
  Ut as V,
  j as W,
  be as X,
  fe as Y,
  W as Z,
  ge as _,
  Yn as a,
  Te as a0,
  te as a1,
  lr as a2,
  N as a3,
  er as a4,
  Me as a5,
  B as a6,
  ft as a7,
  _ as a8,
  d as a9,
  ye as aA,
  Ne as aB,
  Mt as aC,
  wn as aD,
  oe as aE,
  nr as aF,
  bt as aG,
  mn as aH,
  Ie as aI,
  xe as aJ,
  Ae as aK,
  le as aL,
  pt as aM,
  sr as aN,
  Mn as aO,
  Be as aP,
  ke as aQ,
  De as aR,
  Se as aS,
  Fe as aT,
  hn as aa,
  lt as ab,
  bn as ac,
  L as ad,
  gt as ae,
  Ln as af,
  Oe as ag,
  Re as ah,
  Ve as ai,
  Vt as aj,
  kn as ak,
  Jt as al,
  ce as am,
  ze as an,
  se as ao,
  Je as ap,
  Y as aq,
  Fn as ar,
  At as as,
  de as at,
  sn as au,
  Kn as av,
  F as aw,
  pe as ax,
  we as ay,
  he as az,
  dn as b,
  p as c,
  ie as d,
  ve as e,
  rr as f,
  K as g,
  xt as h,
  Ke as i,
  Pe as j,
  Ge as k,
  He as l,
  Bn as m,
  P as n,
  Ye as o,
  Ce as p,
  Sn as q,
  Lt as r,
  $e as s,
  Qe as t,
  Ze as u,
  Ue as v,
  H as w,
  je as x,
  Xe as y,
  We as z
};
