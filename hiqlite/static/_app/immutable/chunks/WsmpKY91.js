import { B as be, x as S, C as sr, g as R, X as fr, D as lr, H as ur, F as me, G, I as H, y as W, ae as Ie, aH as cr, al as ke, m as F, v as ee, aJ as U, w as J, A as dr, ac as Ae, N as vr, aF as we, aK as Me, aL as hr, aM as gr, aN as _r, n as Oe, q as Fe, aO as se, aB as ve, aE as br, o as re, at as Be, aP as Ue, L as pe, aQ as wr, E as De, _ as pr, aR as yr, ax as Er, J as ne, b as Z, aS as mr, aT as kr, aU as Ar, as as Pe, aV as Tr, M as V, aW as Cr, aX as ze, aY as Sr, aZ as Lr, Q as Rr, a_ as ye, a$ as He, b0 as Nr, b1 as Ir, b2 as Mr, b3 as Or, b4 as Fr, b5 as Br, b6 as Ur, b7 as Dr, b8 as Pr, K as qe, a9 as zr, k as M, s as te, l as O, t as Ee, p as Ye, a8 as Q, a as xe, T as I, j as Ve, aa as fe, i as Hr, b9 as qr, Y as he } from "./TbIIo73h.js";
import { b as Yr, i as xr, d as Vr, e as We, n as Wr, g as Qr, f as q, a as D, c as Kr, j as jr } from "./U6xNcRH3.js";
import { B as Gr, p as L, i as ae, b as Qe, r as Xr } from "./Cn5KZez5.js";
import "./BUpFzQn_.js";
function Nt(e, r) {
  return r;
}
function Jr(e, r, t) {
  for (var a = [], i = r.length, n, o = r.length, f = 0; f < i; f++) {
    let w = r[f];
    Fe(w, () => {
      if (n) {
        if (n.pending.delete(w), n.done.add(w), n.pending.size === 0) {
          var v = e.outrogroups;
          ge(we(n.done)), v.delete(n), v.size === 0 && (e.outrogroups = null);
        }
      } else o -= 1;
    }, false);
  }
  if (o === 0) {
    var u = a.length === 0 && t !== null;
    if (u) {
      var c = t, s = c.parentNode;
      br(s), s.append(c), e.items.clear();
    }
    ge(r, !u);
  } else n = { pending: new Set(r), done: /* @__PURE__ */ new Set() }, (e.outrogroups ?? (e.outrogroups = /* @__PURE__ */ new Set())).add(n);
}
function ge(e, r = true) {
  for (var t = 0; t < e.length; t++) re(e[t], r);
}
var Te;
function It(e, r, t, a, i, n = null) {
  var o = e, f = /* @__PURE__ */ new Map(), u = (r & Ue) !== 0;
  if (u) {
    var c = e;
    o = S ? G(Be(c)) : c.appendChild(ee());
  }
  S && sr();
  var s = null, w = fr(() => {
    var d = t();
    return Me(d) ? d : d == null ? [] : we(d);
  }), v, b = true;
  function y() {
    l.fallback = s, Zr(l, v, o, r, a), s !== null && (v.length === 0 ? (s.f & U) === 0 ? Oe(s) : (s.f ^= U, K(s, null, o)) : Fe(s, () => {
      s = null;
    }));
  }
  var E = be(() => {
    v = R(w);
    var d = v.length;
    let k = false;
    if (S) {
      var C = lr(o) === ur;
      C !== (d === 0) && (o = me(), G(o), H(false), k = true);
    }
    for (var m = /* @__PURE__ */ new Set(), T = F, h = dr(), _ = 0; _ < d; _ += 1) {
      S && W.nodeType === Ie && W.data === cr && (o = W, k = true, H(false));
      var A = v[_], p = a(A, _), g = b ? null : f.get(p);
      g ? (g.v && ke(g.v, A), g.i && ke(g.i, _), h && T.skipped_effects.delete(g.e)) : (g = $r(f, b ? o : Te ?? (Te = ee()), A, p, _, i, r, t), b || (g.e.f |= U), f.set(p, g)), m.add(p);
    }
    if (d === 0 && n && !s && (b ? s = J(() => n(o)) : (s = J(() => n(Te ?? (Te = ee()))), s.f |= U)), S && d > 0 && G(me()), !b) if (h) {
      for (const [N, oe] of f) m.has(N) || T.skipped_effects.add(oe.e);
      T.oncommit(y), T.ondiscard(() => {
      });
    } else y();
    k && H(true), R(w);
  }), l = { effect: E, items: f, outrogroups: null, fallback: s };
  b = false, S && (o = W);
}
function Zr(e, r, t, a, i) {
  var _a, _b, _c, _d, _e2, _f, _g, _h, _i;
  var n = (a & wr) !== 0, o = r.length, f = e.items, u = e.effect.first, c, s = null, w, v = [], b = [], y, E, l, d;
  if (n) for (d = 0; d < o; d += 1) y = r[d], E = i(y, d), l = f.get(E).e, (l.f & U) === 0 && ((_b = (_a = l.nodes) == null ? void 0 : _a.a) == null ? void 0 : _b.measure(), (w ?? (w = /* @__PURE__ */ new Set())).add(l));
  for (d = 0; d < o; d += 1) {
    if (y = r[d], E = i(y, d), l = f.get(E).e, e.outrogroups !== null) for (const g of e.outrogroups) g.pending.delete(l), g.done.delete(l);
    if ((l.f & U) !== 0) if (l.f ^= U, l === u) K(l, null, t);
    else {
      var k = s ? s.next : u;
      l === e.effect.last && (e.effect.last = l.prev), l.prev && (l.prev.next = l.next), l.next && (l.next.prev = l.prev), P(e, s, l), P(e, l, k), K(l, k, t), s = l, v = [], b = [], u = s.next;
      continue;
    }
    if ((l.f & se) !== 0 && (Oe(l), n && ((_d = (_c = l.nodes) == null ? void 0 : _c.a) == null ? void 0 : _d.unfix(), (w ?? (w = /* @__PURE__ */ new Set())).delete(l))), l !== u) {
      if (c !== void 0 && c.has(l)) {
        if (v.length < b.length) {
          var C = b[0], m;
          s = C.prev;
          var T = v[0], h = v[v.length - 1];
          for (m = 0; m < v.length; m += 1) K(v[m], C, t);
          for (m = 0; m < b.length; m += 1) c.delete(b[m]);
          P(e, T.prev, h.next), P(e, s, T), P(e, h, C), u = C, s = h, d -= 1, v = [], b = [];
        } else c.delete(l), K(l, u, t), P(e, l.prev, l.next), P(e, l, s === null ? e.effect.first : s.next), P(e, s, l), s = l;
        continue;
      }
      for (v = [], b = []; u !== null && u !== l; ) (c ?? (c = /* @__PURE__ */ new Set())).add(u), b.push(u), u = u.next;
      if (u === null) continue;
    }
    (l.f & U) === 0 && v.push(l), s = l, u = l.next;
  }
  if (e.outrogroups !== null) {
    for (const g of e.outrogroups) g.pending.size === 0 && (ge(we(g.done)), (_e2 = e.outrogroups) == null ? void 0 : _e2.delete(g));
    e.outrogroups.size === 0 && (e.outrogroups = null);
  }
  if (u !== null || c !== void 0) {
    var _ = [];
    if (c !== void 0) for (l of c) (l.f & se) === 0 && _.push(l);
    for (; u !== null; ) (u.f & se) === 0 && u !== e.fallback && _.push(u), u = u.next;
    var A = _.length;
    if (A > 0) {
      var p = (a & Ue) !== 0 && o === 0 ? t : null;
      if (n) {
        for (d = 0; d < A; d += 1) (_g = (_f = _[d].nodes) == null ? void 0 : _f.a) == null ? void 0 : _g.measure();
        for (d = 0; d < A; d += 1) (_i = (_h = _[d].nodes) == null ? void 0 : _h.a) == null ? void 0 : _i.fix();
      }
      Jr(e, _, p);
    }
  }
  n && pe(() => {
    var _a2, _b2;
    if (w !== void 0) for (l of w) (_b2 = (_a2 = l.nodes) == null ? void 0 : _a2.a) == null ? void 0 : _b2.apply();
  });
}
function $r(e, r, t, a, i, n, o, f) {
  var u = (o & hr) !== 0 ? (o & gr) === 0 ? vr(t, false, false) : Ae(t) : null, c = (o & _r) !== 0 ? Ae(i) : null;
  return { v: u, i: c, e: J(() => (n(r, u ?? t, c ?? i, f), () => {
    e.delete(a);
  })) };
}
function K(e, r, t) {
  if (e.nodes) for (var a = e.nodes.start, i = e.nodes.end, n = r && (r.f & U) === 0 ? r.nodes.start : t; a !== null; ) {
    var o = ve(a);
    if (n.before(a), a === i) return;
    a = o;
  }
}
function P(e, r, t) {
  r === null ? e.effect.first = t : r.next = t, t === null ? e.effect.last = r : t.prev = r;
}
function Ke(e, r, ...t) {
  var a = new Gr(e);
  be(() => {
    const i = r() ?? null;
    a.ensure(i, i && ((n) => i(n, ...t)));
  }, De);
}
const et = () => performance.now(), B = { tick: (e) => requestAnimationFrame(e), now: () => et(), tasks: /* @__PURE__ */ new Set() };
function je() {
  const e = B.now();
  B.tasks.forEach((r) => {
    r.c(e) || (B.tasks.delete(r), r.f());
  }), B.tasks.size !== 0 && B.tick(je);
}
function rt(e) {
  let r;
  return B.tasks.size === 0 && B.tick(je), { promise: new Promise((t) => {
    B.tasks.add(r = { c: e, f: t });
  }), abort() {
    B.tasks.delete(r);
  } };
}
function $(e, r) {
  Pe(() => {
    e.dispatchEvent(new CustomEvent(r));
  });
}
function tt(e) {
  if (e === "float") return "cssFloat";
  if (e === "offset") return "cssOffset";
  if (e.startsWith("--")) return e;
  const r = e.split("-");
  return r.length === 1 ? r[0] : r[0] + r.slice(1).map((t) => t[0].toUpperCase() + t.slice(1)).join("");
}
function Ce(e) {
  const r = {}, t = e.split(";");
  for (const a of t) {
    const [i, n] = a.split(":");
    if (!i || n === void 0) break;
    const o = tt(i.trim());
    r[o] = n.trim();
  }
  return r;
}
const at = (e) => e;
function Ge(e, r, t, a) {
  var _a;
  var i = (e & kr) !== 0, n = (e & Ar) !== 0, o = i && n, f = (e & mr) !== 0, u = o ? "both" : i ? "in" : "out", c, s = r.inert, w = r.style.overflow, v, b;
  function y() {
    return Pe(() => c ?? (c = t()(r, (a == null ? void 0 : a()) ?? {}, { direction: u })));
  }
  var E = { is_global: f, in() {
    var _a2;
    if (r.inert = s, !i) {
      b == null ? void 0 : b.abort(), (_a2 = b == null ? void 0 : b.reset) == null ? void 0 : _a2.call(b);
      return;
    }
    n || (v == null ? void 0 : v.abort()), $(r, "introstart"), v = _e(r, y(), b, 1, () => {
      $(r, "introend"), v == null ? void 0 : v.abort(), v = c = void 0, r.style.overflow = w;
    });
  }, out(C) {
    if (!n) {
      C == null ? void 0 : C(), c = void 0;
      return;
    }
    r.inert = true, $(r, "outrostart"), b = _e(r, y(), v, 0, () => {
      $(r, "outroend"), C == null ? void 0 : C();
    });
  }, stop: () => {
    v == null ? void 0 : v.abort(), b == null ? void 0 : b.abort();
  } }, l = pr;
  if (((_a = l.nodes).t ?? (_a.t = [])).push(E), i && Yr) {
    var d = f;
    if (!d) {
      for (var k = l.parent; k && (k.f & De) !== 0; ) for (; (k = k.parent) && (k.f & yr) === 0; ) ;
      d = !k || (k.f & Er) !== 0;
    }
    d && ne(() => {
      Z(() => E.in());
    });
  }
}
function _e(e, r, t, a, i) {
  var n = a === 1;
  if (Tr(r)) {
    var o, f = false;
    return pe(() => {
      if (!f) {
        var l = r({ direction: n ? "in" : "out" });
        o = _e(e, l, t, a, i);
      }
    }), { abort: () => {
      f = true, o == null ? void 0 : o.abort();
    }, deactivate: () => o.deactivate(), reset: () => o.reset(), t: () => o.t() };
  }
  if (t == null ? void 0 : t.deactivate(), !(r == null ? void 0 : r.duration)) return i(), { abort: V, deactivate: V, reset: V, t: () => a };
  const { delay: u = 0, css: c, tick: s, easing: w = at } = r;
  var v = [];
  if (n && t === void 0 && (s && s(0, 1), c)) {
    var b = Ce(c(0, 1));
    v.push(b, b);
  }
  var y = () => 1 - a, E = e.animate(v, { duration: u, fill: "forwards" });
  return E.onfinish = () => {
    E.cancel();
    var l = (t == null ? void 0 : t.t()) ?? 1 - a;
    t == null ? void 0 : t.abort();
    var d = a - l, k = r.duration * Math.abs(d), C = [];
    if (k > 0) {
      var m = false;
      if (c) for (var T = Math.ceil(k / 16.666666666666668), h = 0; h <= T; h += 1) {
        var _ = l + d * w(h / T), A = Ce(c(_, 1 - _));
        C.push(A), m || (m = A.overflow === "hidden");
      }
      m && (e.style.overflow = "hidden"), y = () => {
        var p = E.currentTime;
        return l + d * w(p / k);
      }, s && rt(() => {
        if (E.playState !== "running") return false;
        var p = y();
        return s(p, 1 - p), true;
      });
    }
    E = e.animate(C, { duration: k, fill: "forwards" }), E.onfinish = () => {
      y = () => a, s == null ? void 0 : s(a, 1 - a), i();
    };
  }, { abort: () => {
    E && (E.cancel(), E.effect = null, E.onfinish = V);
  }, deactivate: () => {
    i = V;
  }, reset: () => {
    a === 0 && (s == null ? void 0 : s(1, 0));
  }, t: () => y() };
}
function Mt(e, r) {
  let t = null, a = S;
  var i;
  if (S) {
    t = W;
    for (var n = Be(document.head); n !== null && (n.nodeType !== Ie || n.data !== e); ) n = ve(n);
    if (n === null) H(false);
    else {
      var o = ve(n);
      n.remove(), G(o);
    }
  }
  S || (i = document.head.appendChild(ee()));
  try {
    be(() => r(i), Cr);
  } finally {
    a && (H(true), G(t));
  }
}
function it(e, r) {
  var t = void 0, a;
  ze(() => {
    t !== (t = r()) && (a && (re(a), a = null), t && (a = J(() => {
      ne(() => t(e));
    })));
  });
}
function Xe(e) {
  var r, t, a = "";
  if (typeof e == "string" || typeof e == "number") a += e;
  else if (typeof e == "object") if (Array.isArray(e)) {
    var i = e.length;
    for (r = 0; r < i; r++) e[r] && (t = Xe(e[r])) && (a && (a += " "), a += t);
  } else for (t in e) e[t] && (a && (a += " "), a += t);
  return a;
}
function nt() {
  for (var e, r, t = 0, a = "", i = arguments.length; t < i; t++) (e = arguments[t]) && (r = Xe(e)) && (a && (a += " "), a += r);
  return a;
}
function ot(e) {
  return typeof e == "object" ? nt(e) : e ?? "";
}
const Se = [...` 	
\r\f\xA0\v\uFEFF`];
function st(e, r, t) {
  var a = e == null ? "" : "" + e;
  if (r && (a = a ? a + " " + r : r), t) {
    for (var i in t) if (t[i]) a = a ? a + " " + i : i;
    else if (a.length) for (var n = i.length, o = 0; (o = a.indexOf(i, o)) >= 0; ) {
      var f = o + n;
      (o === 0 || Se.includes(a[o - 1])) && (f === a.length || Se.includes(a[f])) ? a = (o === 0 ? "" : a.substring(0, o)) + a.substring(f + 1) : o = f;
    }
  }
  return a === "" ? null : a;
}
function Le(e, r = false) {
  var t = r ? " !important;" : ";", a = "";
  for (var i in e) {
    var n = e[i];
    n != null && n !== "" && (a += " " + i + ": " + n + t);
  }
  return a;
}
function le(e) {
  return e[0] !== "-" || e[1] !== "-" ? e.toLowerCase() : e;
}
function ft(e, r) {
  if (r) {
    var t = "", a, i;
    if (Array.isArray(r) ? (a = r[0], i = r[1]) : a = r, e) {
      e = String(e).replaceAll(/\s*\/\*.*?\*\/\s*/g, "").trim();
      var n = false, o = 0, f = false, u = [];
      a && u.push(...Object.keys(a).map(le)), i && u.push(...Object.keys(i).map(le));
      var c = 0, s = -1;
      const E = e.length;
      for (var w = 0; w < E; w++) {
        var v = e[w];
        if (f ? v === "/" && e[w - 1] === "*" && (f = false) : n ? n === v && (n = false) : v === "/" && e[w + 1] === "*" ? f = true : v === '"' || v === "'" ? n = v : v === "(" ? o++ : v === ")" && o--, !f && n === false && o === 0) {
          if (v === ":" && s === -1) s = w;
          else if (v === ";" || w === E - 1) {
            if (s !== -1) {
              var b = le(e.substring(c, s).trim());
              if (!u.includes(b)) {
                v !== ";" && w++;
                var y = e.substring(c, w).trim();
                t += " " + y + ";";
              }
            }
            c = w + 1, s = -1;
          }
        }
      }
    }
    return a && (t += Le(a)), i && (t += Le(i, true)), t = t.trim(), t === "" ? null : t;
  }
  return e == null ? null : String(e);
}
function Je(e, r, t, a, i, n) {
  var o = e.__className;
  if (S || o !== t || o === void 0) {
    var f = st(t, a, n);
    (!S || f !== e.getAttribute("class")) && (f == null ? e.removeAttribute("class") : r ? e.className = f : e.setAttribute("class", f)), e.__className = t;
  } else if (n && i !== n) for (var u in n) {
    var c = !!n[u];
    (i == null || c !== !!i[u]) && e.classList.toggle(u, c);
  }
  return n;
}
function ue(e, r = {}, t, a) {
  for (var i in t) {
    var n = t[i];
    r[i] !== n && (t[i] == null ? e.style.removeProperty(i) : e.style.setProperty(i, n, a));
  }
}
function x(e, r, t, a) {
  var i = e.__style;
  if (S || i !== r) {
    var n = ft(r, a);
    (!S || n !== e.getAttribute("style")) && (n == null ? e.removeAttribute("style") : e.style.cssText = n), e.__style = r;
  } else a && (Array.isArray(a) ? (ue(e, t == null ? void 0 : t[0], a[0]), ue(e, t == null ? void 0 : t[1], a[1], "important")) : ue(e, t, a));
  return a;
}
function ie(e, r, t = false) {
  if (e.multiple) {
    if (r == null) return;
    if (!Me(r)) return Sr();
    for (var a of e.options) a.selected = r.includes(X(a));
    return;
  }
  for (a of e.options) {
    var i = X(a);
    if (Lr(i, r)) {
      a.selected = true;
      return;
    }
  }
  (!t || r !== void 0) && (e.selectedIndex = -1);
}
function Ze(e) {
  var r = new MutationObserver(() => {
    ie(e, e.__value);
  });
  r.observe(e, { childList: true, subtree: true, attributes: true, attributeFilter: ["value"] }), Rr(() => {
    r.disconnect();
  });
}
function Ot(e, r, t = r) {
  var a = /* @__PURE__ */ new WeakSet(), i = true;
  ye(e, "change", (n) => {
    var o = n ? "[selected]" : ":checked", f;
    if (e.multiple) f = [].map.call(e.querySelectorAll(o), X);
    else {
      var u = e.querySelector(o) ?? e.querySelector("option:not([disabled])");
      f = u && X(u);
    }
    t(f), F !== null && a.add(F);
  }), ne(() => {
    var n = r();
    if (e === document.activeElement) {
      var o = He ?? F;
      if (a.has(o)) return;
    }
    if (ie(e, n, i), i && n === void 0) {
      var f = e.querySelector(":checked");
      f !== null && (n = X(f), t(n));
    }
    e.__value = n, i = false;
  }), Ze(e);
}
function X(e) {
  return "__value" in e ? e.__value : e.value;
}
const j = Symbol("class"), Y = Symbol("style"), $e = Symbol("is custom element"), er = Symbol("is html");
function lt(e) {
  if (S) {
    var r = false, t = () => {
      if (!r) {
        if (r = true, e.hasAttribute("value")) {
          var a = e.value;
          z(e, "value", null), e.value = a;
        }
        if (e.hasAttribute("checked")) {
          var i = e.checked;
          z(e, "checked", null), e.checked = i;
        }
      }
    };
    e.__on_r = t, pe(t), Dr();
  }
}
function ut(e, r) {
  r ? e.hasAttribute("selected") || e.setAttribute("selected", "") : e.removeAttribute("selected");
}
function z(e, r, t, a) {
  var i = rr(e);
  S && (i[r] = e.getAttribute(r), r === "src" || r === "srcset" || r === "href" && e.nodeName === "LINK") || i[r] !== (i[r] = t) && (r === "loading" && (e[Pr] = t), t == null ? e.removeAttribute(r) : typeof t != "string" && tr(e).includes(r) ? e[r] = t : e.setAttribute(r, t));
}
function ct(e, r, t, a, i = false, n = false) {
  if (S && i && e.tagName === "INPUT") {
    var o = e, f = o.type === "checkbox" ? "defaultChecked" : "defaultValue";
    f in t || lt(o);
  }
  var u = rr(e), c = u[$e], s = !u[er];
  let w = S && c;
  w && H(false);
  var v = r || {}, b = e.tagName === "OPTION";
  for (var y in r) y in t || (t[y] = null);
  t.class ? t.class = ot(t.class) : t.class = null, t[Y] && (t.style ?? (t.style = null));
  var E = tr(e);
  for (const h in t) {
    let _ = t[h];
    if (b && h === "value" && _ == null) {
      e.value = e.__value = "", v[h] = _;
      continue;
    }
    if (h === "class") {
      var l = e.namespaceURI === "http://www.w3.org/1999/xhtml";
      Je(e, l, _, a, r == null ? void 0 : r[j], t[j]), v[h] = _, v[j] = t[j];
      continue;
    }
    if (h === "style") {
      x(e, _, r == null ? void 0 : r[Y], t[Y]), v[h] = _, v[Y] = t[Y];
      continue;
    }
    var d = v[h];
    if (!(_ === d && !(_ === void 0 && e.hasAttribute(h)))) {
      v[h] = _;
      var k = h[0] + h[1];
      if (k !== "$$") if (k === "on") {
        const A = {}, p = "$$" + h;
        let g = h.slice(2);
        var C = Qr(g);
        if (xr(g) && (g = g.slice(0, -7), A.capture = true), !C && d) {
          if (_ != null) continue;
          e.removeEventListener(g, v[p], A), v[p] = null;
        }
        if (_ != null) if (C) e[`__${g}`] = _, We([g]);
        else {
          let N = function(oe) {
            v[h].call(this, oe);
          };
          v[p] = Vr(g, e, N, A);
        }
        else C && (e[`__${g}`] = void 0);
      } else if (h === "style") z(e, h, _);
      else if (h === "autofocus") Br(e, !!_);
      else if (!c && (h === "__value" || h === "value" && _ != null)) e.value = e.__value = _;
      else if (h === "selected" && b) ut(e, _);
      else {
        var m = h;
        s || (m = Wr(m));
        var T = m === "defaultValue" || m === "defaultChecked";
        if (_ == null && !c && !T) if (u[h] = null, m === "value" || m === "checked") {
          let A = e;
          const p = r === void 0;
          if (m === "value") {
            let g = A.defaultValue;
            A.removeAttribute(m), A.defaultValue = g, A.value = A.__value = p ? g : null;
          } else {
            let g = A.defaultChecked;
            A.removeAttribute(m), A.defaultChecked = g, A.checked = p ? g : false;
          }
        } else e.removeAttribute(h);
        else T || E.includes(m) && (c || typeof _ != "string") ? (e[m] = _, m in u && (u[m] = Ur)) : typeof _ != "function" && z(e, m, _);
      }
    }
  }
  return w && H(true), v;
}
function dt(e, r, t = [], a = [], i = [], n, o = false, f = false) {
  Nr(i, t, a, (u) => {
    var c = void 0, s = {}, w = e.nodeName === "SELECT", v = false;
    if (ze(() => {
      var y = r(...u.map(R)), E = ct(e, c, y, n, o, f);
      v && w && "value" in y && ie(e, y.value);
      for (let d of Object.getOwnPropertySymbols(s)) y[d] || re(s[d]);
      for (let d of Object.getOwnPropertySymbols(y)) {
        var l = y[d];
        d.description === Or && (!c || l !== c[d]) && (s[d] && re(s[d]), s[d] = J(() => it(e, () => l))), E[d] = l;
      }
      c = E;
    }), w) {
      var b = e;
      ne(() => {
        ie(b, c.value, true), Ze(b);
      });
    }
    v = true;
  });
}
function rr(e) {
  return e.__attributes ?? (e.__attributes = { [$e]: e.nodeName.includes("-"), [er]: e.namespaceURI === Ir });
}
var Re = /* @__PURE__ */ new Map();
function tr(e) {
  var r = e.getAttribute("is") || e.nodeName, t = Re.get(r);
  if (t) return t;
  Re.set(r, t = []);
  for (var a, i = e, n = Element.prototype; n !== i; ) {
    a = Fr(i);
    for (var o in a) a[o].set && t.push(o);
    i = Mr(i);
  }
  return t;
}
function Ft(e, r, t = r) {
  var a = /* @__PURE__ */ new WeakSet();
  ye(e, "input", async (i) => {
    var n = i ? e.defaultValue : e.value;
    if (n = ce(e) ? de(n) : n, t(n), F !== null && a.add(F), await zr(), n !== (n = r())) {
      var o = e.selectionStart, f = e.selectionEnd, u = e.value.length;
      if (e.value = n ?? "", f !== null) {
        var c = e.value.length;
        o === f && f === u && c > u ? (e.selectionStart = c, e.selectionEnd = c) : (e.selectionStart = o, e.selectionEnd = Math.min(f, c));
      }
    }
  }), (S && e.defaultValue !== e.value || Z(r) == null && e.value) && (t(ce(e) ? de(e.value) : e.value), F !== null && a.add(F)), qe(() => {
    var i = r();
    if (e === document.activeElement) {
      var n = He ?? F;
      if (a.has(n)) return;
    }
    ce(e) && i === de(e.value) || e.type === "date" && !i && !e.value || i !== e.value && (e.value = i ?? "");
  });
}
function Bt(e, r, t = r) {
  ye(e, "change", (a) => {
    var i = a ? e.defaultChecked : e.checked;
    t(i);
  }), (S && e.defaultChecked !== e.checked || Z(r) == null) && t(e.checked), qe(() => {
    var a = r();
    e.checked = !!a;
  });
}
function ce(e) {
  var r = e.type;
  return r === "number" || r === "range";
}
function de(e) {
  return e === "" ? null : +e;
}
const vt = (e) => e;
function ar(e) {
  const r = e - 1;
  return r * r * r + 1;
}
function ir(e, { delay: r = 0, duration: t = 400, easing: a = vt } = {}) {
  const i = +getComputedStyle(e).opacity;
  return { delay: r, duration: t, easing: a, css: (n) => `opacity: ${n * i}` };
}
function Ut(e, { delay: r = 0, duration: t = 400, easing: a = ar, axis: i = "y" } = {}) {
  const n = getComputedStyle(e), o = +n.opacity, f = i === "y" ? "height" : "width", u = parseFloat(n[f]), c = i === "y" ? ["top", "bottom"] : ["left", "right"], s = c.map((d) => `${d[0].toUpperCase()}${d.slice(1)}`), w = parseFloat(n[`padding${s[0]}`]), v = parseFloat(n[`padding${s[1]}`]), b = parseFloat(n[`margin${s[0]}`]), y = parseFloat(n[`margin${s[1]}`]), E = parseFloat(n[`border${s[0]}Width`]), l = parseFloat(n[`border${s[1]}Width`]);
  return { delay: r, duration: t, easing: a, css: (d) => `overflow: hidden;opacity: ${Math.min(d * 20, 1) * o};${f}: ${d * u}px;padding-${c[0]}: ${d * w}px;padding-${c[1]}: ${d * v}px;margin-${c[0]}: ${d * b}px;margin-${c[1]}: ${d * y}px;border-${c[0]}-width: ${d * E}px;border-${c[1]}-width: ${d * l}px;min-${f}: 0` };
}
function Ne(e, r) {
  for (const t in r) e[t] = r[t];
  return e;
}
function Dt({ fallback: e, ...r }) {
  const t = /* @__PURE__ */ new Map(), a = /* @__PURE__ */ new Map();
  function i(o, f, u) {
    const { delay: c = 0, duration: s = (h) => Math.sqrt(h) * 30, easing: w = ar } = Ne(Ne({}, r), u), v = o.getBoundingClientRect(), b = f.getBoundingClientRect(), y = v.left - b.left, E = v.top - b.top, l = v.width / b.width, d = v.height / b.height, k = Math.sqrt(y * y + E * E), C = getComputedStyle(f), m = C.transform === "none" ? "" : C.transform, T = +C.opacity;
    return { delay: c, duration: typeof s == "function" ? s(k) : s, easing: w, css: (h, _) => `
			   opacity: ${h * T};
			   transform-origin: top left;
			   transform: ${m} translate(${_ * y}px,${_ * E}px) scale(${h + (1 - h) * l}, ${h + (1 - h) * d});
		   ` };
  }
  function n(o, f, u) {
    return (c, s) => (o.set(s.key, c), () => {
      if (f.has(s.key)) {
        const w = f.get(s.key);
        return f.delete(s.key), i(w, c, s);
      }
      return o.delete(s.key), e && e(c, s, u);
    });
  }
  return [n(a, t, false), n(t, a, true)];
}
var ht = q('<div><div class="loading svelte-1a5pdw0"><div class="loading-1 svelte-1a5pdw0"></div> <div class="loading-2 svelte-1a5pdw0"></div> <div class="loading-3 svelte-1a5pdw0"></div></div></div>');
function gt(e, r) {
  let t = L(r, "background", 3, false), a = L(r, "color", 3, "var(--col-text)"), i = L(r, "global", 3, false), n = L(r, "offset", 3, 0);
  var o = ht();
  let f;
  var u = M(o), c = M(u);
  let s;
  var w = te(c, 2);
  let v;
  var b = te(w, 2);
  let y;
  O(u), O(o), Ee(() => {
    f = Je(o, 1, "container svelte-1a5pdw0", null, f, { global: i(), local: !i(), background: t() }), x(u, `margin-top: ${n() ?? ""}px;`), s = x(c, "", s, { background: a() }), v = x(w, "", v, { background: a() }), y = x(b, "", y, { background: a() });
  }), Ge(3, o, () => ir, () => ({ duration: 100 })), D(e, o);
}
var _t = q('<div class="load svelte-18sv61c"><!></div>'), bt = q('<div class="font-label"><!></div>'), wt = q("<button><!></button>");
function Pt(e, r) {
  Ye(r, true);
  let t = L(r, "type", 3, "button"), a = L(r, "role", 3, "button"), i = L(r, "ref", 15), n = L(r, "level", 3, 2), o = L(r, "isDisabled", 3, false), f = L(r, "isLoading", 3, false), u = L(r, "destructive", 3, false), c = L(r, "invisible", 3, false), s = L(r, "invisibleOutline", 3, false), w = Xr(r, ["$$slots", "$$events", "$$legacy", "type", "role", "ref", "id", "ariaLabel", "ariaControls", "ariaCurrent", "level", "width", "isDisabled", "isLoading", "destructive", "invisible", "invisibleOutline", "popovertarget", "popovertargetaction", "onclick", "onLeft", "onRight", "onUp", "onDown", "children"]), v = fe(() => {
    if (c()) return "invisible";
    if (u()) return "destructive";
    switch (n()) {
      case 2:
        return "l2";
      case 3:
        return "l3";
      default:
        return "l1";
    }
  }), b = Q(!f()), y = fe(() => o() || f());
  xe(() => {
    f() ? setTimeout(() => {
      I(b, false);
    }, 120) : setTimeout(() => {
      I(b, true);
    }, 120);
  });
  function E() {
    if (u()) return "var(--btn-text)";
    switch (n()) {
      case 2:
        return "hsl(var(--action))";
      case 3:
        return "hsl(var(--action))";
      default:
        return "var(--btn-text)";
    }
  }
  function l(T) {
    var _a, _b, _c, _d, _e2;
    switch (T.code) {
      case "Enter":
        (_a = r.onclick) == null ? void 0 : _a.call(r, T);
        break;
      case "ArrowLeft":
        (_b = r.onLeft) == null ? void 0 : _b.call(r);
        break;
      case "ArrowRight":
        (_c = r.onRight) == null ? void 0 : _c.call(r);
        break;
      case "ArrowUp":
        (_d = r.onUp) == null ? void 0 : _d.call(r);
        break;
      case "ArrowDown":
        (_e2 = r.onDown) == null ? void 0 : _e2.call(r);
        break;
    }
  }
  var d = wt();
  dt(d, () => ({ role: a(), type: t(), id: r.id, "aria-label": r.ariaLabel, "aria-controls": r.ariaControls, "aria-current": r.ariaCurrent, class: R(v), "data-isloading": f(), onclick: r.onclick, onkeydown: l, disabled: R(y), "aria-disabled": R(y), popovertarget: r.popovertarget, popovertargetaction: r.popovertargetaction, ...w, [j]: { invisibleOutline: s() }, [Y]: { width: r.width } }), void 0, void 0, void 0, "svelte-18sv61c");
  var k = M(d);
  {
    var C = (T) => {
      var h = _t(), _ = M(h);
      {
        let A = fe(E);
        gt(_, { background: false, get color() {
          return R(A);
        } });
      }
      O(h), D(T, h);
    }, m = (T) => {
      var h = Kr(), _ = Hr(h);
      {
        var A = (p) => {
          var g = bt(), N = M(g);
          Ke(N, () => r.children), O(g), Ge(1, g, () => ir), D(p, g);
        };
        ae(_, (p) => {
          R(b) && p(A);
        }, true);
      }
      D(T, h);
    };
    ae(k, (T) => {
      f() ? T(C) : T(m, false);
    });
  }
  O(d), Qe(d, (T) => i(T), () => i()), D(e, d), Ve();
}
const pt = qr(void 0), nr = "/dashboard/api";
async function zt(e) {
  let r = await fetch(`${nr}${e}`, { method: "GET" });
  return or(r);
}
async function Ht(e, r) {
  let t = await fetch(`${nr}${e}`, { method: "POST", body: r });
  return or(t);
}
function or(e) {
  return e.status === 401 && pt.set(void 0), e;
}
var yt = jr(`<svg fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M2.036 12.322a1.012 1.012 0 010-.639C3.423 7.51 7.36 4.5 12 4.5c4.638 0 8.573 3.007 9.963
            7.178.07.207.07.431 0 .639C20.577 16.49 16.64 19.5 12 19.5c-4.638 0-8.573-3.007-9.963-7.178z"></path><path stroke-linecap="round" stroke-linejoin="round" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"></path></svg>`);
function qt(e, r) {
  let t = L(r, "color", 8, "var(--col-ok)"), a = L(r, "opacity", 8, 0.9), i = L(r, "width", 8, "1.5rem");
  var n = yt();
  z(n, "stroke-width", 2), Ee(() => {
    z(n, "width", i()), z(n, "color", t()), z(n, "opacity", a());
  }), D(e, n);
}
const Et = `-- comments will be ignored but only a single query is allowed
-- press CTRL + Enter to execute
SELECT 1`, mt = { id: "SELECT 1", query: Et }, Yt = "--!auto-query";
let xt = he([mt]);
const Vt = (e) => {
  let r = "";
  const t = e || 8;
  for (let a = 0; a < t; a += 1) {
    let i = 60;
    for (; i > 57 && i < 65 || i > 90 && i < 97; ) i = Math.floor(Math.random() * 74) + 48;
    r = r.concat(String.fromCharCode(i));
  }
  return r;
};
var kt = q('<div class="relative"><div role="none" class="right svelte-19ulb1h"></div></div>'), At = q('<div class="relative"><div role="none" class="bottom svelte-19ulb1h"></div></div>'), Tt = q('<div><div class="children svelte-19ulb1h"><div class="inner svelte-19ulb1h"><!></div> <!></div> <!></div>');
function Wt(e, r) {
  Ye(r, true);
  let t = L(r, "minWidthPx", 3, 50), a = L(r, "minHeightPx", 3, 50), i, n = Q(void 0), o = Q(void 0), f = Q(he(Z(() => r.initialWidthPx))), u = Q(he(Z(() => r.initialHeightPx)));
  xe(() => {
    c();
  });
  function c() {
    var _a;
    if (i) {
      let p = i.getBoundingClientRect();
      r.resizeRight && (I(o, p.left, true), I(f, p.width, true)), r.resizeBottom && (I(n, p.top, true), I(u, p.height, true)), (_a = r.onResizeBottom) == null ? void 0 : _a.call(r, p.bottom);
    }
  }
  function s() {
    c(), window.addEventListener("mousemove", v), window.addEventListener("mouseup", w, { once: true });
  }
  function w() {
    window.removeEventListener("mousemove", v), c();
  }
  function v(p) {
    let g = window.scrollX + p.x - (R(o) || 0);
    g < t() ? I(f, t()) : I(f, g);
  }
  function b() {
    c(), window.addEventListener("mousemove", E), window.addEventListener("mouseup", y, { once: true });
  }
  function y() {
    window.removeEventListener("mousemove", E), c();
  }
  function E(p) {
    console.log(window.screenY, p.y);
    let g = window.screenY + p.clientY - (R(n) || 0);
    if (g < a() ? I(u, a()) : I(u, g), i && r.onResizeBottom) {
      let N = i.getBoundingClientRect();
      r.onResizeBottom(N.bottom);
    }
  }
  var l = Tt();
  let d;
  var k = M(l), C = M(k), m = M(C);
  Ke(m, () => r.children), O(C);
  var T = te(C, 2);
  {
    var h = (p) => {
      var g = kt(), N = M(g);
      N.__mousedown = s, O(g), D(p, g);
    };
    ae(T, (p) => {
      r.resizeRight && p(h);
    });
  }
  O(k);
  var _ = te(k, 2);
  {
    var A = (p) => {
      var g = At(), N = M(g);
      N.__mousedown = b, O(g), D(p, g);
    };
    ae(_, (p) => {
      r.resizeBottom && p(A);
    });
  }
  O(l), Qe(l, (p) => i = p, () => i), Ee(() => d = x(l, "", d, { width: R(f) && `${R(f)}px`, height: R(u) && `${R(u)}px`, border: r.border, padding: r.padding })), D(e, l), Ve();
}
We(["mousedown"]);
export {
  nr as A,
  Pt as B,
  mt as D,
  qt as I,
  xt as Q,
  Wt as R,
  z as a,
  x as b,
  Dt as c,
  Ft as d,
  Ut as e,
  Mt as f,
  pt as g,
  or as h,
  It as i,
  Nt as j,
  Je as k,
  ot as l,
  zt as m,
  Vt as n,
  Yt as o,
  ut as p,
  Ot as q,
  lt as r,
  Ke as s,
  Ge as t,
  Bt as u,
  Ht as v,
  Et as w
};
