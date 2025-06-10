var dn = Array.isArray, pn = Array.prototype.indexOf, ee = Array.from, re = Object.defineProperty, z = Object.getOwnPropertyDescriptor, hn = Object.getOwnPropertyDescriptors, yn = Object.prototype, wn = Array.prototype, Pt = Object.getPrototypeOf, Nt = Object.isExtensible;
function se(t) {
  return typeof t == "function";
}
const et = () => {
};
function ae(t) {
  return t();
}
function Mt(t) {
  for (var n = 0; n < t.length; n++) t[n]();
}
const T = 2, Ft = 4, ot = 8, wt = 16, R = 32, V = 64, Et = 128, g = 256, rt = 512, y = 1024, O = 2048, L = 4096, H = 8192, gt = 16384, Lt = 32768, qt = 65536, le = 1 << 17, En = 1 << 19, jt = 1 << 20, pt = 1 << 21, C = Symbol("$state"), ie = Symbol("legacy props"), ue = Symbol("");
function Yt(t) {
  return t === this.v;
}
function Ht(t, n) {
  return t != t ? n == n : t !== n || t !== null && typeof t == "object" || typeof t == "function";
}
function Bt(t) {
  return !Ht(t, this.v);
}
function gn(t) {
  throw new Error("https://svelte.dev/e/effect_in_teardown");
}
function mn() {
  throw new Error("https://svelte.dev/e/effect_in_unowned_derived");
}
function bn(t) {
  throw new Error("https://svelte.dev/e/effect_orphan");
}
function Tn() {
  throw new Error("https://svelte.dev/e/effect_update_depth_exceeded");
}
function fe() {
  throw new Error("https://svelte.dev/e/hydration_failed");
}
function oe(t) {
  throw new Error("https://svelte.dev/e/props_invalid_value");
}
function An() {
  throw new Error("https://svelte.dev/e/state_descriptors_fixed");
}
function xn() {
  throw new Error("https://svelte.dev/e/state_prototype_fixed");
}
function In() {
  throw new Error("https://svelte.dev/e/state_unsafe_mutation");
}
let X = false;
function ce() {
  X = true;
}
const _e = 1, ve = 2, de = 4, pe = 8, he = 16, ye = 1, we = 2, Ee = 4, ge = 8, me = 16, be = 1, Te = 2, Ae = 4, xe = 1, Ie = 2, On = "[", Rn = "[!", Nn = "]", mt = {}, w = Symbol(), Oe = "http://www.w3.org/1999/xhtml", Re = "@attach";
function Ut(t) {
  throw new Error("https://svelte.dev/e/lifecycle_outside_component");
}
let d = null;
function Dt(t) {
  d = t;
}
function Ne(t) {
  return bt().get(t);
}
function De(t, n) {
  return bt().set(t, n), n;
}
function Se(t) {
  return bt().has(t);
}
function ke(t, n = false, e) {
  var r = d = { p: d, c: null, d: false, e: null, m: false, s: t, x: null, l: null };
  X && !n && (d.l = { s: null, u: null, r1: [], r2: At(false) }), Fn(() => {
    r.d = true;
  });
}
function Ce(t) {
  const n = d;
  if (n !== null) {
    const f = n.e;
    if (f !== null) {
      var e = p, r = v;
      n.e = null;
      try {
        for (var s = 0; s < f.length; s++) {
          var a = f[s];
          it(a.effect), U(a.reaction), Jt(a.fn);
        }
      } finally {
        it(e), U(r);
      }
    }
    d = n.p, n.m = true;
  }
  return {};
}
function ct() {
  return !X || d !== null && d.l === null;
}
function bt(t) {
  return d === null && Ut(), d.c ?? (d.c = new Map(Dn(d) || void 0));
}
function Dn(t) {
  let n = t.p;
  for (; n !== null; ) {
    const e = n.c;
    if (e !== null) return e;
    n = n.p;
  }
  return null;
}
function Y(t) {
  if (typeof t != "object" || t === null || C in t) return t;
  const n = Pt(t);
  if (n !== yn && n !== wn) return t;
  var e = /* @__PURE__ */ new Map(), r = dn(t), s = N(0), a = v, f = (u) => {
    var i = v;
    U(a);
    var l = u();
    return U(i), l;
  };
  return r && e.set("length", N(t.length)), new Proxy(t, { defineProperty(u, i, l) {
    (!("value" in l) || l.configurable === false || l.enumerable === false || l.writable === false) && An();
    var c = e.get(i);
    return c === void 0 ? (c = f(() => N(l.value)), e.set(i, c)) : D(c, f(() => Y(l.value))), true;
  }, deleteProperty(u, i) {
    var l = e.get(i);
    if (l === void 0) i in u && (e.set(i, f(() => N(w))), dt(s));
    else {
      if (r && typeof i == "string") {
        var c = e.get("length"), o = Number(i);
        Number.isInteger(o) && o < c.v && D(c, o);
      }
      D(l, w), dt(s);
    }
    return true;
  }, get(u, i, l) {
    var _a;
    if (i === C) return t;
    var c = e.get(i), o = i in u;
    if (c === void 0 && (!o || ((_a = z(u, i)) == null ? void 0 : _a.writable)) && (c = f(() => N(Y(o ? u[i] : w))), e.set(i, c)), c !== void 0) {
      var _ = $(c);
      return _ === w ? void 0 : _;
    }
    return Reflect.get(u, i, l);
  }, getOwnPropertyDescriptor(u, i) {
    var l = Reflect.getOwnPropertyDescriptor(u, i);
    if (l && "value" in l) {
      var c = e.get(i);
      c && (l.value = $(c));
    } else if (l === void 0) {
      var o = e.get(i), _ = o == null ? void 0 : o.v;
      if (o !== void 0 && _ !== w) return { enumerable: true, configurable: true, value: _, writable: true };
    }
    return l;
  }, has(u, i) {
    var _a;
    if (i === C) return true;
    var l = e.get(i), c = l !== void 0 && l.v !== w || Reflect.has(u, i);
    if (l !== void 0 || p !== null && (!c || ((_a = z(u, i)) == null ? void 0 : _a.writable))) {
      l === void 0 && (l = f(() => N(c ? Y(u[i]) : w)), e.set(i, l));
      var o = $(l);
      if (o === w) return false;
    }
    return c;
  }, set(u, i, l, c) {
    var _a;
    var o = e.get(i), _ = i in u;
    if (r && i === "length") for (var k = l; k < o.v; k += 1) {
      var nt = e.get(k + "");
      nt !== void 0 ? D(nt, w) : k in u && (nt = f(() => N(w)), e.set(k + "", nt));
    }
    o === void 0 ? (!_ || ((_a = z(u, i)) == null ? void 0 : _a.writable)) && (o = f(() => N(void 0)), D(o, f(() => Y(l))), e.set(i, o)) : (_ = o.v !== w, D(o, f(() => Y(l))));
    var Ot = Reflect.getOwnPropertyDescriptor(u, i);
    if ((Ot == null ? void 0 : Ot.set) && Ot.set.call(c, l), !_) {
      if (r && typeof i == "string") {
        var Rt = e.get("length"), vt = Number(i);
        Number.isInteger(vt) && vt >= Rt.v && D(Rt, vt + 1);
      }
      dt(s);
    }
    return true;
  }, ownKeys(u) {
    $(s);
    var i = Reflect.ownKeys(u).filter((o) => {
      var _ = e.get(o);
      return _ === void 0 || _.v !== w;
    });
    for (var [l, c] of e) c.v !== w && !(l in u) && i.push(l);
    return i;
  }, setPrototypeOf() {
    xn();
  } });
}
function dt(t, n = 1) {
  D(t, t.v + n);
}
function St(t) {
  try {
    if (t !== null && typeof t == "object" && C in t) return t[C];
  } catch {
  }
  return t;
}
function Pe(t, n) {
  return Object.is(St(t), St(n));
}
function Tt(t) {
  var n = T | O, e = v !== null && (v.f & T) !== 0 ? v : null;
  return p === null || e !== null && (e.f & g) !== 0 ? n |= g : p.f |= jt, { ctx: d, deps: null, effects: null, equals: Yt, f: n, fn: t, reactions: null, rv: 0, v: null, wv: 0, parent: e ?? p };
}
function Me(t) {
  const n = Tt(t);
  return ln(n), n;
}
function Fe(t) {
  const n = Tt(t);
  return n.equals = Bt, n;
}
function Vt(t) {
  var n = t.effects;
  if (n !== null) {
    t.effects = null;
    for (var e = 0; e < n.length; e += 1) F(n[e]);
  }
}
function Sn(t) {
  for (var n = t.parent; n !== null; ) {
    if ((n.f & T) === 0) return n;
    n = n.parent;
  }
  return null;
}
function Gt(t) {
  var n, e = p;
  it(Sn(t));
  try {
    Vt(t), n = cn(t);
  } finally {
    it(e);
  }
  return n;
}
function Kt(t) {
  var n = Gt(t);
  if (t.equals(n) || (t.v = n, t.wv = fn()), !K) {
    var e = (S || (t.f & g) !== 0) && t.deps !== null ? L : y;
    A(t, e);
  }
}
const Z = /* @__PURE__ */ new Map();
function At(t, n) {
  var e = { f: 0, v: t, reactions: null, equals: Yt, rv: 0, wv: 0 };
  return e;
}
function N(t, n) {
  const e = At(t);
  return ln(e), e;
}
function Le(t, n = false) {
  var _a;
  const e = At(t);
  return n || (e.equals = Bt), X && d !== null && d.l !== null && ((_a = d.l).s ?? (_a.s = [])).push(e), e;
}
function D(t, n, e = false) {
  v !== null && !x && ct() && (v.f & (T | wt)) !== 0 && !(I == null ? void 0 : I.includes(t)) && In();
  let r = e ? Y(n) : n;
  return kn(t, r);
}
function kn(t, n) {
  if (!t.equals(n)) {
    var e = t.v;
    K ? Z.set(t, n) : Z.set(t, e), t.v = n, (t.f & T) !== 0 && ((t.f & O) !== 0 && Gt(t), A(t, (t.f & g) === 0 ? y : L)), t.wv = fn(), $t(t, O), ct() && p !== null && (p.f & y) !== 0 && (p.f & (R | V)) === 0 && (b === null ? $n([t]) : b.push(t));
  }
  return n;
}
function $t(t, n) {
  var e = t.reactions;
  if (e !== null) for (var r = ct(), s = e.length, a = 0; a < s; a++) {
    var f = e[a], u = f.f;
    (u & O) === 0 && (!r && f === p || (A(f, n), (u & (y | g)) !== 0 && ((u & T) !== 0 ? $t(f, L) : _t(f))));
  }
}
function xt(t) {
  console.warn("https://svelte.dev/e/hydration_mismatch");
}
function qe() {
  console.warn("https://svelte.dev/e/select_multiple_invalid_value");
}
let M = false;
function je(t) {
  M = t;
}
let m;
function B(t) {
  if (t === null) throw xt(), mt;
  return m = t;
}
function Ye() {
  return B(q(m));
}
function He(t) {
  if (M) {
    if (q(m) !== null) throw xt(), mt;
    m = t;
  }
}
function Be(t = 1) {
  if (M) {
    for (var n = t, e = m; n--; ) e = q(e);
    m = e;
  }
}
function Ue() {
  for (var t = 0, n = m; ; ) {
    if (n.nodeType === 8) {
      var e = n.data;
      if (e === Nn) {
        if (t === 0) return n;
        t -= 1;
      } else (e === On || e === Rn) && (t += 1);
    }
    var r = q(n);
    n.remove(), n = r;
  }
}
function Ve(t) {
  if (!t || t.nodeType !== 8) throw xt(), mt;
  return t.data;
}
var kt, Cn, Pn, zt, Wt;
function Ge() {
  if (kt === void 0) {
    kt = window, Cn = document, Pn = /Firefox/.test(navigator.userAgent);
    var t = Element.prototype, n = Node.prototype, e = Text.prototype;
    zt = z(n, "firstChild").get, Wt = z(n, "nextSibling").get, Nt(t) && (t.__click = void 0, t.__className = void 0, t.__attributes = null, t.__style = void 0, t.__e = void 0), Nt(e) && (e.__t = void 0);
  }
}
function st(t = "") {
  return document.createTextNode(t);
}
function ht(t) {
  return zt.call(t);
}
function q(t) {
  return Wt.call(t);
}
function Ke(t, n) {
  if (!M) return ht(t);
  var e = ht(m);
  if (e === null) e = m.appendChild(st());
  else if (n && e.nodeType !== 3) {
    var r = st();
    return e == null ? void 0 : e.before(r), B(r), r;
  }
  return B(e), e;
}
function $e(t, n) {
  if (!M) {
    var e = ht(t);
    return e instanceof Comment && e.data === "" ? q(e) : e;
  }
  if (n && (m == null ? void 0 : m.nodeType) !== 3) {
    var r = st();
    return m == null ? void 0 : m.before(r), B(r), r;
  }
  return m;
}
function ze(t, n = 1, e = false) {
  let r = M ? m : t;
  for (var s; n--; ) s = r, r = q(r);
  if (!M) return r;
  var a = r == null ? void 0 : r.nodeType;
  if (e && a !== 3) {
    var f = st();
    return r === null ? s == null ? void 0 : s.after(f) : r.before(f), B(f), f;
  }
  return B(r), r;
}
function We(t) {
  t.textContent = "";
}
function Zt(t) {
  p === null && v === null && bn(), v !== null && (v.f & g) !== 0 && p === null && mn(), K && gn();
}
function Mn(t, n) {
  var e = n.last;
  e === null ? n.last = n.first = t : (e.next = t, t.prev = e, n.last = t);
}
function G(t, n, e, r = true) {
  var s = p, a = { ctx: d, deps: null, nodes_start: null, nodes_end: null, f: t | O, first: null, fn: n, last: null, next: null, parent: s, prev: null, teardown: null, transitions: null, wv: 0 };
  if (e) try {
    It(a), a.f |= Lt;
  } catch (i) {
    throw F(a), i;
  }
  else n !== null && _t(a);
  var f = e && a.deps === null && a.first === null && a.nodes_start === null && a.teardown === null && (a.f & (jt | Et)) === 0;
  if (!f && r && (s !== null && Mn(a, s), v !== null && (v.f & T) !== 0)) {
    var u = v;
    (u.effects ?? (u.effects = [])).push(a);
  }
  return a;
}
function Fn(t) {
  const n = G(ot, null, false);
  return A(n, y), n.teardown = t, n;
}
function Ln(t) {
  Zt();
  var n = p !== null && (p.f & R) !== 0 && d !== null && !d.m;
  if (n) {
    var e = d;
    (e.e ?? (e.e = [])).push({ fn: t, effect: p, reaction: v });
  } else {
    var r = Jt(t);
    return r;
  }
}
function Ze(t) {
  return Zt(), qn(t);
}
function Je(t) {
  const n = G(V, t, true);
  return (e = {}) => new Promise((r) => {
    e.outro ? Bn(n, () => {
      F(n), r(void 0);
    }) : (F(n), r(void 0));
  });
}
function Jt(t) {
  return G(Ft, t, false);
}
function qn(t) {
  return G(ot, t, true);
}
function Qe(t, n = [], e = Tt) {
  const r = n.map(e);
  return jn(() => t(...r.map($)));
}
function jn(t, n = 0) {
  return G(ot | wt | n, t, true);
}
function Xe(t, n = true) {
  return G(ot | R, t, true, n);
}
function Qt(t) {
  var n = t.teardown;
  if (n !== null) {
    const e = K, r = v;
    Ct(true), U(null);
    try {
      n.call(null);
    } finally {
      Ct(e), U(r);
    }
  }
}
function Xt(t, n = false) {
  var e = t.first;
  for (t.first = t.last = null; e !== null; ) {
    var r = e.next;
    (e.f & V) !== 0 ? e.parent = null : F(e, n), e = r;
  }
}
function Yn(t) {
  for (var n = t.first; n !== null; ) {
    var e = n.next;
    (n.f & R) === 0 && F(n), n = e;
  }
}
function F(t, n = true) {
  var e = false;
  (n || (t.f & En) !== 0) && t.nodes_start !== null && t.nodes_end !== null && (Hn(t.nodes_start, t.nodes_end), e = true), Xt(t, n && !e), ft(t, 0), A(t, gt);
  var r = t.transitions;
  if (r !== null) for (const a of r) a.stop();
  Qt(t);
  var s = t.parent;
  s !== null && s.first !== null && tn(t), t.next = t.prev = t.teardown = t.ctx = t.deps = t.fn = t.nodes_start = t.nodes_end = null;
}
function Hn(t, n) {
  for (; t !== null; ) {
    var e = t === n ? null : q(t);
    t.remove(), t = e;
  }
}
function tn(t) {
  var n = t.parent, e = t.prev, r = t.next;
  e !== null && (e.next = r), r !== null && (r.prev = e), n !== null && (n.first === t && (n.first = r), n.last === t && (n.last = e));
}
function Bn(t, n) {
  var e = [];
  nn(t, e, true), Un(e, () => {
    F(t), n && n();
  });
}
function Un(t, n) {
  var e = t.length;
  if (e > 0) {
    var r = () => --e || n();
    for (var s of t) s.out(r);
  } else n();
}
function nn(t, n, e) {
  if ((t.f & H) === 0) {
    if (t.f ^= H, t.transitions !== null) for (const f of t.transitions) (f.is_global || e) && n.push(f);
    for (var r = t.first; r !== null; ) {
      var s = r.next, a = (r.f & qt) !== 0 || (r.f & R) !== 0;
      nn(r, n, a ? e : false), r = s;
    }
  }
}
function tr(t) {
  en(t, true);
}
function en(t, n) {
  if ((t.f & H) !== 0) {
    t.f ^= H, (t.f & y) === 0 && (t.f ^= y), tt(t) && (A(t, O), _t(t));
    for (var e = t.first; e !== null; ) {
      var r = e.next, s = (e.f & qt) !== 0 || (e.f & R) !== 0;
      en(e, s ? n : false), e = r;
    }
    if (t.transitions !== null) for (const a of t.transitions) (a.is_global || n) && a.in();
  }
}
const Vn = typeof requestIdleCallback > "u" ? (t) => setTimeout(t, 1) : requestIdleCallback;
let J = [], Q = [];
function rn() {
  var t = J;
  J = [], Mt(t);
}
function sn() {
  var t = Q;
  Q = [], Mt(t);
}
function nr(t) {
  J.length === 0 && queueMicrotask(rn), J.push(t);
}
function er(t) {
  Q.length === 0 && Vn(sn), Q.push(t);
}
function Gn() {
  J.length > 0 && rn(), Q.length > 0 && sn();
}
function Kn(t) {
  var n = p;
  if ((n.f & Lt) === 0) {
    if ((n.f & Et) === 0) throw t;
    n.fn(t);
  } else an(t, n);
}
function an(t, n) {
  for (; n !== null; ) {
    if ((n.f & Et) !== 0) try {
      n.fn(t);
      return;
    } catch {
    }
    n = n.parent;
  }
  throw t;
}
let at = false, lt = null, P = false, K = false;
function Ct(t) {
  K = t;
}
let W = [];
let v = null, x = false;
function U(t) {
  v = t;
}
let p = null;
function it(t) {
  p = t;
}
let I = null;
function ln(t) {
  v !== null && v.f & pt && (I === null ? I = [t] : I.push(t));
}
let h = null, E = 0, b = null;
function $n(t) {
  b = t;
}
let un = 1, ut = 0, S = false;
function fn() {
  return ++un;
}
function tt(t) {
  var _a;
  var n = t.f;
  if ((n & O) !== 0) return true;
  if ((n & L) !== 0) {
    var e = t.deps, r = (n & g) !== 0;
    if (e !== null) {
      var s, a, f = (n & rt) !== 0, u = r && p !== null && !S, i = e.length;
      if (f || u) {
        var l = t, c = l.parent;
        for (s = 0; s < i; s++) a = e[s], (f || !((_a = a == null ? void 0 : a.reactions) == null ? void 0 : _a.includes(l))) && (a.reactions ?? (a.reactions = [])).push(l);
        f && (l.f ^= rt), u && c !== null && (c.f & g) === 0 && (l.f ^= g);
      }
      for (s = 0; s < i; s++) if (a = e[s], tt(a) && Kt(a), a.wv > t.wv) return true;
    }
    (!r || p !== null && !S) && A(t, y);
  }
  return false;
}
function on(t, n, e = true) {
  var r = t.reactions;
  if (r !== null) for (var s = 0; s < r.length; s++) {
    var a = r[s];
    (I == null ? void 0 : I.includes(t)) || ((a.f & T) !== 0 ? on(a, n, false) : n === a && (e ? A(a, O) : (a.f & y) !== 0 && A(a, L), _t(a)));
  }
}
function cn(t) {
  var _a;
  var n = h, e = E, r = b, s = v, a = S, f = I, u = d, i = x, l = t.f;
  h = null, E = 0, b = null, S = (l & g) !== 0 && (x || !P || v === null), v = (l & (R | V)) === 0 ? t : null, I = null, Dt(t.ctx), x = false, ut++, t.f |= pt;
  try {
    var c = (0, t.fn)(), o = t.deps;
    if (h !== null) {
      var _;
      if (ft(t, E), o !== null && E > 0) for (o.length = E + h.length, _ = 0; _ < h.length; _++) o[E + _] = h[_];
      else t.deps = o = h;
      if (!S) for (_ = E; _ < o.length; _++) ((_a = o[_]).reactions ?? (_a.reactions = [])).push(t);
    } else o !== null && E < o.length && (ft(t, E), o.length = E);
    if (ct() && b !== null && !x && o !== null && (t.f & (T | L | O)) === 0) for (_ = 0; _ < b.length; _++) on(b[_], t);
    return s !== null && s !== t && (ut++, b !== null && (r === null ? r = b : r.push(...b))), c;
  } catch (k) {
    Kn(k);
  } finally {
    h = n, E = e, b = r, v = s, S = a, I = f, Dt(u), x = i, t.f ^= pt;
  }
}
function zn(t, n) {
  let e = n.reactions;
  if (e !== null) {
    var r = pn.call(e, t);
    if (r !== -1) {
      var s = e.length - 1;
      s === 0 ? e = n.reactions = null : (e[r] = e[s], e.pop());
    }
  }
  e === null && (n.f & T) !== 0 && (h === null || !h.includes(n)) && (A(n, L), (n.f & (g | rt)) === 0 && (n.f ^= rt), Vt(n), ft(n, 0));
}
function ft(t, n) {
  var e = t.deps;
  if (e !== null) for (var r = n; r < e.length; r++) zn(t, e[r]);
}
function It(t) {
  var n = t.f;
  if ((n & gt) === 0) {
    A(t, y);
    var e = p, r = P;
    p = t, P = true;
    try {
      (n & wt) !== 0 ? Yn(t) : Xt(t), Qt(t);
      var s = cn(t);
      t.teardown = typeof s == "function" ? s : null, t.wv = un;
      var a = t.deps, f;
    } finally {
      P = r, p = e;
    }
  }
}
function Wn() {
  try {
    Tn();
  } catch (t) {
    if (lt !== null) an(t, lt);
    else throw t;
  }
}
function _n() {
  var t = P;
  try {
    var n = 0;
    for (P = true; W.length > 0; ) {
      n++ > 1e3 && Wn();
      var e = W, r = e.length;
      W = [];
      for (var s = 0; s < r; s++) {
        var a = Jn(e[s]);
        Zn(a);
      }
      Z.clear();
    }
  } finally {
    at = false, P = t, lt = null;
  }
}
function Zn(t) {
  var n = t.length;
  if (n !== 0) for (var e = 0; e < n; e++) {
    var r = t[e];
    (r.f & (gt | H)) === 0 && tt(r) && (It(r), r.deps === null && r.first === null && r.nodes_start === null && (r.teardown === null ? tn(r) : r.fn = null));
  }
}
function _t(t) {
  at || (at = true, queueMicrotask(_n));
  for (var n = lt = t; n.parent !== null; ) {
    n = n.parent;
    var e = n.f;
    if ((e & (V | R)) !== 0) {
      if ((e & y) === 0) return;
      n.f ^= y;
    }
  }
  W.push(n);
}
function Jn(t) {
  for (var n = [], e = t; e !== null; ) {
    var r = e.f, s = (r & (R | V)) !== 0, a = s && (r & y) !== 0;
    if (!a && (r & H) === 0) {
      (r & Ft) !== 0 ? n.push(e) : s ? e.f ^= y : tt(e) && It(e);
      var f = e.first;
      if (f !== null) {
        e = f;
        continue;
      }
    }
    var u = e.parent;
    for (e = e.next; e === null && u !== null; ) e = u.next, u = u.parent;
  }
  return n;
}
function Qn(t) {
  for (var n; ; ) {
    if (Gn(), W.length === 0) return n;
    at = true, _n();
  }
}
async function rr() {
  await Promise.resolve(), Qn();
}
function $(t) {
  var n = t.f, e = (n & T) !== 0;
  if (v !== null && !x) {
    if (!(I == null ? void 0 : I.includes(t))) {
      var r = v.deps;
      t.rv < ut && (t.rv = ut, h === null && r !== null && r[E] === t ? E++ : h === null ? h = [t] : (!S || !h.includes(t)) && h.push(t));
    }
  } else if (e && t.deps === null && t.effects === null) {
    var s = t, a = s.parent;
    a !== null && (a.f & g) === 0 && (s.f ^= g);
  }
  return e && (s = t, tt(s) && Kt(s)), K && Z.has(t) ? Z.get(t) : t.v;
}
function vn(t) {
  var n = x;
  try {
    return x = true, t();
  } finally {
    x = n;
  }
}
const Xn = -7169;
function A(t, n) {
  t.f = t.f & Xn | n;
}
function sr(t) {
  if (!(typeof t != "object" || !t || t instanceof EventTarget)) {
    if (C in t) yt(t);
    else if (!Array.isArray(t)) for (let n in t) {
      const e = t[n];
      typeof e == "object" && e && C in e && yt(e);
    }
  }
}
function yt(t, n = /* @__PURE__ */ new Set()) {
  if (typeof t == "object" && t !== null && !(t instanceof EventTarget) && !n.has(t)) {
    n.add(t), t instanceof Date && t.getTime();
    for (let r in t) try {
      yt(t[r], n);
    } catch {
    }
    const e = Pt(t);
    if (e !== Object.prototype && e !== Array.prototype && e !== Map.prototype && e !== Set.prototype && e !== Date.prototype) {
      const r = hn(e);
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
function te(t, n, e) {
  if (t == null) return n(void 0), et;
  const r = vn(() => t.subscribe(n, e));
  return r.unsubscribe ? () => r.unsubscribe() : r;
}
const j = [];
function ar(t, n = et) {
  let e = null;
  const r = /* @__PURE__ */ new Set();
  function s(u) {
    if (Ht(t, u) && (t = u, e)) {
      const i = !j.length;
      for (const l of r) l[1](), j.push(l, t);
      if (i) {
        for (let l = 0; l < j.length; l += 2) j[l][0](j[l + 1]);
        j.length = 0;
      }
    }
  }
  function a(u) {
    s(u(t));
  }
  function f(u, i = et) {
    const l = [u, i];
    return r.add(l), r.size === 1 && (e = n(s, a) || et), u(t), () => {
      r.delete(l), r.size === 0 && e && (e(), e = null);
    };
  }
  return { set: s, update: a, subscribe: f };
}
function lr(t) {
  let n;
  return te(t, (e) => n = e)(), n;
}
function ir(t) {
  d === null && Ut(), X && d.l !== null ? ne(d).m.push(t) : Ln(() => {
    const n = vn(t);
    if (typeof n == "function") return n;
  });
}
function ne(t) {
  var n = t.l;
  return n.u ?? (n.u = { a: [], b: [], m: [] });
}
export {
  we as $,
  Xe as A,
  Bn as B,
  m as C,
  Jt as D,
  qt as E,
  qn as F,
  nr as G,
  On as H,
  et as I,
  Le as J,
  te as K,
  lr as L,
  Fn as M,
  re as N,
  D as O,
  z as P,
  oe as Q,
  le as R,
  C as S,
  Fe as T,
  w as U,
  Ee as V,
  Bt as W,
  Y as X,
  ge as Y,
  ie as Z,
  X as _,
  Ln as a,
  ye as a0,
  me as a1,
  Qn as a2,
  ir as a3,
  N as a4,
  rr as a5,
  Me as a6,
  U as a7,
  it as a8,
  v as a9,
  de as aA,
  pe as aB,
  qe as aC,
  Pe as aD,
  Oe as aE,
  Pt as aF,
  Re as aG,
  hn as aH,
  ue as aI,
  er as aJ,
  wt as aK,
  Lt as aL,
  Ae as aM,
  be as aN,
  Te as aO,
  se as aP,
  ct as aQ,
  ar as aR,
  Cn as aS,
  Be as aT,
  Se as aU,
  Ne as aV,
  De as aW,
  p as aa,
  dn as ab,
  st as ac,
  En as ad,
  q as ae,
  ht as af,
  Pn as ag,
  xe as ah,
  Ie as ai,
  Ge as aj,
  mt as ak,
  Nn as al,
  xt as am,
  fe as an,
  We as ao,
  ee as ap,
  Je as aq,
  H as ar,
  kn as as,
  At as at,
  ve as au,
  nn as av,
  Un as aw,
  F as ax,
  _e as ay,
  he as az,
  vn as b,
  d as c,
  ae as d,
  ce as e,
  sr as f,
  $ as g,
  Tt as h,
  $e as i,
  Ce as j,
  Ke as k,
  He as l,
  jn as m,
  M as n,
  Ye as o,
  ke as p,
  Ve as q,
  Mt as r,
  ze as s,
  Qe as t,
  Ze as u,
  Rn as v,
  Ue as w,
  B as x,
  je as y,
  tr as z
};
