import { m as te, n as S, o as rt, g as L, T as at, q as it, v as nt, w as be, x as ie, y as Q, C as D, al as ot, z as Le, A as K, B as st, ar as ne, ap as Re, aa as J, as as we, J as lt, at as ye, ab as Ne, au as de, av as ft, ao as ut, aw as ct, ax as j, ay as _e, az as vt, ae as dt, af as _t, ac as ht, aA as Ie, G as Me, aB as gt, E as Oe, I as U, D as re, b as z, aC as bt, aD as wt, h as yt, aE as pt, aF as kt, aG as mt, aH as Et, aI as At, aJ as Tt, aK as Ct, aL as St, aM as Lt, aN as Rt, aO as Nt, aP as It, a7 as pe, a8 as ke, a9 as Mt, F as qe, aQ as Ot, k as N, s as Z, l as I, t as he, p as Be, a4 as V, a as Fe, j as De, O as R, a6 as oe, aR as qt, X as ce } from "./CxznHt52.js";
import { l as ge, i as Bt, b as Ft, d as Ue, e as Dt, n as Ut, g as Pt, j as xt, k as zt, w as Ht, f as F, a as B, o as Yt } from "./C_iGe9Tc.js";
import { p as C, i as $, b as Pe, r as Vt } from "./DQ-LUyGE.js";
import "./C0SPxqjE.js";
function Tr(e, t) {
  return t;
}
function Gt(e, t, r, a) {
  for (var i = [], n = t.length, o = 0; o < n; o++) ft(t[o].e, i, true);
  var s = n > 0 && i.length === 0 && r !== null;
  if (s) {
    var w = r.parentNode;
    ut(w), w.append(r), a.clear(), O(e, t[0].prev, t[n - 1].next);
  }
  ct(i, () => {
    for (var l = 0; l < n; l++) {
      var u = t[l];
      s || (a.delete(u.k), O(e, u.prev, u.next)), j(u.e, !s);
    }
  });
}
function Cr(e, t, r, a, i, n = null) {
  var o = e, s = { flags: t, items: /* @__PURE__ */ new Map(), first: null }, w = (t & Ie) !== 0;
  if (w) {
    var l = e;
    o = S ? ie(_t(l)) : l.appendChild(ht());
  }
  S && rt();
  var u = null, y = false, f = at(() => {
    var d = r();
    return Ne(d) ? d : d == null ? [] : Re(d);
  });
  te(() => {
    var d = L(f), c = d.length;
    if (y && c === 0) return;
    y = c === 0;
    let p = false;
    if (S) {
      var k = it(o) === nt;
      k !== (c === 0) && (o = be(), ie(o), Q(false), p = true);
    }
    if (S) {
      for (var h = null, E, g = 0; g < c; g++) {
        if (D.nodeType === 8 && D.data === ot) {
          o = D, p = true, Q(false);
          break;
        }
        var v = d[g], _ = a(v, g);
        E = xe(D, s, h, null, v, _, g, i, t, r), s.items.set(_, E), h = E;
      }
      c > 0 && ie(be());
    }
    S || Qt(d, s, o, i, t, a, r), n !== null && (c === 0 ? u ? Le(u) : u = K(() => n(o)) : u !== null && st(u, () => {
      u = null;
    })), p && Q(true), L(f);
  }), S && (o = D);
}
function Qt(e, t, r, a, i, n, o) {
  var _a, _b, _c, _d;
  var s = (i & gt) !== 0, w = (i & (_e | de)) !== 0, l = e.length, u = t.items, y = t.first, f = y, d, c = null, p, k = [], h = [], E, g, v, _;
  if (s) for (_ = 0; _ < l; _ += 1) E = e[_], g = n(E, _), v = u.get(g), v !== void 0 && ((_a = v.a) == null ? void 0 : _a.measure(), (p ?? (p = /* @__PURE__ */ new Set())).add(v));
  for (_ = 0; _ < l; _ += 1) {
    if (E = e[_], g = n(E, _), v = u.get(g), v === void 0) {
      var A = f ? f.e.nodes_start : r;
      c = xe(A, t, c, c === null ? t.first : c.next, E, g, _, a, i, o), u.set(g, c), k = [], h = [], f = c.next;
      continue;
    }
    if (w && Wt(v, E, _, i), (v.e.f & ne) !== 0 && (Le(v.e), s && ((_b = v.a) == null ? void 0 : _b.unfix(), (p ?? (p = /* @__PURE__ */ new Set())).delete(v))), v !== f) {
      if (d !== void 0 && d.has(v)) {
        if (k.length < h.length) {
          var b = h[0], m;
          c = b.prev;
          var T = k[0], H = k[k.length - 1];
          for (m = 0; m < k.length; m += 1) me(k[m], b, r);
          for (m = 0; m < h.length; m += 1) d.delete(h[m]);
          O(t, T.prev, H.next), O(t, c, T), O(t, H, b), f = b, c = H, _ -= 1, k = [], h = [];
        } else d.delete(v), me(v, f, r), O(t, v.prev, v.next), O(t, v, c === null ? t.first : c.next), O(t, c, v), c = v;
        continue;
      }
      for (k = [], h = []; f !== null && f.k !== g; ) (f.e.f & ne) === 0 && (d ?? (d = /* @__PURE__ */ new Set())).add(f), h.push(f), f = f.next;
      if (f === null) continue;
      v = f;
    }
    k.push(v), c = v, f = v.next;
  }
  if (f !== null || d !== void 0) {
    for (var Y = d === void 0 ? [] : Re(d); f !== null; ) (f.e.f & ne) === 0 && Y.push(f), f = f.next;
    var ae = Y.length;
    if (ae > 0) {
      var tt = (i & Ie) !== 0 && l === 0 ? r : null;
      if (s) {
        for (_ = 0; _ < ae; _ += 1) (_c = Y[_].a) == null ? void 0 : _c.measure();
        for (_ = 0; _ < ae; _ += 1) (_d = Y[_].a) == null ? void 0 : _d.fix();
      }
      Gt(t, Y, tt, u);
    }
  }
  s && Me(() => {
    var _a2;
    if (p !== void 0) for (v of p) (_a2 = v.a) == null ? void 0 : _a2.apply();
  }), J.first = t.first && t.first.e, J.last = c && c.e;
}
function Wt(e, t, r, a) {
  (a & _e) !== 0 && we(e.v, t), (a & de) !== 0 ? we(e.i, r) : e.i = r;
}
function xe(e, t, r, a, i, n, o, s, w, l) {
  var u = (w & _e) !== 0, y = (w & vt) === 0, f = u ? y ? lt(i) : ye(i) : i, d = (w & de) === 0 ? o : ye(o), c = { i: d, v: f, k: n, a: null, e: null, prev: r, next: a };
  try {
    return c.e = K(() => s(e, f, d, l), S), c.e.prev = r && r.e, c.e.next = a && a.e, r === null ? t.first = c : (r.next = c, r.e.next = c.e), a !== null && (a.prev = c, a.e.prev = c.e), c;
  } finally {
  }
}
function me(e, t, r) {
  for (var a = e.next ? e.next.e.nodes_start : r, i = t ? t.e.nodes_start : r, n = e.e.nodes_start; n !== a; ) {
    var o = dt(n);
    i.before(n), n = o;
  }
}
function O(e, t, r) {
  t === null ? e.first = r : (t.next = r, t.e.next = r && r.e), r !== null && (r.prev = t, r.e.prev = t && t.e);
}
function ze(e, t, ...r) {
  var a = e, i = U, n;
  te(() => {
    i !== (i = t()) && (n && (j(n), n = null), n = K(() => i(a, ...r)));
  }, Oe), S && (a = D);
}
function jt(e, t) {
  var r = void 0, a;
  te(() => {
    r !== (r = t()) && (a && (j(a), a = null), r && (a = K(() => {
      re(() => r(e));
    })));
  });
}
function He(e) {
  var t, r, a = "";
  if (typeof e == "string" || typeof e == "number") a += e;
  else if (typeof e == "object") if (Array.isArray(e)) {
    var i = e.length;
    for (t = 0; t < i; t++) e[t] && (r = He(e[t])) && (a && (a += " "), a += r);
  } else for (r in e) e[r] && (a && (a += " "), a += r);
  return a;
}
function Kt() {
  for (var e, t, r = 0, a = "", i = arguments.length; r < i; r++) (e = arguments[r]) && (t = He(e)) && (a && (a += " "), a += t);
  return a;
}
function Xt(e) {
  return typeof e == "object" ? Kt(e) : e ?? "";
}
const Ee = [...` 	
\r\f\xA0\v\uFEFF`];
function Jt(e, t, r) {
  var a = e == null ? "" : "" + e;
  if (t && (a = a ? a + " " + t : t), r) {
    for (var i in r) if (r[i]) a = a ? a + " " + i : i;
    else if (a.length) for (var n = i.length, o = 0; (o = a.indexOf(i, o)) >= 0; ) {
      var s = o + n;
      (o === 0 || Ee.includes(a[o - 1])) && (s === a.length || Ee.includes(a[s])) ? a = (o === 0 ? "" : a.substring(0, o)) + a.substring(s + 1) : o = s;
    }
  }
  return a === "" ? null : a;
}
function Ae(e, t = false) {
  var r = t ? " !important;" : ";", a = "";
  for (var i in e) {
    var n = e[i];
    n != null && n !== "" && (a += " " + i + ": " + n + r);
  }
  return a;
}
function se(e) {
  return e[0] !== "-" || e[1] !== "-" ? e.toLowerCase() : e;
}
function Zt(e, t) {
  if (t) {
    var r = "", a, i;
    if (Array.isArray(t) ? (a = t[0], i = t[1]) : a = t, e) {
      e = String(e).replaceAll(/\s*\/\*.*?\*\/\s*/g, "").trim();
      var n = false, o = 0, s = false, w = [];
      a && w.push(...Object.keys(a).map(se)), i && w.push(...Object.keys(i).map(se));
      var l = 0, u = -1;
      const p = e.length;
      for (var y = 0; y < p; y++) {
        var f = e[y];
        if (s ? f === "/" && e[y - 1] === "*" && (s = false) : n ? n === f && (n = false) : f === "/" && e[y + 1] === "*" ? s = true : f === '"' || f === "'" ? n = f : f === "(" ? o++ : f === ")" && o--, !s && n === false && o === 0) {
          if (f === ":" && u === -1) u = y;
          else if (f === ";" || y === p - 1) {
            if (u !== -1) {
              var d = se(e.substring(l, u).trim());
              if (!w.includes(d)) {
                f !== ";" && y++;
                var c = e.substring(l, y).trim();
                r += " " + c + ";";
              }
            }
            l = y + 1, u = -1;
          }
        }
      }
    }
    return a && (r += Ae(a)), i && (r += Ae(i, true)), r = r.trim(), r === "" ? null : r;
  }
  return e == null ? null : String(e);
}
function Ye(e, t, r, a, i, n) {
  var o = e.__className;
  if (S || o !== r || o === void 0) {
    var s = Jt(r, a, n);
    (!S || s !== e.getAttribute("class")) && (s == null ? e.removeAttribute("class") : t ? e.className = s : e.setAttribute("class", s)), e.__className = r;
  } else if (n && i !== n) for (var w in n) {
    var l = !!n[w];
    (i == null || l !== !!i[w]) && e.classList.toggle(w, l);
  }
  return n;
}
function le(e, t = {}, r, a) {
  for (var i in r) {
    var n = r[i];
    t[i] !== n && (r[i] == null ? e.style.removeProperty(i) : e.style.setProperty(i, n, a));
  }
}
function x(e, t, r, a) {
  var i = e.__style;
  if (S || i !== t) {
    var n = Zt(t, a);
    (!S || n !== e.getAttribute("style")) && (n == null ? e.removeAttribute("style") : e.style.cssText = n), e.__style = t;
  } else a && (Array.isArray(a) ? (le(e, r == null ? void 0 : r[0], a[0]), le(e, r == null ? void 0 : r[1], a[1], "important")) : le(e, r, a));
  return a;
}
function ee(e, t, r) {
  if (e.multiple) {
    if (t == null) return;
    if (!Ne(t)) return bt();
    for (var a of e.options) a.selected = t.includes(W(a));
    return;
  }
  for (a of e.options) {
    var i = W(a);
    if (wt(i, t)) {
      a.selected = true;
      return;
    }
  }
  (!r || t !== void 0) && (e.selectedIndex = -1);
}
function Ve(e, t) {
  let r = true;
  re(() => {
    t && ee(e, z(t), r), r = false;
    var a = new MutationObserver(() => {
      var i = e.__value;
      ee(e, i);
    });
    return a.observe(e, { childList: true, subtree: true, attributes: true, attributeFilter: ["value"] }), () => {
      a.disconnect();
    };
  });
}
function Sr(e, t, r = t) {
  var a = true;
  ge(e, "change", (i) => {
    var n = i ? "[selected]" : ":checked", o;
    if (e.multiple) o = [].map.call(e.querySelectorAll(n), W);
    else {
      var s = e.querySelector(n) ?? e.querySelector("option:not([disabled])");
      o = s && W(s);
    }
    r(o);
  }), re(() => {
    var i = t();
    if (ee(e, i, a), a && i === void 0) {
      var n = e.querySelector(":checked");
      n !== null && (i = W(n), r(i));
    }
    e.__value = i, a = false;
  }), Ve(e);
}
function W(e) {
  return "__value" in e ? e.__value : e.value;
}
const G = Symbol("class"), P = Symbol("style"), Ge = Symbol("is custom element"), Qe = Symbol("is html");
function Lr(e) {
  if (S) {
    var t = false, r = () => {
      if (!t) {
        if (t = true, e.hasAttribute("value")) {
          var a = e.value;
          q(e, "value", null), e.value = a;
        }
        if (e.hasAttribute("checked")) {
          var i = e.checked;
          q(e, "checked", null), e.checked = i;
        }
      }
    };
    e.__on_r = r, Tt(r), xt();
  }
}
function $t(e, t) {
  t ? e.hasAttribute("selected") || e.setAttribute("selected", "") : e.removeAttribute("selected");
}
function q(e, t, r, a) {
  var i = We(e);
  S && (i[t] = e.getAttribute(t), t === "src" || t === "srcset" || t === "href" && e.nodeName === "LINK") || i[t] !== (i[t] = r) && (t === "loading" && (e[At] = r), r == null ? e.removeAttribute(t) : typeof r != "string" && je(e).includes(t) ? e[t] = r : e.setAttribute(t, r));
}
function er(e, t, r, a, i = false) {
  var n = We(e), o = n[Ge], s = !n[Qe];
  let w = S && o;
  w && Q(false);
  var l = t || {}, u = e.tagName === "OPTION";
  for (var y in t) y in r || (r[y] = null);
  r.class ? r.class = Xt(r.class) : r.class = null, r[P] && (r.style ?? (r.style = null));
  var f = je(e);
  for (const g in r) {
    let v = r[g];
    if (u && g === "value" && v == null) {
      e.value = e.__value = "", l[g] = v;
      continue;
    }
    if (g === "class") {
      var d = e.namespaceURI === "http://www.w3.org/1999/xhtml";
      Ye(e, d, v, a, t == null ? void 0 : t[G], r[G]), l[g] = v, l[G] = r[G];
      continue;
    }
    if (g === "style") {
      x(e, v, t == null ? void 0 : t[P], r[P]), l[g] = v, l[P] = r[P];
      continue;
    }
    var c = l[g];
    if (v !== c) {
      l[g] = v;
      var p = g[0] + g[1];
      if (p !== "$$") if (p === "on") {
        const _ = {}, A = "$$" + g;
        let b = g.slice(2);
        var k = Pt(b);
        if (Bt(b) && (b = b.slice(0, -7), _.capture = true), !k && c) {
          if (v != null) continue;
          e.removeEventListener(b, l[A], _), l[A] = null;
        }
        if (v != null) if (k) e[`__${b}`] = v, Ue([b]);
        else {
          let m = function(T) {
            l[g].call(this, T);
          };
          l[A] = Ft(b, e, m, _);
        }
        else k && (e[`__${b}`] = void 0);
      } else if (g === "style") q(e, g, v);
      else if (g === "autofocus") Dt(e, !!v);
      else if (!o && (g === "__value" || g === "value" && v != null)) e.value = e.__value = v;
      else if (g === "selected" && u) $t(e, v);
      else {
        var h = g;
        s || (h = Ut(h));
        var E = h === "defaultValue" || h === "defaultChecked";
        if (v == null && !o && !E) if (n[g] = null, h === "value" || h === "checked") {
          let _ = e;
          const A = t === void 0;
          if (h === "value") {
            let b = _.defaultValue;
            _.removeAttribute(h), _.defaultValue = b, _.value = _.__value = A ? b : null;
          } else {
            let b = _.defaultChecked;
            _.removeAttribute(h), _.defaultChecked = b, _.checked = A ? b : false;
          }
        } else e.removeAttribute(g);
        else E || f.includes(h) && (o || typeof v != "string") ? e[h] = v : typeof v != "function" && q(e, h, v);
      }
    }
  }
  return w && Q(true), l;
}
function tr(e, t, r = [], a, i = false, n = yt) {
  const o = r.map(n);
  var s = void 0, w = {}, l = e.nodeName === "SELECT", u = false;
  te(() => {
    var y = t(...o.map(L));
    er(e, s, y, a, i), u && l && "value" in y && ee(e, y.value, false);
    for (let d of Object.getOwnPropertySymbols(w)) y[d] || j(w[d]);
    for (let d of Object.getOwnPropertySymbols(y)) {
      var f = y[d];
      d.description === mt && (!s || f !== s[d]) && (w[d] && j(w[d]), w[d] = K(() => jt(e, () => f)));
    }
    s = y;
  }), l && Ve(e, () => s.value), u = true;
}
function We(e) {
  return e.__attributes ?? (e.__attributes = { [Ge]: e.nodeName.includes("-"), [Qe]: e.namespaceURI === pt });
}
var Te = /* @__PURE__ */ new Map();
function je(e) {
  var t = Te.get(e.nodeName);
  if (t) return t;
  Te.set(e.nodeName, t = []);
  for (var r, a = e, i = Element.prototype; i !== a; ) {
    r = Et(a);
    for (var n in r) r[n].set && t.push(n);
    a = kt(a);
  }
  return t;
}
const rr = () => performance.now(), M = { tick: (e) => requestAnimationFrame(e), now: () => rr(), tasks: /* @__PURE__ */ new Set() };
function Ke() {
  const e = M.now();
  M.tasks.forEach((t) => {
    t.c(e) || (M.tasks.delete(t), t.f());
  }), M.tasks.size !== 0 && M.tick(Ke);
}
function ar(e) {
  let t;
  return M.tasks.size === 0 && M.tick(Ke), { promise: new Promise((r) => {
    M.tasks.add(t = { c: e, f: r });
  }), abort() {
    M.tasks.delete(t);
  } };
}
function X(e, t) {
  Ht(() => {
    e.dispatchEvent(new CustomEvent(t));
  });
}
function ir(e) {
  if (e === "float") return "cssFloat";
  if (e === "offset") return "cssOffset";
  if (e.startsWith("--")) return e;
  const t = e.split("-");
  return t.length === 1 ? t[0] : t[0] + t.slice(1).map((r) => r[0].toUpperCase() + r.slice(1)).join("");
}
function Ce(e) {
  const t = {}, r = e.split(";");
  for (const a of r) {
    const [i, n] = a.split(":");
    if (!i || n === void 0) break;
    const o = ir(i.trim());
    t[o] = n.trim();
  }
  return t;
}
const nr = (e) => e;
function Xe(e, t, r, a) {
  var i = (e & Rt) !== 0, n = (e & Nt) !== 0, o = i && n, s = (e & Lt) !== 0, w = o ? "both" : i ? "in" : "out", l, u = t.inert, y = t.style.overflow, f, d;
  function c() {
    var g = Mt, v = J;
    pe(null), ke(null);
    try {
      return l ?? (l = r()(t, (a == null ? void 0 : a()) ?? {}, { direction: w }));
    } finally {
      pe(g), ke(v);
    }
  }
  var p = { is_global: s, in() {
    var _a;
    if (t.inert = u, !i) {
      d == null ? void 0 : d.abort(), (_a = d == null ? void 0 : d.reset) == null ? void 0 : _a.call(d);
      return;
    }
    n || (f == null ? void 0 : f.abort()), X(t, "introstart"), f = ve(t, c(), d, 1, () => {
      X(t, "introend"), f == null ? void 0 : f.abort(), f = l = void 0, t.style.overflow = y;
    });
  }, out(g) {
    if (!n) {
      g == null ? void 0 : g(), l = void 0;
      return;
    }
    t.inert = true, X(t, "outrostart"), d = ve(t, c(), f, 0, () => {
      X(t, "outroend"), g == null ? void 0 : g();
    });
  }, stop: () => {
    f == null ? void 0 : f.abort(), d == null ? void 0 : d.abort();
  } }, k = J;
  if ((k.transitions ?? (k.transitions = [])).push(p), i && zt) {
    var h = s;
    if (!h) {
      for (var E = k.parent; E && (E.f & Oe) !== 0; ) for (; (E = E.parent) && (E.f & Ct) === 0; ) ;
      h = !E || (E.f & St) !== 0;
    }
    h && re(() => {
      z(() => p.in());
    });
  }
}
function ve(e, t, r, a, i) {
  var n = a === 1;
  if (It(t)) {
    var o, s = false;
    return Me(() => {
      if (!s) {
        var k = t({ direction: n ? "in" : "out" });
        o = ve(e, k, r, a, i);
      }
    }), { abort: () => {
      s = true, o == null ? void 0 : o.abort();
    }, deactivate: () => o.deactivate(), reset: () => o.reset(), t: () => o.t() };
  }
  if (r == null ? void 0 : r.deactivate(), !(t == null ? void 0 : t.duration)) return i(), { abort: U, deactivate: U, reset: U, t: () => a };
  const { delay: w = 0, css: l, tick: u, easing: y = nr } = t;
  var f = [];
  if (n && r === void 0 && (u && u(0, 1), l)) {
    var d = Ce(l(0, 1));
    f.push(d, d);
  }
  var c = () => 1 - a, p = e.animate(f, { duration: w, fill: "forwards" });
  return p.onfinish = () => {
    p.cancel();
    var k = (r == null ? void 0 : r.t()) ?? 1 - a;
    r == null ? void 0 : r.abort();
    var h = a - k, E = t.duration * Math.abs(h), g = [];
    if (E > 0) {
      var v = false;
      if (l) for (var _ = Math.ceil(E / 16.666666666666668), A = 0; A <= _; A += 1) {
        var b = k + h * y(A / _), m = Ce(l(b, 1 - b));
        g.push(m), v || (v = m.overflow === "hidden");
      }
      v && (e.style.overflow = "hidden"), c = () => {
        var T = p.currentTime;
        return k + h * y(T / E);
      }, u && ar(() => {
        if (p.playState !== "running") return false;
        var T = c();
        return u(T, 1 - T), true;
      });
    }
    p = e.animate(g, { duration: E, fill: "forwards" }), p.onfinish = () => {
      c = () => a, u == null ? void 0 : u(a, 1 - a), i();
    };
  }, { abort: () => {
    p && (p.cancel(), p.effect = null, p.onfinish = U);
  }, deactivate: () => {
    i = U;
  }, reset: () => {
    a === 0 && (u == null ? void 0 : u(1, 0));
  }, t: () => c() };
}
function Rr(e, t, r = t) {
  var a = Ot();
  ge(e, "input", (i) => {
    var n = i ? e.defaultValue : e.value;
    if (n = fe(e) ? ue(n) : n, r(n), a && n !== (n = t())) {
      var o = e.selectionStart, s = e.selectionEnd;
      e.value = n ?? "", s !== null && (e.selectionStart = o, e.selectionEnd = Math.min(s, e.value.length));
    }
  }), (S && e.defaultValue !== e.value || z(t) == null && e.value) && r(fe(e) ? ue(e.value) : e.value), qe(() => {
    var i = t();
    fe(e) && i === ue(e.value) || e.type === "date" && !i && !e.value || i !== e.value && (e.value = i ?? "");
  });
}
function Nr(e, t, r = t) {
  ge(e, "change", (a) => {
    var i = a ? e.defaultChecked : e.checked;
    r(i);
  }), (S && e.defaultChecked !== e.checked || z(t) == null) && r(e.checked), qe(() => {
    var a = t();
    e.checked = !!a;
  });
}
function fe(e) {
  var t = e.type;
  return t === "number" || t === "range";
}
function ue(e) {
  return e === "" ? null : +e;
}
const or = (e) => e;
function Je(e) {
  const t = e - 1;
  return t * t * t + 1;
}
function Ze(e, { delay: t = 0, duration: r = 400, easing: a = or } = {}) {
  const i = +getComputedStyle(e).opacity;
  return { delay: t, duration: r, easing: a, css: (n) => `opacity: ${n * i}` };
}
function Ir(e, { delay: t = 0, duration: r = 400, easing: a = Je, axis: i = "y" } = {}) {
  const n = getComputedStyle(e), o = +n.opacity, s = i === "y" ? "height" : "width", w = parseFloat(n[s]), l = i === "y" ? ["top", "bottom"] : ["left", "right"], u = l.map((h) => `${h[0].toUpperCase()}${h.slice(1)}`), y = parseFloat(n[`padding${u[0]}`]), f = parseFloat(n[`padding${u[1]}`]), d = parseFloat(n[`margin${u[0]}`]), c = parseFloat(n[`margin${u[1]}`]), p = parseFloat(n[`border${u[0]}Width`]), k = parseFloat(n[`border${u[1]}Width`]);
  return { delay: t, duration: r, easing: a, css: (h) => `overflow: hidden;opacity: ${Math.min(h * 20, 1) * o};${s}: ${h * w}px;padding-${l[0]}: ${h * y}px;padding-${l[1]}: ${h * f}px;margin-${l[0]}: ${h * d}px;margin-${l[1]}: ${h * c}px;border-${l[0]}-width: ${h * p}px;border-${l[1]}-width: ${h * k}px;min-${s}: 0` };
}
function Se(e, t) {
  for (const r in t) e[r] = t[r];
  return e;
}
function Mr({ fallback: e, ...t }) {
  const r = /* @__PURE__ */ new Map(), a = /* @__PURE__ */ new Map();
  function i(o, s, w) {
    const { delay: l = 0, duration: u = (A) => Math.sqrt(A) * 30, easing: y = Je } = Se(Se({}, t), w), f = o.getBoundingClientRect(), d = s.getBoundingClientRect(), c = f.left - d.left, p = f.top - d.top, k = f.width / d.width, h = f.height / d.height, E = Math.sqrt(c * c + p * p), g = getComputedStyle(s), v = g.transform === "none" ? "" : g.transform, _ = +g.opacity;
    return { delay: l, duration: typeof u == "function" ? u(E) : u, easing: y, css: (A, b) => `
			   opacity: ${A * _};
			   transform-origin: top left;
			   transform: ${v} translate(${b * c}px,${b * p}px) scale(${A + (1 - A) * k}, ${A + (1 - A) * h});
		   ` };
  }
  function n(o, s, w) {
    return (l, u) => (o.set(u.key, l), () => {
      if (s.has(u.key)) {
        const y = s.get(u.key);
        return s.delete(u.key), i(y, l, u);
      }
      return o.delete(u.key), e && e(l, u, w);
    });
  }
  return [n(a, r, false), n(r, a, true)];
}
var sr = F('<div><div class="loading svelte-1yqkxw6"><div class="loading-1 svelte-1yqkxw6"></div> <div class="loading-2 svelte-1yqkxw6"></div> <div class="loading-3 svelte-1yqkxw6"></div></div></div>');
function lr(e, t) {
  let r = C(t, "background", 3, false), a = C(t, "color", 3, "var(--col-text)"), i = C(t, "global", 3, false), n = C(t, "offset", 3, 0);
  var o = sr();
  let s;
  var w = N(o), l = N(w);
  let u;
  var y = Z(l, 2);
  let f;
  var d = Z(y, 2);
  let c;
  I(w), I(o), he((p) => {
    s = Ye(o, 1, "container svelte-1yqkxw6", null, s, p), x(w, `margin-top: ${n() ?? ""}px;`), u = x(l, "", u, { background: a() }), f = x(y, "", f, { background: a() }), c = x(d, "", c, { background: a() });
  }, [() => ({ global: i(), local: !i(), background: r() })]), Xe(3, o, () => Ze, () => ({ duration: 100 })), B(e, o);
}
var fr = F('<div class="load svelte-1m0mzre"><!></div>'), ur = F('<div class="font-label"><!></div>'), cr = F("<button><!></button>");
function Or(e, t) {
  Be(t, true);
  let r = C(t, "type", 3, "button"), a = C(t, "role", 3, "button"), i = C(t, "ref", 15), n = C(t, "level", 3, 2), o = C(t, "isDisabled", 3, false), s = C(t, "isLoading", 3, false), w = C(t, "destructive", 3, false), l = C(t, "invisible", 3, false), u = C(t, "invisibleOutline", 3, false), y = Vt(t, ["$$slots", "$$events", "$$legacy", "type", "role", "ref", "id", "ariaLabel", "ariaControls", "ariaCurrent", "level", "width", "isDisabled", "isLoading", "destructive", "invisible", "invisibleOutline", "popovertarget", "popovertargetaction", "onclick", "onLeft", "onRight", "onUp", "onDown", "children"]), f = oe(() => {
    if (l()) return "invisible";
    if (w()) return "destructive";
    switch (n()) {
      case 2:
        return "l2";
      case 3:
        return "l3";
      default:
        return "l1";
    }
  }), d = V(!s()), c = oe(() => o() || s());
  Fe(() => {
    s() ? setTimeout(() => {
      R(d, false);
    }, 120) : setTimeout(() => {
      R(d, true);
    }, 120);
  });
  function p() {
    if (w()) return "var(--btn-text)";
    switch (n()) {
      case 2:
        return "hsl(var(--action))";
      case 3:
        return "hsl(var(--action))";
      default:
        return "var(--btn-text)";
    }
  }
  function k(_) {
    var _a, _b, _c, _d, _e2;
    switch (_.code) {
      case "Enter":
        (_a = t.onclick) == null ? void 0 : _a.call(t, _);
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
  var h = cr();
  tr(h, (_) => ({ role: a(), type: r(), id: t.id, "aria-label": t.ariaLabel, "aria-controls": t.ariaControls, "aria-current": t.ariaCurrent, class: L(f), "data-isloading": s(), onclick: t.onclick, onkeydown: k, disabled: L(c), "aria-disabled": L(c), popovertarget: t.popovertarget, popovertargetaction: t.popovertargetaction, ...y, [G]: _, [P]: { width: t.width } }), [() => ({ invisibleOutline: u() })], "svelte-1m0mzre");
  var E = N(h);
  {
    var g = (_) => {
      var A = fr(), b = N(A);
      const m = oe(p);
      lr(b, { background: false, get color() {
        return L(m);
      } }), I(A), B(_, A);
    }, v = (_, A) => {
      {
        var b = (m) => {
          var T = ur(), H = N(T);
          ze(H, () => t.children), I(T), Xe(1, T, () => Ze), B(m, T);
        };
        $(_, (m) => {
          L(d) && m(b);
        }, A);
      }
    };
    $(E, (_) => {
      s() ? _(g) : _(v, false);
    });
  }
  I(h), Pe(h, (_) => i(_), () => i()), B(e, h), De();
}
const vr = qt(void 0), $e = "/dashboard/api";
async function qr(e) {
  let t = await fetch(`${$e}${e}`, { method: "GET" });
  return et(t);
}
async function Br(e, t) {
  let r = await fetch(`${$e}${e}`, { method: "POST", body: t });
  return et(r);
}
function et(e) {
  return e.status === 401 && vr.set(void 0), e;
}
var dr = Yt(`<svg fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M2.036 12.322a1.012 1.012 0 010-.639C3.423 7.51 7.36 4.5 12 4.5c4.638 0 8.573 3.007 9.963
            7.178.07.207.07.431 0 .639C20.577 16.49 16.64 19.5 12 19.5c-4.638 0-8.573-3.007-9.963-7.178z"></path><path stroke-linecap="round" stroke-linejoin="round" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"></path></svg>`);
function Fr(e, t) {
  let r = C(t, "color", 8, "var(--col-ok)"), a = C(t, "opacity", 8, 0.9), i = C(t, "width", 8, "1.5rem");
  var n = dr();
  q(n, "stroke-width", 2), he(() => {
    q(n, "width", i()), q(n, "color", r()), q(n, "opacity", a());
  }), B(e, n);
}
const _r = `-- comments will be ignored but only a single query is allowed
-- press CTRL + Enter to execute
SELECT 1`, hr = { id: "SELECT 1", query: _r }, Dr = "--!auto-query";
let Ur = ce([hr]);
const Pr = (e) => {
  let t = "";
  const r = e || 8;
  for (let a = 0; a < r; a += 1) {
    let i = 60;
    for (; i > 57 && i < 65 || i > 90 && i < 97; ) i = Math.floor(Math.random() * 74) + 48;
    t = t.concat(String.fromCharCode(i));
  }
  return t;
};
function gr(e, t, r, a) {
  t(), window.addEventListener("mousemove", r), window.addEventListener("mouseup", a, { once: true });
}
function br(e, t, r, a) {
  t(), window.addEventListener("mousemove", r), window.addEventListener("mouseup", a, { once: true });
}
var wr = F('<div class="relative"><div role="none" class="right svelte-1u5iq19"></div></div>'), yr = F('<div class="relative"><div role="none" class="bottom svelte-1u5iq19"></div></div>'), pr = F('<div><div class="children svelte-1u5iq19"><div class="inner svelte-1u5iq19"><!></div> <!></div> <!></div>');
function xr(e, t) {
  Be(t, true);
  let r = C(t, "minWidthPx", 3, 50), a = C(t, "minHeightPx", 3, 50), i, n = V(void 0), o = V(void 0), s = V(ce(z(() => t.initialWidthPx))), w = V(ce(z(() => t.initialHeightPx)));
  Fe(() => {
    l();
  });
  function l() {
    var _a;
    if (i) {
      let b = i.getBoundingClientRect();
      t.resizeRight && (R(o, b.left, true), R(s, b.width, true)), t.resizeBottom && (R(n, b.top, true), R(w, b.height, true)), (_a = t.onResizeBottom) == null ? void 0 : _a.call(t, b.bottom);
    }
  }
  function u() {
    window.removeEventListener("mousemove", y), l();
  }
  function y(b) {
    let m = window.scrollX + b.x - (L(o) || 0);
    m < r() ? R(s, r()) : R(s, m);
  }
  function f() {
    window.removeEventListener("mousemove", d), l();
  }
  function d(b) {
    console.log(window.screenY, b.y);
    let m = window.screenY + b.clientY - (L(n) || 0);
    if (m < a() ? R(w, a()) : R(w, m), i && t.onResizeBottom) {
      let T = i.getBoundingClientRect();
      t.onResizeBottom(T.bottom);
    }
  }
  var c = pr();
  let p;
  var k = N(c), h = N(k), E = N(h);
  ze(E, () => t.children), I(h);
  var g = Z(h, 2);
  {
    var v = (b) => {
      var m = wr(), T = N(m);
      T.__mousedown = [gr, l, y, u], I(m), B(b, m);
    };
    $(g, (b) => {
      t.resizeRight && b(v);
    });
  }
  I(k);
  var _ = Z(k, 2);
  {
    var A = (b) => {
      var m = yr(), T = N(m);
      T.__mousedown = [br, l, d, f], I(m), B(b, m);
    };
    $(_, (b) => {
      t.resizeBottom && b(A);
    });
  }
  I(c), Pe(c, (b) => i = b, () => i), he(() => p = x(c, "", p, { width: L(s) && `${L(s)}px`, height: L(w) && `${L(w)}px`, border: t.border, padding: t.padding })), B(e, c), De();
}
Ue(["mousedown"]);
export {
  $e as A,
  Or as B,
  hr as D,
  Fr as I,
  Ur as Q,
  xr as R,
  q as a,
  x as b,
  Mr as c,
  Rr as d,
  Ir as e,
  vr as f,
  Cr as g,
  et as h,
  Tr as i,
  Ye as j,
  Xt as k,
  qr as l,
  Pr as m,
  Dr as n,
  $t as o,
  Sr as p,
  Nr as q,
  Lr as r,
  ze as s,
  Xe as t,
  Br as u,
  _r as v
};
