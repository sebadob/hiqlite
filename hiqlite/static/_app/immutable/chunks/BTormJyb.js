import { B as ge, x as L, C as ct, g as N, X as vt, D as dt, H as ht, F as ke, G, I as z, y as Q, ae as Ie, aH as _t, al as Ae, m as D, w as J, v as te, A as bt, N as gt, ac as Te, aF as Me, aJ as Oe, aK as Fe, n as Be, q as wt, aL as De, aM as pt, aN as oe, aB as he, aO as mt, aP as yt, aE as Et, o as re, at as Ue, aQ as Pe, L as we, aR as kt, E as xe, aS as At, aT as ze, J as se, aU as Tt, aV as Ct, Q as St, aW as pe, aX as He, aY as Lt, aZ as Rt, a_ as Nt, a$ as It, b0 as Mt, b1 as Ot, b2 as Ft, b3 as Bt, b4 as Dt, _ as Ut, b5 as Pt, ax as xt, b as Z, b6 as zt, b7 as Ht, b8 as qt, as as qe, b9 as Yt, M as W, K as Ye, a9 as Vt, k as F, s as ae, l as B, t as me, p as Ve, a8 as K, a as We, T as O, j as Qe, aa as le, i as Wt, ba as Qt, Y as _e } from "./3KHOJ3O8.js";
import { i as Kt, b as jt, d as Ke, n as Gt, e as Xt, g as Jt, f as H, a as P, c as Zt, j as $t } from "./CcmLzrf2.js";
import { B as er, p as R, i as ie, b as je, r as tr } from "./BR4MeHRE.js";
import "./BYtxu643.js";
function Br(e, t) {
  return t;
}
function rr(e, t, r) {
  for (var a = [], i = t.length, n = 0; n < i; n++) mt(t[n].e, a, true);
  yt(a, () => {
    var s = a.length === 0 && r !== null;
    if (s) {
      var o = r, f = o.parentNode;
      Et(f), f.append(o), e.items.clear(), M(e, t[0].prev, t[i - 1].next);
    }
    for (var u = 0; u < i; u++) {
      var l = t[u];
      s || (e.items.delete(l.k), M(e, l.prev, l.next)), re(l.e, !s);
    }
    e.first === t[0] && (e.first = t[0].prev);
  });
}
function Dr(e, t, r, a, i, n = null) {
  var s = e, o = /* @__PURE__ */ new Map(), f = null, u = (t & Pe) !== 0, l = (t & De) !== 0, g = (t & Fe) !== 0;
  if (u) {
    var c = e;
    s = L ? G(Ue(c)) : c.appendChild(te());
  }
  L && ct();
  var h = null, m = vt(() => {
    var p = r();
    return Oe(p) ? p : p == null ? [] : Me(p);
  }), w, d = true;
  function v() {
    ar(C, w, s, t, a), h !== null && (w.length === 0 ? (h.fragment ? (s.before(h.fragment), h.fragment = null) : Be(h.effect), A.first = h.effect) : wt(h.effect, () => {
      h = null;
    }));
  }
  var A = ge(() => {
    w = N(m);
    var p = w.length;
    let T = false;
    if (L) {
      var _ = dt(s) === ht;
      _ !== (p === 0) && (s = ke(), G(s), z(false), T = true);
    }
    for (var y = /* @__PURE__ */ new Set(), k = D, b = null, E = bt(), S = 0; S < p; S += 1) {
      L && Q.nodeType === Ie && Q.data === _t && (s = Q, T = true, z(false));
      var V = w[S], $ = a(V, S), I = d ? null : o.get($);
      I ? (l && Ae(I.v, V), g ? Ae(I.i, S) : I.i = S, E && k.skipped_effects.delete(I.e)) : (I = ir(d ? s : null, b, V, $, S, i, t, r), d && (I.o = true, b === null ? f = I : b.next = I, b = I), o.set($, I)), y.add($);
    }
    if (p === 0 && n && !h) if (d) h = { fragment: null, effect: J(() => n(s)) };
    else {
      var ye = document.createDocumentFragment(), Ee = te();
      ye.append(Ee), h = { fragment: ye, effect: J(() => n(Ee)) };
    }
    if (L && p > 0 && G(ke()), !d) if (E) {
      for (const [ft, ut] of o) y.has(ft) || k.skipped_effects.add(ut.e);
      k.oncommit(v), k.ondiscard(() => {
      });
    } else v();
    T && z(true), N(m);
  }), C = { effect: A, items: o, first: f };
  d = false, L && (s = Q);
}
function ar(e, t, r, a, i) {
  var _a, _b, _c, _d;
  var n = (a & kt) !== 0, s = t.length, o = e.items, f = e.first, u, l = null, g, c = [], h = [], m, w, d, v;
  if (n) for (v = 0; v < s; v += 1) m = t[v], w = i(m, v), d = o.get(w), d.o && ((_a = d.a) == null ? void 0 : _a.measure(), (g ?? (g = /* @__PURE__ */ new Set())).add(d));
  for (v = 0; v < s; v += 1) {
    if (m = t[v], w = i(m, v), d = o.get(w), e.first ?? (e.first = d), !d.o) {
      d.o = true;
      var A = l ? l.next : f;
      M(e, l, d), M(e, d, A), fe(d, A, r), l = d, c = [], h = [], f = l.next;
      continue;
    }
    if ((d.e.f & oe) !== 0 && (Be(d.e), n && ((_b = d.a) == null ? void 0 : _b.unfix(), (g ?? (g = /* @__PURE__ */ new Set())).delete(d))), d !== f) {
      if (u !== void 0 && u.has(d)) {
        if (c.length < h.length) {
          var C = h[0], p;
          l = C.prev;
          var T = c[0], _ = c[c.length - 1];
          for (p = 0; p < c.length; p += 1) fe(c[p], C, r);
          for (p = 0; p < h.length; p += 1) u.delete(h[p]);
          M(e, T.prev, _.next), M(e, l, T), M(e, _, C), f = C, l = _, v -= 1, c = [], h = [];
        } else u.delete(d), fe(d, f, r), M(e, d.prev, d.next), M(e, d, l === null ? e.first : l.next), M(e, l, d), l = d;
        continue;
      }
      for (c = [], h = []; f !== null && f.k !== w; ) (f.e.f & oe) === 0 && (u ?? (u = /* @__PURE__ */ new Set())).add(f), h.push(f), f = f.next;
      if (f === null) continue;
      d = f;
    }
    c.push(d), l = d, f = d.next;
  }
  let y = o.size > s;
  if (f !== null || u !== void 0) {
    for (var k = u === void 0 ? [] : Me(u); f !== null; ) (f.e.f & oe) === 0 && k.push(f), f = f.next;
    var b = k.length;
    if (y = o.size - b > s, b > 0) {
      var E = (a & Pe) !== 0 && s === 0 ? r : null;
      if (n) {
        for (v = 0; v < b; v += 1) (_c = k[v].a) == null ? void 0 : _c.measure();
        for (v = 0; v < b; v += 1) (_d = k[v].a) == null ? void 0 : _d.fix();
      }
      rr(e, k, E);
    }
  }
  if (y) for (const S of o.values()) S.o || (M(e, l, S), l = S);
  e.effect.last = l && l.e, n && we(() => {
    var _a2;
    if (g !== void 0) for (d of g) (_a2 = d.a) == null ? void 0 : _a2.apply();
  });
}
function ir(e, t, r, a, i, n, s, o) {
  var f = (s & De) !== 0, u = (s & pt) === 0, l = f ? u ? gt(r, false, false) : Te(r) : r, g = (s & Fe) === 0 ? i : Te(i), c = { i: g, v: l, k: a, a: null, e: null, o: false, prev: t, next: null };
  try {
    if (e === null) {
      var h = document.createDocumentFragment();
      h.append(e = te());
    }
    return c.e = J(() => n(e, l, g, o)), t !== null && (t.next = c), c;
  } finally {
  }
}
function fe(e, t, r) {
  for (var a = e.next ? e.next.e.nodes_start : r, i = t ? t.e.nodes_start : r, n = e.e.nodes_start; n !== null && n !== a; ) {
    var s = he(n);
    i.before(n), n = s;
  }
}
function M(e, t, r) {
  t === null ? (e.first = r, e.effect.first = r && r.e) : (t.e.next && (t.e.next.prev = null), t.next = r, t.e.next = r && r.e), r !== null && (r.e.prev && (r.e.prev.next = null), r.prev = t, r.e.prev = t && t.e);
}
function Ge(e, t, ...r) {
  var a = new er(e);
  ge(() => {
    const i = t() ?? null;
    a.ensure(i, i && ((n) => i(n, ...r)));
  }, xe);
}
function Ur(e, t) {
  let r = null, a = L;
  var i;
  if (L) {
    r = Q;
    for (var n = Ue(document.head); n !== null && (n.nodeType !== Ie || n.data !== e); ) n = he(n);
    if (n === null) z(false);
    else {
      var s = he(n);
      n.remove(), G(s);
    }
  }
  L || (i = document.head.appendChild(te()));
  try {
    ge(() => t(i), At);
  } finally {
    a && (z(true), G(r));
  }
}
function nr(e, t) {
  var r = void 0, a;
  ze(() => {
    r !== (r = t()) && (a && (re(a), a = null), r && (a = J(() => {
      se(() => r(e));
    })));
  });
}
function Xe(e) {
  var t, r, a = "";
  if (typeof e == "string" || typeof e == "number") a += e;
  else if (typeof e == "object") if (Array.isArray(e)) {
    var i = e.length;
    for (t = 0; t < i; t++) e[t] && (r = Xe(e[t])) && (a && (a += " "), a += r);
  } else for (r in e) e[r] && (a && (a += " "), a += r);
  return a;
}
function sr() {
  for (var e, t, r = 0, a = "", i = arguments.length; r < i; r++) (e = arguments[r]) && (t = Xe(e)) && (a && (a += " "), a += t);
  return a;
}
function or(e) {
  return typeof e == "object" ? sr(e) : e ?? "";
}
const Ce = [...` 	
\r\f\xA0\v\uFEFF`];
function lr(e, t, r) {
  var a = e == null ? "" : "" + e;
  if (t && (a = a ? a + " " + t : t), r) {
    for (var i in r) if (r[i]) a = a ? a + " " + i : i;
    else if (a.length) for (var n = i.length, s = 0; (s = a.indexOf(i, s)) >= 0; ) {
      var o = s + n;
      (s === 0 || Ce.includes(a[s - 1])) && (o === a.length || Ce.includes(a[o])) ? a = (s === 0 ? "" : a.substring(0, s)) + a.substring(o + 1) : s = o;
    }
  }
  return a === "" ? null : a;
}
function Se(e, t = false) {
  var r = t ? " !important;" : ";", a = "";
  for (var i in e) {
    var n = e[i];
    n != null && n !== "" && (a += " " + i + ": " + n + r);
  }
  return a;
}
function ue(e) {
  return e[0] !== "-" || e[1] !== "-" ? e.toLowerCase() : e;
}
function fr(e, t) {
  if (t) {
    var r = "", a, i;
    if (Array.isArray(t) ? (a = t[0], i = t[1]) : a = t, e) {
      e = String(e).replaceAll(/\s*\/\*.*?\*\/\s*/g, "").trim();
      var n = false, s = 0, o = false, f = [];
      a && f.push(...Object.keys(a).map(ue)), i && f.push(...Object.keys(i).map(ue));
      var u = 0, l = -1;
      const w = e.length;
      for (var g = 0; g < w; g++) {
        var c = e[g];
        if (o ? c === "/" && e[g - 1] === "*" && (o = false) : n ? n === c && (n = false) : c === "/" && e[g + 1] === "*" ? o = true : c === '"' || c === "'" ? n = c : c === "(" ? s++ : c === ")" && s--, !o && n === false && s === 0) {
          if (c === ":" && l === -1) l = g;
          else if (c === ";" || g === w - 1) {
            if (l !== -1) {
              var h = ue(e.substring(u, l).trim());
              if (!f.includes(h)) {
                c !== ";" && g++;
                var m = e.substring(u, g).trim();
                r += " " + m + ";";
              }
            }
            u = g + 1, l = -1;
          }
        }
      }
    }
    return a && (r += Se(a)), i && (r += Se(i, true)), r = r.trim(), r === "" ? null : r;
  }
  return e == null ? null : String(e);
}
function Je(e, t, r, a, i, n) {
  var s = e.__className;
  if (L || s !== r || s === void 0) {
    var o = lr(r, a, n);
    (!L || o !== e.getAttribute("class")) && (o == null ? e.removeAttribute("class") : t ? e.className = o : e.setAttribute("class", o)), e.__className = r;
  } else if (n && i !== n) for (var f in n) {
    var u = !!n[f];
    (i == null || u !== !!i[f]) && e.classList.toggle(f, u);
  }
  return n;
}
function ce(e, t = {}, r, a) {
  for (var i in r) {
    var n = r[i];
    t[i] !== n && (r[i] == null ? e.style.removeProperty(i) : e.style.setProperty(i, n, a));
  }
}
function Y(e, t, r, a) {
  var i = e.__style;
  if (L || i !== t) {
    var n = fr(t, a);
    (!L || n !== e.getAttribute("style")) && (n == null ? e.removeAttribute("style") : e.style.cssText = n), e.__style = t;
  } else a && (Array.isArray(a) ? (ce(e, r == null ? void 0 : r[0], a[0]), ce(e, r == null ? void 0 : r[1], a[1], "important")) : ce(e, r, a));
  return a;
}
function ne(e, t, r = false) {
  if (e.multiple) {
    if (t == null) return;
    if (!Oe(t)) return Tt();
    for (var a of e.options) a.selected = t.includes(X(a));
    return;
  }
  for (a of e.options) {
    var i = X(a);
    if (Ct(i, t)) {
      a.selected = true;
      return;
    }
  }
  (!r || t !== void 0) && (e.selectedIndex = -1);
}
function Ze(e) {
  var t = new MutationObserver(() => {
    ne(e, e.__value);
  });
  t.observe(e, { childList: true, subtree: true, attributes: true, attributeFilter: ["value"] }), St(() => {
    t.disconnect();
  });
}
function Pr(e, t, r = t) {
  var a = /* @__PURE__ */ new WeakSet(), i = true;
  pe(e, "change", (n) => {
    var s = n ? "[selected]" : ":checked", o;
    if (e.multiple) o = [].map.call(e.querySelectorAll(s), X);
    else {
      var f = e.querySelector(s) ?? e.querySelector("option:not([disabled])");
      o = f && X(f);
    }
    r(o), D !== null && a.add(D);
  }), se(() => {
    var n = t();
    if (e === document.activeElement) {
      var s = He ?? D;
      if (a.has(s)) return;
    }
    if (ne(e, n, i), i && n === void 0) {
      var o = e.querySelector(":checked");
      o !== null && (n = X(o), r(n));
    }
    e.__value = n, i = false;
  }), Ze(e);
}
function X(e) {
  return "__value" in e ? e.__value : e.value;
}
const j = Symbol("class"), q = Symbol("style"), $e = Symbol("is custom element"), et = Symbol("is html");
function ur(e) {
  if (L) {
    var t = false, r = () => {
      if (!t) {
        if (t = true, e.hasAttribute("value")) {
          var a = e.value;
          x(e, "value", null), e.value = a;
        }
        if (e.hasAttribute("checked")) {
          var i = e.checked;
          x(e, "checked", null), e.checked = i;
        }
      }
    };
    e.__on_r = r, we(r), Bt();
  }
}
function cr(e, t) {
  t ? e.hasAttribute("selected") || e.setAttribute("selected", "") : e.removeAttribute("selected");
}
function x(e, t, r, a) {
  var i = tt(e);
  L && (i[t] = e.getAttribute(t), t === "src" || t === "srcset" || t === "href" && e.nodeName === "LINK") || i[t] !== (i[t] = r) && (t === "loading" && (e[Dt] = r), r == null ? e.removeAttribute(t) : typeof r != "string" && rt(e).includes(t) ? e[t] = r : e.setAttribute(t, r));
}
function vr(e, t, r, a, i = false, n = false) {
  if (L && i && e.tagName === "INPUT") {
    var s = e, o = s.type === "checkbox" ? "defaultChecked" : "defaultValue";
    o in r || ur(s);
  }
  var f = tt(e), u = f[$e], l = !f[et];
  let g = L && u;
  g && z(false);
  var c = t || {}, h = e.tagName === "OPTION";
  for (var m in t) m in r || (r[m] = null);
  r.class ? r.class = or(r.class) : r.class = null, r[q] && (r.style ?? (r.style = null));
  var w = rt(e);
  for (const _ in r) {
    let y = r[_];
    if (h && _ === "value" && y == null) {
      e.value = e.__value = "", c[_] = y;
      continue;
    }
    if (_ === "class") {
      var d = e.namespaceURI === "http://www.w3.org/1999/xhtml";
      Je(e, d, y, a, t == null ? void 0 : t[j], r[j]), c[_] = y, c[j] = r[j];
      continue;
    }
    if (_ === "style") {
      Y(e, y, t == null ? void 0 : t[q], r[q]), c[_] = y, c[q] = r[q];
      continue;
    }
    var v = c[_];
    if (!(y === v && !(y === void 0 && e.hasAttribute(_)))) {
      c[_] = y;
      var A = _[0] + _[1];
      if (A !== "$$") if (A === "on") {
        const k = {}, b = "$$" + _;
        let E = _.slice(2);
        var C = Xt(E);
        if (Kt(E) && (E = E.slice(0, -7), k.capture = true), !C && v) {
          if (y != null) continue;
          e.removeEventListener(E, c[b], k), c[b] = null;
        }
        if (y != null) if (C) e[`__${E}`] = y, Ke([E]);
        else {
          let S = function(V) {
            c[_].call(this, V);
          };
          c[b] = jt(E, e, S, k);
        }
        else C && (e[`__${E}`] = void 0);
      } else if (_ === "style") x(e, _, y);
      else if (_ === "autofocus") Ot(e, !!y);
      else if (!u && (_ === "__value" || _ === "value" && y != null)) e.value = e.__value = y;
      else if (_ === "selected" && h) cr(e, y);
      else {
        var p = _;
        l || (p = Gt(p));
        var T = p === "defaultValue" || p === "defaultChecked";
        if (y == null && !u && !T) if (f[_] = null, p === "value" || p === "checked") {
          let k = e;
          const b = t === void 0;
          if (p === "value") {
            let E = k.defaultValue;
            k.removeAttribute(p), k.defaultValue = E, k.value = k.__value = b ? E : null;
          } else {
            let E = k.defaultChecked;
            k.removeAttribute(p), k.defaultChecked = E, k.checked = b ? E : false;
          }
        } else e.removeAttribute(_);
        else T || w.includes(p) && (u || typeof y != "string") ? (e[p] = y, p in f && (f[p] = Ft)) : typeof y != "function" && x(e, p, y);
      }
    }
  }
  return g && z(true), c;
}
function dr(e, t, r = [], a = [], i = [], n, s = false, o = false) {
  Lt(i, r, a, (f) => {
    var u = void 0, l = {}, g = e.nodeName === "SELECT", c = false;
    if (ze(() => {
      var m = t(...f.map(N)), w = vr(e, u, m, n, s, o);
      c && g && "value" in m && ne(e, m.value);
      for (let v of Object.getOwnPropertySymbols(l)) m[v] || re(l[v]);
      for (let v of Object.getOwnPropertySymbols(m)) {
        var d = m[v];
        v.description === It && (!u || d !== u[v]) && (l[v] && re(l[v]), l[v] = J(() => nr(e, () => d))), w[v] = d;
      }
      u = w;
    }), g) {
      var h = e;
      se(() => {
        ne(h, u.value, true), Ze(h);
      });
    }
    c = true;
  });
}
function tt(e) {
  return e.__attributes ?? (e.__attributes = { [$e]: e.nodeName.includes("-"), [et]: e.namespaceURI === Rt });
}
var Le = /* @__PURE__ */ new Map();
function rt(e) {
  var t = e.getAttribute("is") || e.nodeName, r = Le.get(t);
  if (r) return r;
  Le.set(t, r = []);
  for (var a, i = e, n = Element.prototype; n !== i; ) {
    a = Mt(i);
    for (var s in a) a[s].set && r.push(s);
    i = Nt(i);
  }
  return r;
}
const hr = () => performance.now(), U = { tick: (e) => requestAnimationFrame(e), now: () => hr(), tasks: /* @__PURE__ */ new Set() };
function at() {
  const e = U.now();
  U.tasks.forEach((t) => {
    t.c(e) || (U.tasks.delete(t), t.f());
  }), U.tasks.size !== 0 && U.tick(at);
}
function _r(e) {
  let t;
  return U.tasks.size === 0 && U.tick(at), { promise: new Promise((r) => {
    U.tasks.add(t = { c: e, f: r });
  }), abort() {
    U.tasks.delete(t);
  } };
}
function ee(e, t) {
  qe(() => {
    e.dispatchEvent(new CustomEvent(t));
  });
}
function br(e) {
  if (e === "float") return "cssFloat";
  if (e === "offset") return "cssOffset";
  if (e.startsWith("--")) return e;
  const t = e.split("-");
  return t.length === 1 ? t[0] : t[0] + t.slice(1).map((r) => r[0].toUpperCase() + r.slice(1)).join("");
}
function Re(e) {
  const t = {}, r = e.split(";");
  for (const a of r) {
    const [i, n] = a.split(":");
    if (!i || n === void 0) break;
    const s = br(i.trim());
    t[s] = n.trim();
  }
  return t;
}
const gr = (e) => e;
function it(e, t, r, a) {
  var i = (e & Ht) !== 0, n = (e & qt) !== 0, s = i && n, o = (e & zt) !== 0, f = s ? "both" : i ? "in" : "out", u, l = t.inert, g = t.style.overflow, c, h;
  function m() {
    return qe(() => u ?? (u = r()(t, (a == null ? void 0 : a()) ?? {}, { direction: f })));
  }
  var w = { is_global: o, in() {
    var _a;
    if (t.inert = l, !i) {
      h == null ? void 0 : h.abort(), (_a = h == null ? void 0 : h.reset) == null ? void 0 : _a.call(h);
      return;
    }
    n || (c == null ? void 0 : c.abort()), ee(t, "introstart"), c = be(t, m(), h, 1, () => {
      ee(t, "introend"), c == null ? void 0 : c.abort(), c = u = void 0, t.style.overflow = g;
    });
  }, out(C) {
    if (!n) {
      C == null ? void 0 : C(), u = void 0;
      return;
    }
    t.inert = true, ee(t, "outrostart"), h = be(t, m(), c, 0, () => {
      ee(t, "outroend"), C == null ? void 0 : C();
    });
  }, stop: () => {
    c == null ? void 0 : c.abort(), h == null ? void 0 : h.abort();
  } }, d = Ut;
  if ((d.transitions ?? (d.transitions = [])).push(w), i && Jt) {
    var v = o;
    if (!v) {
      for (var A = d.parent; A && (A.f & xe) !== 0; ) for (; (A = A.parent) && (A.f & Pt) === 0; ) ;
      v = !A || (A.f & xt) !== 0;
    }
    v && se(() => {
      Z(() => w.in());
    });
  }
}
function be(e, t, r, a, i) {
  var n = a === 1;
  if (Yt(t)) {
    var s, o = false;
    return we(() => {
      if (!o) {
        var d = t({ direction: n ? "in" : "out" });
        s = be(e, d, r, a, i);
      }
    }), { abort: () => {
      o = true, s == null ? void 0 : s.abort();
    }, deactivate: () => s.deactivate(), reset: () => s.reset(), t: () => s.t() };
  }
  if (r == null ? void 0 : r.deactivate(), !(t == null ? void 0 : t.duration)) return i(), { abort: W, deactivate: W, reset: W, t: () => a };
  const { delay: f = 0, css: u, tick: l, easing: g = gr } = t;
  var c = [];
  if (n && r === void 0 && (l && l(0, 1), u)) {
    var h = Re(u(0, 1));
    c.push(h, h);
  }
  var m = () => 1 - a, w = e.animate(c, { duration: f, fill: "forwards" });
  return w.onfinish = () => {
    w.cancel();
    var d = (r == null ? void 0 : r.t()) ?? 1 - a;
    r == null ? void 0 : r.abort();
    var v = a - d, A = t.duration * Math.abs(v), C = [];
    if (A > 0) {
      var p = false;
      if (u) for (var T = Math.ceil(A / 16.666666666666668), _ = 0; _ <= T; _ += 1) {
        var y = d + v * g(_ / T), k = Re(u(y, 1 - y));
        C.push(k), p || (p = k.overflow === "hidden");
      }
      p && (e.style.overflow = "hidden"), m = () => {
        var b = w.currentTime;
        return d + v * g(b / A);
      }, l && _r(() => {
        if (w.playState !== "running") return false;
        var b = m();
        return l(b, 1 - b), true;
      });
    }
    w = e.animate(C, { duration: A, fill: "forwards" }), w.onfinish = () => {
      m = () => a, l == null ? void 0 : l(a, 1 - a), i();
    };
  }, { abort: () => {
    w && (w.cancel(), w.effect = null, w.onfinish = W);
  }, deactivate: () => {
    i = W;
  }, reset: () => {
    a === 0 && (l == null ? void 0 : l(1, 0));
  }, t: () => m() };
}
function xr(e, t, r = t) {
  var a = /* @__PURE__ */ new WeakSet();
  pe(e, "input", async (i) => {
    var n = i ? e.defaultValue : e.value;
    if (n = ve(e) ? de(n) : n, r(n), D !== null && a.add(D), await Vt(), n !== (n = t())) {
      var s = e.selectionStart, o = e.selectionEnd, f = e.value.length;
      if (e.value = n ?? "", o !== null) {
        var u = e.value.length;
        s === o && o === f && u > f ? (e.selectionStart = u, e.selectionEnd = u) : (e.selectionStart = s, e.selectionEnd = Math.min(o, u));
      }
    }
  }), (L && e.defaultValue !== e.value || Z(t) == null && e.value) && (r(ve(e) ? de(e.value) : e.value), D !== null && a.add(D)), Ye(() => {
    var i = t();
    if (e === document.activeElement) {
      var n = He ?? D;
      if (a.has(n)) return;
    }
    ve(e) && i === de(e.value) || e.type === "date" && !i && !e.value || i !== e.value && (e.value = i ?? "");
  });
}
function zr(e, t, r = t) {
  pe(e, "change", (a) => {
    var i = a ? e.defaultChecked : e.checked;
    r(i);
  }), (L && e.defaultChecked !== e.checked || Z(t) == null) && r(e.checked), Ye(() => {
    var a = t();
    e.checked = !!a;
  });
}
function ve(e) {
  var t = e.type;
  return t === "number" || t === "range";
}
function de(e) {
  return e === "" ? null : +e;
}
const wr = (e) => e;
function nt(e) {
  const t = e - 1;
  return t * t * t + 1;
}
function st(e, { delay: t = 0, duration: r = 400, easing: a = wr } = {}) {
  const i = +getComputedStyle(e).opacity;
  return { delay: t, duration: r, easing: a, css: (n) => `opacity: ${n * i}` };
}
function Hr(e, { delay: t = 0, duration: r = 400, easing: a = nt, axis: i = "y" } = {}) {
  const n = getComputedStyle(e), s = +n.opacity, o = i === "y" ? "height" : "width", f = parseFloat(n[o]), u = i === "y" ? ["top", "bottom"] : ["left", "right"], l = u.map((v) => `${v[0].toUpperCase()}${v.slice(1)}`), g = parseFloat(n[`padding${l[0]}`]), c = parseFloat(n[`padding${l[1]}`]), h = parseFloat(n[`margin${l[0]}`]), m = parseFloat(n[`margin${l[1]}`]), w = parseFloat(n[`border${l[0]}Width`]), d = parseFloat(n[`border${l[1]}Width`]);
  return { delay: t, duration: r, easing: a, css: (v) => `overflow: hidden;opacity: ${Math.min(v * 20, 1) * s};${o}: ${v * f}px;padding-${u[0]}: ${v * g}px;padding-${u[1]}: ${v * c}px;margin-${u[0]}: ${v * h}px;margin-${u[1]}: ${v * m}px;border-${u[0]}-width: ${v * w}px;border-${u[1]}-width: ${v * d}px;min-${o}: 0` };
}
function Ne(e, t) {
  for (const r in t) e[r] = t[r];
  return e;
}
function qr({ fallback: e, ...t }) {
  const r = /* @__PURE__ */ new Map(), a = /* @__PURE__ */ new Map();
  function i(s, o, f) {
    const { delay: u = 0, duration: l = (_) => Math.sqrt(_) * 30, easing: g = nt } = Ne(Ne({}, t), f), c = s.getBoundingClientRect(), h = o.getBoundingClientRect(), m = c.left - h.left, w = c.top - h.top, d = c.width / h.width, v = c.height / h.height, A = Math.sqrt(m * m + w * w), C = getComputedStyle(o), p = C.transform === "none" ? "" : C.transform, T = +C.opacity;
    return { delay: u, duration: typeof l == "function" ? l(A) : l, easing: g, css: (_, y) => `
			   opacity: ${_ * T};
			   transform-origin: top left;
			   transform: ${p} translate(${y * m}px,${y * w}px) scale(${_ + (1 - _) * d}, ${_ + (1 - _) * v});
		   ` };
  }
  function n(s, o, f) {
    return (u, l) => (s.set(l.key, u), () => {
      if (o.has(l.key)) {
        const g = o.get(l.key);
        return o.delete(l.key), i(g, u, l);
      }
      return s.delete(l.key), e && e(u, l, f);
    });
  }
  return [n(a, r, false), n(r, a, true)];
}
var pr = H('<div><div class="loading svelte-1a5pdw0"><div class="loading-1 svelte-1a5pdw0"></div> <div class="loading-2 svelte-1a5pdw0"></div> <div class="loading-3 svelte-1a5pdw0"></div></div></div>');
function mr(e, t) {
  let r = R(t, "background", 3, false), a = R(t, "color", 3, "var(--col-text)"), i = R(t, "global", 3, false), n = R(t, "offset", 3, 0);
  var s = pr();
  let o;
  var f = F(s), u = F(f);
  let l;
  var g = ae(u, 2);
  let c;
  var h = ae(g, 2);
  let m;
  B(f), B(s), me(() => {
    o = Je(s, 1, "container svelte-1a5pdw0", null, o, { global: i(), local: !i(), background: r() }), Y(f, `margin-top: ${n() ?? ""}px;`), l = Y(u, "", l, { background: a() }), c = Y(g, "", c, { background: a() }), m = Y(h, "", m, { background: a() });
  }), it(3, s, () => st, () => ({ duration: 100 })), P(e, s);
}
var yr = H('<div class="load svelte-18sv61c"><!></div>'), Er = H('<div class="font-label"><!></div>'), kr = H("<button><!></button>");
function Yr(e, t) {
  Ve(t, true);
  let r = R(t, "type", 3, "button"), a = R(t, "role", 3, "button"), i = R(t, "ref", 15), n = R(t, "level", 3, 2), s = R(t, "isDisabled", 3, false), o = R(t, "isLoading", 3, false), f = R(t, "destructive", 3, false), u = R(t, "invisible", 3, false), l = R(t, "invisibleOutline", 3, false), g = tr(t, ["$$slots", "$$events", "$$legacy", "type", "role", "ref", "id", "ariaLabel", "ariaControls", "ariaCurrent", "level", "width", "isDisabled", "isLoading", "destructive", "invisible", "invisibleOutline", "popovertarget", "popovertargetaction", "onclick", "onLeft", "onRight", "onUp", "onDown", "children"]), c = le(() => {
    if (u()) return "invisible";
    if (f()) return "destructive";
    switch (n()) {
      case 2:
        return "l2";
      case 3:
        return "l3";
      default:
        return "l1";
    }
  }), h = K(!o()), m = le(() => s() || o());
  We(() => {
    o() ? setTimeout(() => {
      O(h, false);
    }, 120) : setTimeout(() => {
      O(h, true);
    }, 120);
  });
  function w() {
    if (f()) return "var(--btn-text)";
    switch (n()) {
      case 2:
        return "hsl(var(--action))";
      case 3:
        return "hsl(var(--action))";
      default:
        return "var(--btn-text)";
    }
  }
  function d(T) {
    var _a, _b, _c, _d, _e2;
    switch (T.code) {
      case "Enter":
        (_a = t.onclick) == null ? void 0 : _a.call(t, T);
        break;
      case "ArrowLeft":
        (_b = t.onLeft) == null ? void 0 : _b.call(t);
        break;
      case "ArrowRight":
        (_c = t.onRight) == null ? void 0 : _c.call(t);
        break;
      case "ArrowUp":
        (_d = t.onUp) == null ? void 0 : _d.call(t);
        break;
      case "ArrowDown":
        (_e2 = t.onDown) == null ? void 0 : _e2.call(t);
        break;
    }
  }
  var v = kr();
  dr(v, () => ({ role: a(), type: r(), id: t.id, "aria-label": t.ariaLabel, "aria-controls": t.ariaControls, "aria-current": t.ariaCurrent, class: N(c), "data-isloading": o(), onclick: t.onclick, onkeydown: d, disabled: N(m), "aria-disabled": N(m), popovertarget: t.popovertarget, popovertargetaction: t.popovertargetaction, ...g, [j]: { invisibleOutline: l() }, [q]: { width: t.width } }), void 0, void 0, void 0, "svelte-18sv61c");
  var A = F(v);
  {
    var C = (T) => {
      var _ = yr(), y = F(_);
      {
        let k = le(w);
        mr(y, { background: false, get color() {
          return N(k);
        } });
      }
      B(_), P(T, _);
    }, p = (T) => {
      var _ = Zt(), y = Wt(_);
      {
        var k = (b) => {
          var E = Er(), S = F(E);
          Ge(S, () => t.children), B(E), it(1, E, () => st), P(b, E);
        };
        ie(y, (b) => {
          N(h) && b(k);
        }, true);
      }
      P(T, _);
    };
    ie(A, (T) => {
      o() ? T(C) : T(p, false);
    });
  }
  B(v), je(v, (T) => i(T), () => i()), P(e, v), Qe();
}
const Ar = Qt(void 0), ot = "/dashboard/api";
async function Vr(e) {
  let t = await fetch(`${ot}${e}`, { method: "GET" });
  return lt(t);
}
async function Wr(e, t) {
  let r = await fetch(`${ot}${e}`, { method: "POST", body: t });
  return lt(r);
}
function lt(e) {
  return e.status === 401 && Ar.set(void 0), e;
}
var Tr = $t(`<svg fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M2.036 12.322a1.012 1.012 0 010-.639C3.423 7.51 7.36 4.5 12 4.5c4.638 0 8.573 3.007 9.963
            7.178.07.207.07.431 0 .639C20.577 16.49 16.64 19.5 12 19.5c-4.638 0-8.573-3.007-9.963-7.178z"></path><path stroke-linecap="round" stroke-linejoin="round" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"></path></svg>`);
function Qr(e, t) {
  let r = R(t, "color", 8, "var(--col-ok)"), a = R(t, "opacity", 8, 0.9), i = R(t, "width", 8, "1.5rem");
  var n = Tr();
  x(n, "stroke-width", 2), me(() => {
    x(n, "width", i()), x(n, "color", r()), x(n, "opacity", a());
  }), P(e, n);
}
const Cr = `-- comments will be ignored but only a single query is allowed
-- press CTRL + Enter to execute
SELECT 1`, Sr = { id: "SELECT 1", query: Cr }, Kr = "--!auto-query";
let jr = _e([Sr]);
const Gr = (e) => {
  let t = "";
  const r = e || 8;
  for (let a = 0; a < r; a += 1) {
    let i = 60;
    for (; i > 57 && i < 65 || i > 90 && i < 97; ) i = Math.floor(Math.random() * 74) + 48;
    t = t.concat(String.fromCharCode(i));
  }
  return t;
};
var Lr = H('<div class="relative"><div role="none" class="right svelte-19ulb1h"></div></div>'), Rr = H('<div class="relative"><div role="none" class="bottom svelte-19ulb1h"></div></div>'), Nr = H('<div><div class="children svelte-19ulb1h"><div class="inner svelte-19ulb1h"><!></div> <!></div> <!></div>');
function Xr(e, t) {
  Ve(t, true);
  let r = R(t, "minWidthPx", 3, 50), a = R(t, "minHeightPx", 3, 50), i, n = K(void 0), s = K(void 0), o = K(_e(Z(() => t.initialWidthPx))), f = K(_e(Z(() => t.initialHeightPx)));
  We(() => {
    u();
  });
  function u() {
    var _a;
    if (i) {
      let b = i.getBoundingClientRect();
      t.resizeRight && (O(s, b.left, true), O(o, b.width, true)), t.resizeBottom && (O(n, b.top, true), O(f, b.height, true)), (_a = t.onResizeBottom) == null ? void 0 : _a.call(t, b.bottom);
    }
  }
  function l() {
    u(), window.addEventListener("mousemove", c), window.addEventListener("mouseup", g, { once: true });
  }
  function g() {
    window.removeEventListener("mousemove", c), u();
  }
  function c(b) {
    let E = window.scrollX + b.x - (N(s) || 0);
    E < r() ? O(o, r()) : O(o, E);
  }
  function h() {
    u(), window.addEventListener("mousemove", w), window.addEventListener("mouseup", m, { once: true });
  }
  function m() {
    window.removeEventListener("mousemove", w), u();
  }
  function w(b) {
    console.log(window.screenY, b.y);
    let E = window.screenY + b.clientY - (N(n) || 0);
    if (E < a() ? O(f, a()) : O(f, E), i && t.onResizeBottom) {
      let S = i.getBoundingClientRect();
      t.onResizeBottom(S.bottom);
    }
  }
  var d = Nr();
  let v;
  var A = F(d), C = F(A), p = F(C);
  Ge(p, () => t.children), B(C);
  var T = ae(C, 2);
  {
    var _ = (b) => {
      var E = Lr(), S = F(E);
      S.__mousedown = l, B(E), P(b, E);
    };
    ie(T, (b) => {
      t.resizeRight && b(_);
    });
  }
  B(A);
  var y = ae(A, 2);
  {
    var k = (b) => {
      var E = Rr(), S = F(E);
      S.__mousedown = h, B(E), P(b, E);
    };
    ie(y, (b) => {
      t.resizeBottom && b(k);
    });
  }
  B(d), je(d, (b) => i = b, () => i), me(() => v = Y(d, "", v, { width: N(o) && `${N(o)}px`, height: N(f) && `${N(f)}px`, border: t.border, padding: t.padding })), P(e, d), Qe();
}
Ke(["mousedown"]);
export {
  ot as A,
  Yr as B,
  Sr as D,
  Qr as I,
  jr as Q,
  Xr as R,
  x as a,
  Y as b,
  qr as c,
  xr as d,
  Hr as e,
  Ur as f,
  Ar as g,
  lt as h,
  Dr as i,
  Br as j,
  Je as k,
  or as l,
  Vr as m,
  Gr as n,
  Kr as o,
  cr as p,
  Pr as q,
  ur as r,
  Ge as s,
  it as t,
  zr as u,
  Wr as v,
  Cr as w
};
