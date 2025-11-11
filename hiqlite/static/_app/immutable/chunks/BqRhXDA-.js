import { B as ee, x as S, C as dt, g as N, X as ht, D as _t, H as bt, F as Ce, G, I as q, y as Y, ae as Be, aH as gt, w as J, A as wt, m as D, al as Se, v as Ee, N as yt, ac as Le, aF as Fe, aJ as De, aK as se, n as Ue, q as pt, aL as oe, aM as mt, aN as ce, o as Z, aB as we, aO as Et, aE as At, aP as kt, _ as Pe, at as He, aQ as qe, L as Ae, aR as Tt, E as ze, aS as Ct, J as le, aT as St, aU as Lt, Q as Rt, aV as ke, aW as Ye, aX as Nt, aY as Mt, aZ as It, a_ as Ot, a$ as Bt, b0 as Ft, b1 as Dt, b2 as Ut, b3 as Pt, b4 as Ht, ax as qt, b as $, b5 as zt, b6 as Yt, b7 as xt, as as xe, b8 as Vt, M as Q, K as Ve, a9 as Wt, k as B, s as ae, l as F, t as Te, p as We, a8 as K, a as Qe, T as I, j as Ke, aa as ve, i as Qt, b9 as Kt, Y as ye } from "./BDwp15xD.js";
import { i as jt, b as Gt, d as je, n as Xt, e as Jt, g as Zt, f as z, a as P, c as $t, j as er } from "./BCOsh4zG.js";
import { B as tr, p as L, i as ie, b as Ge, r as rr } from "./BusyLArd.js";
import "./COHE0qRA.js";
function Fr(e, t) {
  return t;
}
function ar(e, t, r) {
  for (var a = e.items, i = [], n = t.length, s = 0; s < n; s++) Et(t[s].e, i, true);
  var l = n > 0 && i.length === 0 && r !== null;
  if (l) {
    var _ = r.parentNode;
    At(_), _.append(r), a.clear(), O(e, t[0].prev, t[n - 1].next);
  }
  kt(i, () => {
    for (var f = 0; f < n; f++) {
      var c = t[f];
      l || (a.delete(c.k), O(e, c.prev, c.next)), Z(c.e, !l);
    }
  });
}
function Dr(e, t, r, a, i, n = null) {
  var s = e, l = { flags: t, items: /* @__PURE__ */ new Map(), first: null }, _ = (t & qe) !== 0;
  if (_) {
    var f = e;
    s = S ? G(He(f)) : f.appendChild(Ee());
  }
  S && dt();
  var c = null, w = false, v = /* @__PURE__ */ new Map(), p = ht(() => {
    var h = r();
    return De(h) ? h : h == null ? [] : Fe(h);
  }), u, b;
  function g() {
    ir(b, u, l, v, s, i, t, a, r), n !== null && (u.length === 0 ? c ? Ue(c) : c = J(() => n(s)) : c !== null && pt(c, () => {
      c = null;
    }));
  }
  ee(() => {
    b ?? (b = Pe), u = N(p);
    var h = u.length;
    if (w && h === 0) return;
    w = h === 0;
    let E = false;
    if (S) {
      var C = _t(s) === bt;
      C !== (h === 0) && (s = Ce(), G(s), q(false), E = true);
    }
    if (S) {
      for (var T = null, A, o = 0; o < h; o++) {
        if (Y.nodeType === Be && Y.data === gt) {
          s = Y, E = true, q(false);
          break;
        }
        var d = u[o], k = a(d, o);
        A = pe(Y, l, T, null, d, k, o, i, t, r), l.items.set(k, A), T = A;
      }
      h > 0 && G(Ce());
    }
    if (S) h === 0 && n && (c = J(() => n(s)));
    else if (wt()) {
      var y = /* @__PURE__ */ new Set(), m = D;
      for (o = 0; o < h; o += 1) {
        d = u[o], k = a(d, o);
        var R = l.items.get(k) ?? v.get(k);
        R ? (t & (oe | se)) !== 0 && Xe(R, d, o, t) : (A = pe(null, l, null, null, d, k, o, i, t, r, true), v.set(k, A)), y.add(k);
      }
      for (const [M, te] of l.items) y.has(M) || m.skipped_effects.add(te.e);
      m.oncommit(g);
    } else g();
    E && q(true), N(p);
  }), S && (s = Y);
}
function ir(e, t, r, a, i, n, s, l, _) {
  var _a, _b, _c, _d;
  var f = (s & Tt) !== 0, c = (s & (oe | se)) !== 0, w = t.length, v = r.items, p = r.first, u = p, b, g = null, h, E = [], C = [], T, A, o, d;
  if (f) for (d = 0; d < w; d += 1) T = t[d], A = l(T, d), o = v.get(A), o !== void 0 && ((_a = o.a) == null ? void 0 : _a.measure(), (h ?? (h = /* @__PURE__ */ new Set())).add(o));
  for (d = 0; d < w; d += 1) {
    if (T = t[d], A = l(T, d), o = v.get(A), o === void 0) {
      var k = a.get(A);
      if (k !== void 0) {
        a.delete(A), v.set(A, k);
        var y = g ? g.next : u;
        O(r, g, k), O(r, k, y), de(k, y, i), g = k;
      } else {
        var m = u ? u.e.nodes_start : i;
        g = pe(m, r, g, g === null ? r.first : g.next, T, A, d, n, s, _);
      }
      v.set(A, g), E = [], C = [], u = g.next;
      continue;
    }
    if (c && Xe(o, T, d, s), (o.e.f & ce) !== 0 && (Ue(o.e), f && ((_b = o.a) == null ? void 0 : _b.unfix(), (h ?? (h = /* @__PURE__ */ new Set())).delete(o))), o !== u) {
      if (b !== void 0 && b.has(o)) {
        if (E.length < C.length) {
          var R = C[0], M;
          g = R.prev;
          var te = E[0], fe = E[E.length - 1];
          for (M = 0; M < E.length; M += 1) de(E[M], R, i);
          for (M = 0; M < C.length; M += 1) b.delete(C[M]);
          O(r, te.prev, fe.next), O(r, g, te), O(r, fe, R), u = R, g = fe, d -= 1, E = [], C = [];
        } else b.delete(o), de(o, u, i), O(r, o.prev, o.next), O(r, o, g === null ? r.first : g.next), O(r, g, o), g = o;
        continue;
      }
      for (E = [], C = []; u !== null && u.k !== A; ) (u.e.f & ce) === 0 && (b ?? (b = /* @__PURE__ */ new Set())).add(u), C.push(u), u = u.next;
      if (u === null) continue;
      o = u;
    }
    E.push(o), g = o, u = o.next;
  }
  if (u !== null || b !== void 0) {
    for (var W = b === void 0 ? [] : Fe(b); u !== null; ) (u.e.f & ce) === 0 && W.push(u), u = u.next;
    var ue = W.length;
    if (ue > 0) {
      var ct = (s & qe) !== 0 && w === 0 ? i : null;
      if (f) {
        for (d = 0; d < ue; d += 1) (_c = W[d].a) == null ? void 0 : _c.measure();
        for (d = 0; d < ue; d += 1) (_d = W[d].a) == null ? void 0 : _d.fix();
      }
      ar(r, W, ct);
    }
  }
  f && Ae(() => {
    var _a2;
    if (h !== void 0) for (o of h) (_a2 = o.a) == null ? void 0 : _a2.apply();
  }), e.first = r.first && r.first.e, e.last = g && g.e;
  for (var vt of a.values()) Z(vt.e);
  a.clear();
}
function Xe(e, t, r, a) {
  (a & oe) !== 0 && Se(e.v, t), (a & se) !== 0 ? Se(e.i, r) : e.i = r;
}
function pe(e, t, r, a, i, n, s, l, _, f, c) {
  var w = (_ & oe) !== 0, v = (_ & mt) === 0, p = w ? v ? yt(i, false, false) : Le(i) : i, u = (_ & se) === 0 ? s : Le(s), b = { i: u, v: p, k: n, a: null, e: null, prev: r, next: a };
  try {
    if (e === null) {
      var g = document.createDocumentFragment();
      g.append(e = Ee());
    }
    return b.e = J(() => l(e, p, u, f), S), b.e.prev = r && r.e, b.e.next = a && a.e, r === null ? c || (t.first = b) : (r.next = b, r.e.next = b.e), a !== null && (a.prev = b, a.e.prev = b.e), b;
  } finally {
  }
}
function de(e, t, r) {
  for (var a = e.next ? e.next.e.nodes_start : r, i = t ? t.e.nodes_start : r, n = e.e.nodes_start; n !== null && n !== a; ) {
    var s = we(n);
    i.before(n), n = s;
  }
}
function O(e, t, r) {
  t === null ? e.first = r : (t.next = r, t.e.next = r && r.e), r !== null && (r.prev = t, r.e.prev = t && t.e);
}
function Je(e, t, ...r) {
  var a = new tr(e);
  ee(() => {
    const i = t() ?? null;
    a.ensure(i, i && ((n) => i(n, ...r)));
  }, ze);
}
function Ur(e, t) {
  let r = null, a = S;
  var i;
  if (S) {
    r = Y;
    for (var n = He(document.head); n !== null && (n.nodeType !== Be || n.data !== e); ) n = we(n);
    if (n === null) q(false);
    else {
      var s = we(n);
      n.remove(), G(s);
    }
  }
  S || (i = document.head.appendChild(Ee()));
  try {
    ee(() => t(i), Ct);
  } finally {
    a && (q(true), G(r));
  }
}
function nr(e, t) {
  var r = void 0, a;
  ee(() => {
    r !== (r = t()) && (a && (Z(a), a = null), r && (a = J(() => {
      le(() => r(e));
    })));
  });
}
function Ze(e) {
  var t, r, a = "";
  if (typeof e == "string" || typeof e == "number") a += e;
  else if (typeof e == "object") if (Array.isArray(e)) {
    var i = e.length;
    for (t = 0; t < i; t++) e[t] && (r = Ze(e[t])) && (a && (a += " "), a += r);
  } else for (r in e) e[r] && (a && (a += " "), a += r);
  return a;
}
function sr() {
  for (var e, t, r = 0, a = "", i = arguments.length; r < i; r++) (e = arguments[r]) && (t = Ze(e)) && (a && (a += " "), a += t);
  return a;
}
function or(e) {
  return typeof e == "object" ? sr(e) : e ?? "";
}
const Re = [...` 	
\r\f\xA0\v\uFEFF`];
function lr(e, t, r) {
  var a = e == null ? "" : "" + e;
  if (t && (a = a ? a + " " + t : t), r) {
    for (var i in r) if (r[i]) a = a ? a + " " + i : i;
    else if (a.length) for (var n = i.length, s = 0; (s = a.indexOf(i, s)) >= 0; ) {
      var l = s + n;
      (s === 0 || Re.includes(a[s - 1])) && (l === a.length || Re.includes(a[l])) ? a = (s === 0 ? "" : a.substring(0, s)) + a.substring(l + 1) : s = l;
    }
  }
  return a === "" ? null : a;
}
function Ne(e, t = false) {
  var r = t ? " !important;" : ";", a = "";
  for (var i in e) {
    var n = e[i];
    n != null && n !== "" && (a += " " + i + ": " + n + r);
  }
  return a;
}
function he(e) {
  return e[0] !== "-" || e[1] !== "-" ? e.toLowerCase() : e;
}
function fr(e, t) {
  if (t) {
    var r = "", a, i;
    if (Array.isArray(t) ? (a = t[0], i = t[1]) : a = t, e) {
      e = String(e).replaceAll(/\s*\/\*.*?\*\/\s*/g, "").trim();
      var n = false, s = 0, l = false, _ = [];
      a && _.push(...Object.keys(a).map(he)), i && _.push(...Object.keys(i).map(he));
      var f = 0, c = -1;
      const b = e.length;
      for (var w = 0; w < b; w++) {
        var v = e[w];
        if (l ? v === "/" && e[w - 1] === "*" && (l = false) : n ? n === v && (n = false) : v === "/" && e[w + 1] === "*" ? l = true : v === '"' || v === "'" ? n = v : v === "(" ? s++ : v === ")" && s--, !l && n === false && s === 0) {
          if (v === ":" && c === -1) c = w;
          else if (v === ";" || w === b - 1) {
            if (c !== -1) {
              var p = he(e.substring(f, c).trim());
              if (!_.includes(p)) {
                v !== ";" && w++;
                var u = e.substring(f, w).trim();
                r += " " + u + ";";
              }
            }
            f = w + 1, c = -1;
          }
        }
      }
    }
    return a && (r += Ne(a)), i && (r += Ne(i, true)), r = r.trim(), r === "" ? null : r;
  }
  return e == null ? null : String(e);
}
function $e(e, t, r, a, i, n) {
  var s = e.__className;
  if (S || s !== r || s === void 0) {
    var l = lr(r, a, n);
    (!S || l !== e.getAttribute("class")) && (l == null ? e.removeAttribute("class") : t ? e.className = l : e.setAttribute("class", l)), e.__className = r;
  } else if (n && i !== n) for (var _ in n) {
    var f = !!n[_];
    (i == null || f !== !!i[_]) && e.classList.toggle(_, f);
  }
  return n;
}
function _e(e, t = {}, r, a) {
  for (var i in r) {
    var n = r[i];
    t[i] !== n && (r[i] == null ? e.style.removeProperty(i) : e.style.setProperty(i, n, a));
  }
}
function V(e, t, r, a) {
  var i = e.__style;
  if (S || i !== t) {
    var n = fr(t, a);
    (!S || n !== e.getAttribute("style")) && (n == null ? e.removeAttribute("style") : e.style.cssText = n), e.__style = t;
  } else a && (Array.isArray(a) ? (_e(e, r == null ? void 0 : r[0], a[0]), _e(e, r == null ? void 0 : r[1], a[1], "important")) : _e(e, r, a));
  return a;
}
function ne(e, t, r = false) {
  if (e.multiple) {
    if (t == null) return;
    if (!De(t)) return St();
    for (var a of e.options) a.selected = t.includes(X(a));
    return;
  }
  for (a of e.options) {
    var i = X(a);
    if (Lt(i, t)) {
      a.selected = true;
      return;
    }
  }
  (!r || t !== void 0) && (e.selectedIndex = -1);
}
function et(e) {
  var t = new MutationObserver(() => {
    ne(e, e.__value);
  });
  t.observe(e, { childList: true, subtree: true, attributes: true, attributeFilter: ["value"] }), Rt(() => {
    t.disconnect();
  });
}
function Pr(e, t, r = t) {
  var a = /* @__PURE__ */ new WeakSet(), i = true;
  ke(e, "change", (n) => {
    var s = n ? "[selected]" : ":checked", l;
    if (e.multiple) l = [].map.call(e.querySelectorAll(s), X);
    else {
      var _ = e.querySelector(s) ?? e.querySelector("option:not([disabled])");
      l = _ && X(_);
    }
    r(l), D !== null && a.add(D);
  }), le(() => {
    var n = t();
    if (e === document.activeElement) {
      var s = Ye ?? D;
      if (a.has(s)) return;
    }
    if (ne(e, n, i), i && n === void 0) {
      var l = e.querySelector(":checked");
      l !== null && (n = X(l), r(n));
    }
    e.__value = n, i = false;
  }), et(e);
}
function X(e) {
  return "__value" in e ? e.__value : e.value;
}
const j = Symbol("class"), x = Symbol("style"), tt = Symbol("is custom element"), rt = Symbol("is html");
function ur(e) {
  if (S) {
    var t = false, r = () => {
      if (!t) {
        if (t = true, e.hasAttribute("value")) {
          var a = e.value;
          H(e, "value", null), e.value = a;
        }
        if (e.hasAttribute("checked")) {
          var i = e.checked;
          H(e, "checked", null), e.checked = i;
        }
      }
    };
    e.__on_r = r, Ae(r), Ut();
  }
}
function cr(e, t) {
  t ? e.hasAttribute("selected") || e.setAttribute("selected", "") : e.removeAttribute("selected");
}
function H(e, t, r, a) {
  var i = at(e);
  S && (i[t] = e.getAttribute(t), t === "src" || t === "srcset" || t === "href" && e.nodeName === "LINK") || i[t] !== (i[t] = r) && (t === "loading" && (e[Pt] = r), r == null ? e.removeAttribute(t) : typeof r != "string" && it(e).includes(t) ? e[t] = r : e.setAttribute(t, r));
}
function vr(e, t, r, a, i = false, n = false) {
  if (S && i && e.tagName === "INPUT") {
    var s = e, l = s.type === "checkbox" ? "defaultChecked" : "defaultValue";
    l in r || ur(s);
  }
  var _ = at(e), f = _[tt], c = !_[rt];
  let w = S && f;
  w && q(false);
  var v = t || {}, p = e.tagName === "OPTION";
  for (var u in t) u in r || (r[u] = null);
  r.class ? r.class = or(r.class) : r.class = null, r[x] && (r.style ?? (r.style = null));
  var b = it(e);
  for (const o in r) {
    let d = r[o];
    if (p && o === "value" && d == null) {
      e.value = e.__value = "", v[o] = d;
      continue;
    }
    if (o === "class") {
      var g = e.namespaceURI === "http://www.w3.org/1999/xhtml";
      $e(e, g, d, a, t == null ? void 0 : t[j], r[j]), v[o] = d, v[j] = r[j];
      continue;
    }
    if (o === "style") {
      V(e, d, t == null ? void 0 : t[x], r[x]), v[o] = d, v[x] = r[x];
      continue;
    }
    var h = v[o];
    if (!(d === h && !(d === void 0 && e.hasAttribute(o)))) {
      v[o] = d;
      var E = o[0] + o[1];
      if (E !== "$$") if (E === "on") {
        const k = {}, y = "$$" + o;
        let m = o.slice(2);
        var C = Jt(m);
        if (jt(m) && (m = m.slice(0, -7), k.capture = true), !C && h) {
          if (d != null) continue;
          e.removeEventListener(m, v[y], k), v[y] = null;
        }
        if (d != null) if (C) e[`__${m}`] = d, je([m]);
        else {
          let R = function(M) {
            v[o].call(this, M);
          };
          v[y] = Gt(m, e, R, k);
        }
        else C && (e[`__${m}`] = void 0);
      } else if (o === "style") H(e, o, d);
      else if (o === "autofocus") Ft(e, !!d);
      else if (!f && (o === "__value" || o === "value" && d != null)) e.value = e.__value = d;
      else if (o === "selected" && p) cr(e, d);
      else {
        var T = o;
        c || (T = Xt(T));
        var A = T === "defaultValue" || T === "defaultChecked";
        if (d == null && !f && !A) if (_[o] = null, T === "value" || T === "checked") {
          let k = e;
          const y = t === void 0;
          if (T === "value") {
            let m = k.defaultValue;
            k.removeAttribute(T), k.defaultValue = m, k.value = k.__value = y ? m : null;
          } else {
            let m = k.defaultChecked;
            k.removeAttribute(T), k.defaultChecked = m, k.checked = y ? m : false;
          }
        } else e.removeAttribute(o);
        else A || b.includes(T) && (f || typeof d != "string") ? (e[T] = d, T in _ && (_[T] = Dt)) : typeof d != "function" && H(e, T, d);
      }
    }
  }
  return w && q(true), v;
}
function dr(e, t, r = [], a = [], i = [], n, s = false, l = false) {
  Nt(i, r, a, (_) => {
    var f = void 0, c = {}, w = e.nodeName === "SELECT", v = false;
    if (ee(() => {
      var u = t(..._.map(N)), b = vr(e, f, u, n, s, l);
      v && w && "value" in u && ne(e, u.value);
      for (let h of Object.getOwnPropertySymbols(c)) u[h] || Z(c[h]);
      for (let h of Object.getOwnPropertySymbols(u)) {
        var g = u[h];
        h.description === Ot && (!f || g !== f[h]) && (c[h] && Z(c[h]), c[h] = J(() => nr(e, () => g))), b[h] = g;
      }
      f = b;
    }), w) {
      var p = e;
      le(() => {
        ne(p, f.value, true), et(p);
      });
    }
    v = true;
  });
}
function at(e) {
  return e.__attributes ?? (e.__attributes = { [tt]: e.nodeName.includes("-"), [rt]: e.namespaceURI === Mt });
}
var Me = /* @__PURE__ */ new Map();
function it(e) {
  var t = e.getAttribute("is") || e.nodeName, r = Me.get(t);
  if (r) return r;
  Me.set(t, r = []);
  for (var a, i = e, n = Element.prototype; n !== i; ) {
    a = Bt(i);
    for (var s in a) a[s].set && r.push(s);
    i = It(i);
  }
  return r;
}
const hr = () => performance.now(), U = { tick: (e) => requestAnimationFrame(e), now: () => hr(), tasks: /* @__PURE__ */ new Set() };
function nt() {
  const e = U.now();
  U.tasks.forEach((t) => {
    t.c(e) || (U.tasks.delete(t), t.f());
  }), U.tasks.size !== 0 && U.tick(nt);
}
function _r(e) {
  let t;
  return U.tasks.size === 0 && U.tick(nt), { promise: new Promise((r) => {
    U.tasks.add(t = { c: e, f: r });
  }), abort() {
    U.tasks.delete(t);
  } };
}
function re(e, t) {
  xe(() => {
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
function Ie(e) {
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
function st(e, t, r, a) {
  var i = (e & Yt) !== 0, n = (e & xt) !== 0, s = i && n, l = (e & zt) !== 0, _ = s ? "both" : i ? "in" : "out", f, c = t.inert, w = t.style.overflow, v, p;
  function u() {
    return xe(() => f ?? (f = r()(t, (a == null ? void 0 : a()) ?? {}, { direction: _ })));
  }
  var b = { is_global: l, in() {
    var _a;
    if (t.inert = c, !i) {
      p == null ? void 0 : p.abort(), (_a = p == null ? void 0 : p.reset) == null ? void 0 : _a.call(p);
      return;
    }
    n || (v == null ? void 0 : v.abort()), re(t, "introstart"), v = me(t, u(), p, 1, () => {
      re(t, "introend"), v == null ? void 0 : v.abort(), v = f = void 0, t.style.overflow = w;
    });
  }, out(C) {
    if (!n) {
      C == null ? void 0 : C(), f = void 0;
      return;
    }
    t.inert = true, re(t, "outrostart"), p = me(t, u(), v, 0, () => {
      re(t, "outroend"), C == null ? void 0 : C();
    });
  }, stop: () => {
    v == null ? void 0 : v.abort(), p == null ? void 0 : p.abort();
  } }, g = Pe;
  if ((g.transitions ?? (g.transitions = [])).push(b), i && Zt) {
    var h = l;
    if (!h) {
      for (var E = g.parent; E && (E.f & ze) !== 0; ) for (; (E = E.parent) && (E.f & Ht) === 0; ) ;
      h = !E || (E.f & qt) !== 0;
    }
    h && le(() => {
      $(() => b.in());
    });
  }
}
function me(e, t, r, a, i) {
  var n = a === 1;
  if (Vt(t)) {
    var s, l = false;
    return Ae(() => {
      if (!l) {
        var g = t({ direction: n ? "in" : "out" });
        s = me(e, g, r, a, i);
      }
    }), { abort: () => {
      l = true, s == null ? void 0 : s.abort();
    }, deactivate: () => s.deactivate(), reset: () => s.reset(), t: () => s.t() };
  }
  if (r == null ? void 0 : r.deactivate(), !(t == null ? void 0 : t.duration)) return i(), { abort: Q, deactivate: Q, reset: Q, t: () => a };
  const { delay: _ = 0, css: f, tick: c, easing: w = gr } = t;
  var v = [];
  if (n && r === void 0 && (c && c(0, 1), f)) {
    var p = Ie(f(0, 1));
    v.push(p, p);
  }
  var u = () => 1 - a, b = e.animate(v, { duration: _, fill: "forwards" });
  return b.onfinish = () => {
    b.cancel();
    var g = (r == null ? void 0 : r.t()) ?? 1 - a;
    r == null ? void 0 : r.abort();
    var h = a - g, E = t.duration * Math.abs(h), C = [];
    if (E > 0) {
      var T = false;
      if (f) for (var A = Math.ceil(E / 16.666666666666668), o = 0; o <= A; o += 1) {
        var d = g + h * w(o / A), k = Ie(f(d, 1 - d));
        C.push(k), T || (T = k.overflow === "hidden");
      }
      T && (e.style.overflow = "hidden"), u = () => {
        var y = b.currentTime;
        return g + h * w(y / E);
      }, c && _r(() => {
        if (b.playState !== "running") return false;
        var y = u();
        return c(y, 1 - y), true;
      });
    }
    b = e.animate(C, { duration: E, fill: "forwards" }), b.onfinish = () => {
      u = () => a, c == null ? void 0 : c(a, 1 - a), i();
    };
  }, { abort: () => {
    b && (b.cancel(), b.effect = null, b.onfinish = Q);
  }, deactivate: () => {
    i = Q;
  }, reset: () => {
    a === 0 && (c == null ? void 0 : c(1, 0));
  }, t: () => u() };
}
function Hr(e, t, r = t) {
  var a = /* @__PURE__ */ new WeakSet();
  ke(e, "input", async (i) => {
    var n = i ? e.defaultValue : e.value;
    if (n = be(e) ? ge(n) : n, r(n), D !== null && a.add(D), await Wt(), n !== (n = t())) {
      var s = e.selectionStart, l = e.selectionEnd, _ = e.value.length;
      if (e.value = n ?? "", l !== null) {
        var f = e.value.length;
        s === l && l === _ && f > _ ? (e.selectionStart = f, e.selectionEnd = f) : (e.selectionStart = s, e.selectionEnd = Math.min(l, f));
      }
    }
  }), (S && e.defaultValue !== e.value || $(t) == null && e.value) && (r(be(e) ? ge(e.value) : e.value), D !== null && a.add(D)), Ve(() => {
    var i = t();
    if (e === document.activeElement) {
      var n = Ye ?? D;
      if (a.has(n)) return;
    }
    be(e) && i === ge(e.value) || e.type === "date" && !i && !e.value || i !== e.value && (e.value = i ?? "");
  });
}
function qr(e, t, r = t) {
  ke(e, "change", (a) => {
    var i = a ? e.defaultChecked : e.checked;
    r(i);
  }), (S && e.defaultChecked !== e.checked || $(t) == null) && r(e.checked), Ve(() => {
    var a = t();
    e.checked = !!a;
  });
}
function be(e) {
  var t = e.type;
  return t === "number" || t === "range";
}
function ge(e) {
  return e === "" ? null : +e;
}
const wr = (e) => e;
function ot(e) {
  const t = e - 1;
  return t * t * t + 1;
}
function lt(e, { delay: t = 0, duration: r = 400, easing: a = wr } = {}) {
  const i = +getComputedStyle(e).opacity;
  return { delay: t, duration: r, easing: a, css: (n) => `opacity: ${n * i}` };
}
function zr(e, { delay: t = 0, duration: r = 400, easing: a = ot, axis: i = "y" } = {}) {
  const n = getComputedStyle(e), s = +n.opacity, l = i === "y" ? "height" : "width", _ = parseFloat(n[l]), f = i === "y" ? ["top", "bottom"] : ["left", "right"], c = f.map((h) => `${h[0].toUpperCase()}${h.slice(1)}`), w = parseFloat(n[`padding${c[0]}`]), v = parseFloat(n[`padding${c[1]}`]), p = parseFloat(n[`margin${c[0]}`]), u = parseFloat(n[`margin${c[1]}`]), b = parseFloat(n[`border${c[0]}Width`]), g = parseFloat(n[`border${c[1]}Width`]);
  return { delay: t, duration: r, easing: a, css: (h) => `overflow: hidden;opacity: ${Math.min(h * 20, 1) * s};${l}: ${h * _}px;padding-${f[0]}: ${h * w}px;padding-${f[1]}: ${h * v}px;margin-${f[0]}: ${h * p}px;margin-${f[1]}: ${h * u}px;border-${f[0]}-width: ${h * b}px;border-${f[1]}-width: ${h * g}px;min-${l}: 0` };
}
function Oe(e, t) {
  for (const r in t) e[r] = t[r];
  return e;
}
function Yr({ fallback: e, ...t }) {
  const r = /* @__PURE__ */ new Map(), a = /* @__PURE__ */ new Map();
  function i(s, l, _) {
    const { delay: f = 0, duration: c = (o) => Math.sqrt(o) * 30, easing: w = ot } = Oe(Oe({}, t), _), v = s.getBoundingClientRect(), p = l.getBoundingClientRect(), u = v.left - p.left, b = v.top - p.top, g = v.width / p.width, h = v.height / p.height, E = Math.sqrt(u * u + b * b), C = getComputedStyle(l), T = C.transform === "none" ? "" : C.transform, A = +C.opacity;
    return { delay: f, duration: typeof c == "function" ? c(E) : c, easing: w, css: (o, d) => `
			   opacity: ${o * A};
			   transform-origin: top left;
			   transform: ${T} translate(${d * u}px,${d * b}px) scale(${o + (1 - o) * g}, ${o + (1 - o) * h});
		   ` };
  }
  function n(s, l, _) {
    return (f, c) => (s.set(c.key, f), () => {
      if (l.has(c.key)) {
        const w = l.get(c.key);
        return l.delete(c.key), i(w, f, c);
      }
      return s.delete(c.key), e && e(f, c, _);
    });
  }
  return [n(a, r, false), n(r, a, true)];
}
var yr = z('<div><div class="loading svelte-1a5pdw0"><div class="loading-1 svelte-1a5pdw0"></div> <div class="loading-2 svelte-1a5pdw0"></div> <div class="loading-3 svelte-1a5pdw0"></div></div></div>');
function pr(e, t) {
  let r = L(t, "background", 3, false), a = L(t, "color", 3, "var(--col-text)"), i = L(t, "global", 3, false), n = L(t, "offset", 3, 0);
  var s = yr();
  let l;
  var _ = B(s), f = B(_);
  let c;
  var w = ae(f, 2);
  let v;
  var p = ae(w, 2);
  let u;
  F(_), F(s), Te(() => {
    l = $e(s, 1, "container svelte-1a5pdw0", null, l, { global: i(), local: !i(), background: r() }), V(_, `margin-top: ${n() ?? ""}px;`), c = V(f, "", c, { background: a() }), v = V(w, "", v, { background: a() }), u = V(p, "", u, { background: a() });
  }), st(3, s, () => lt, () => ({ duration: 100 })), P(e, s);
}
var mr = z('<div class="load svelte-18sv61c"><!></div>'), Er = z('<div class="font-label"><!></div>'), Ar = z("<button><!></button>");
function xr(e, t) {
  We(t, true);
  let r = L(t, "type", 3, "button"), a = L(t, "role", 3, "button"), i = L(t, "ref", 15), n = L(t, "level", 3, 2), s = L(t, "isDisabled", 3, false), l = L(t, "isLoading", 3, false), _ = L(t, "destructive", 3, false), f = L(t, "invisible", 3, false), c = L(t, "invisibleOutline", 3, false), w = rr(t, ["$$slots", "$$events", "$$legacy", "type", "role", "ref", "id", "ariaLabel", "ariaControls", "ariaCurrent", "level", "width", "isDisabled", "isLoading", "destructive", "invisible", "invisibleOutline", "popovertarget", "popovertargetaction", "onclick", "onLeft", "onRight", "onUp", "onDown", "children"]), v = ve(() => {
    if (f()) return "invisible";
    if (_()) return "destructive";
    switch (n()) {
      case 2:
        return "l2";
      case 3:
        return "l3";
      default:
        return "l1";
    }
  }), p = K(!l()), u = ve(() => s() || l());
  Qe(() => {
    l() ? setTimeout(() => {
      I(p, false);
    }, 120) : setTimeout(() => {
      I(p, true);
    }, 120);
  });
  function b() {
    if (_()) return "var(--btn-text)";
    switch (n()) {
      case 2:
        return "hsl(var(--action))";
      case 3:
        return "hsl(var(--action))";
      default:
        return "var(--btn-text)";
    }
  }
  function g(A) {
    var _a, _b, _c, _d, _e2;
    switch (A.code) {
      case "Enter":
        (_a = t.onclick) == null ? void 0 : _a.call(t, A);
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
  var h = Ar();
  dr(h, () => ({ role: a(), type: r(), id: t.id, "aria-label": t.ariaLabel, "aria-controls": t.ariaControls, "aria-current": t.ariaCurrent, class: N(v), "data-isloading": l(), onclick: t.onclick, onkeydown: g, disabled: N(u), "aria-disabled": N(u), popovertarget: t.popovertarget, popovertargetaction: t.popovertargetaction, ...w, [j]: { invisibleOutline: c() }, [x]: { width: t.width } }), void 0, void 0, void 0, "svelte-18sv61c");
  var E = B(h);
  {
    var C = (A) => {
      var o = mr(), d = B(o);
      {
        let k = ve(b);
        pr(d, { background: false, get color() {
          return N(k);
        } });
      }
      F(o), P(A, o);
    }, T = (A) => {
      var o = $t(), d = Qt(o);
      {
        var k = (y) => {
          var m = Er(), R = B(m);
          Je(R, () => t.children), F(m), st(1, m, () => lt), P(y, m);
        };
        ie(d, (y) => {
          N(p) && y(k);
        }, true);
      }
      P(A, o);
    };
    ie(E, (A) => {
      l() ? A(C) : A(T, false);
    });
  }
  F(h), Ge(h, (A) => i(A), () => i()), P(e, h), Ke();
}
const kr = Kt(void 0), ft = "/dashboard/api";
async function Vr(e) {
  let t = await fetch(`${ft}${e}`, { method: "GET" });
  return ut(t);
}
async function Wr(e, t) {
  let r = await fetch(`${ft}${e}`, { method: "POST", body: t });
  return ut(r);
}
function ut(e) {
  return e.status === 401 && kr.set(void 0), e;
}
var Tr = er(`<svg fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M2.036 12.322a1.012 1.012 0 010-.639C3.423 7.51 7.36 4.5 12 4.5c4.638 0 8.573 3.007 9.963
            7.178.07.207.07.431 0 .639C20.577 16.49 16.64 19.5 12 19.5c-4.638 0-8.573-3.007-9.963-7.178z"></path><path stroke-linecap="round" stroke-linejoin="round" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"></path></svg>`);
function Qr(e, t) {
  let r = L(t, "color", 8, "var(--col-ok)"), a = L(t, "opacity", 8, 0.9), i = L(t, "width", 8, "1.5rem");
  var n = Tr();
  H(n, "stroke-width", 2), Te(() => {
    H(n, "width", i()), H(n, "color", r()), H(n, "opacity", a());
  }), P(e, n);
}
const Cr = `-- comments will be ignored but only a single query is allowed
-- press CTRL + Enter to execute
SELECT 1`, Sr = { id: "SELECT 1", query: Cr }, Kr = "--!auto-query";
let jr = ye([Sr]);
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
var Lr = z('<div class="relative"><div role="none" class="right svelte-19ulb1h"></div></div>'), Rr = z('<div class="relative"><div role="none" class="bottom svelte-19ulb1h"></div></div>'), Nr = z('<div><div class="children svelte-19ulb1h"><div class="inner svelte-19ulb1h"><!></div> <!></div> <!></div>');
function Xr(e, t) {
  We(t, true);
  let r = L(t, "minWidthPx", 3, 50), a = L(t, "minHeightPx", 3, 50), i, n = K(void 0), s = K(void 0), l = K(ye($(() => t.initialWidthPx))), _ = K(ye($(() => t.initialHeightPx)));
  Qe(() => {
    f();
  });
  function f() {
    var _a;
    if (i) {
      let y = i.getBoundingClientRect();
      t.resizeRight && (I(s, y.left, true), I(l, y.width, true)), t.resizeBottom && (I(n, y.top, true), I(_, y.height, true)), (_a = t.onResizeBottom) == null ? void 0 : _a.call(t, y.bottom);
    }
  }
  function c() {
    f(), window.addEventListener("mousemove", v), window.addEventListener("mouseup", w, { once: true });
  }
  function w() {
    window.removeEventListener("mousemove", v), f();
  }
  function v(y) {
    let m = window.scrollX + y.x - (N(s) || 0);
    m < r() ? I(l, r()) : I(l, m);
  }
  function p() {
    f(), window.addEventListener("mousemove", b), window.addEventListener("mouseup", u, { once: true });
  }
  function u() {
    window.removeEventListener("mousemove", b), f();
  }
  function b(y) {
    console.log(window.screenY, y.y);
    let m = window.screenY + y.clientY - (N(n) || 0);
    if (m < a() ? I(_, a()) : I(_, m), i && t.onResizeBottom) {
      let R = i.getBoundingClientRect();
      t.onResizeBottom(R.bottom);
    }
  }
  var g = Nr();
  let h;
  var E = B(g), C = B(E), T = B(C);
  Je(T, () => t.children), F(C);
  var A = ae(C, 2);
  {
    var o = (y) => {
      var m = Lr(), R = B(m);
      R.__mousedown = c, F(m), P(y, m);
    };
    ie(A, (y) => {
      t.resizeRight && y(o);
    });
  }
  F(E);
  var d = ae(E, 2);
  {
    var k = (y) => {
      var m = Rr(), R = B(m);
      R.__mousedown = p, F(m), P(y, m);
    };
    ie(d, (y) => {
      t.resizeBottom && y(k);
    });
  }
  F(g), Ge(g, (y) => i = y, () => i), Te(() => h = V(g, "", h, { width: N(l) && `${N(l)}px`, height: N(_) && `${N(_)}px`, border: t.border, padding: t.padding })), P(e, g), Ke();
}
je(["mousedown"]);
export {
  ft as A,
  xr as B,
  Sr as D,
  Qr as I,
  jr as Q,
  Xr as R,
  H as a,
  V as b,
  Yr as c,
  Hr as d,
  zr as e,
  Ur as f,
  kr as g,
  ut as h,
  Dr as i,
  Fr as j,
  $e as k,
  or as l,
  Vr as m,
  Gr as n,
  Kr as o,
  cr as p,
  Pr as q,
  ur as r,
  Je as s,
  st as t,
  qr as u,
  Wr as v,
  Cr as w
};
